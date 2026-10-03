//! The single source of local knowledge and representability constraints.
//!
//! Checks report paths relative to the checked node; callers prefix them with
//! the node's own path only when a check fails.

use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind, Node,
    NodeId, NodeKind, Slot,
};
use crate::knowledge::{ArgForm, ArgSpec, CommandKind, DelimiterToken, ParsedArgSpec, ValueKind};
use crate::lexer::{Token, is_whitespace_char};
use crate::parse::KnowledgeBase;
use logos::Logos;

use super::NodeSlot;

/// Stable codes describing a violated document constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConformanceRule {
    KnownFlagMismatch,
    CommandKindMismatch,
    UnknownWithArguments,
    ArgumentCount,
    ArgumentForm,
    ArgumentValueType,
    MissingRequiredArgument,
    InvalidArgumentValue,
    ModeMismatch,
    InfixPlacement,
    ScriptedWithoutScript,
    InvalidScriptedBase,
    PairedDelimiter,
    InvalidChar,
    InvalidText,
    ErrorNode,
    InvalidPrimeCount,
    InvalidDelimiter,
    EnvironmentBodyMode,
    InvalidName,
}

impl ConformanceRule {
    /// Return the stable snake_case code used by language bindings.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KnownFlagMismatch => "known_flag_mismatch",
            Self::CommandKindMismatch => "command_kind_mismatch",
            Self::UnknownWithArguments => "unknown_with_arguments",
            Self::ArgumentCount => "argument_count",
            Self::ArgumentForm => "argument_form",
            Self::ArgumentValueType => "argument_value_type",
            Self::MissingRequiredArgument => "missing_required_argument",
            Self::InvalidArgumentValue => "invalid_argument_value",
            Self::ModeMismatch => "mode_mismatch",
            Self::InfixPlacement => "infix_placement",
            Self::ScriptedWithoutScript => "scripted_without_script",
            Self::InvalidScriptedBase => "invalid_scripted_base",
            Self::PairedDelimiter => "paired_delimiter",
            Self::InvalidChar => "invalid_char",
            Self::InvalidText => "invalid_text",
            Self::ErrorNode => "error_node",
            Self::InvalidPrimeCount => "invalid_prime_count",
            Self::InvalidDelimiter => "invalid_delimiter",
            Self::EnvironmentBodyMode => "environment_body_mode",
            Self::InvalidName => "invalid_name",
        }
    }
}

impl std::fmt::Display for ConformanceRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A document constraint failure.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConformanceError {
    /// Node path rooted at `root`, or at `detached` for a detached subtree.
    /// Empty for input that is rejected before it relates to any node, such
    /// as an invalid delimiter string.
    pub path: String,
    pub rule: ConformanceRule,
    pub message: String,
}

impl ConformanceError {
    pub(super) fn new(rule: ConformanceRule, message: impl Into<String>) -> Self {
        Self {
            path: String::new(),
            rule,
            message: message.into(),
        }
    }

    /// Prefix a path relative to a checked node with that node's path.
    pub(super) fn under(mut self, base: &str) -> Self {
        self.path = if self.path.is_empty() {
            base.to_owned()
        } else {
            format!("{base}.{}", self.path)
        };
        self
    }
}

impl std::fmt::Display for ConformanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.path.is_empty() {
            write!(f, "{}: ", self.path)?;
        }
        write!(f, "{} ({})", self.message, self.rule)
    }
}
impl std::error::Error for ConformanceError {}

type Check = Result<(), ConformanceError>;

fn reject(rule: ConformanceRule, message: &str) -> Check {
    Err(ConformanceError::new(rule, message))
}

pub(super) fn check_command_kind(actual: CommandKind, expected: CommandKind) -> Check {
    if actual == expected {
        return Ok(());
    }
    // Name the constructor by kind so every binding's spelling matches.
    let kind = match expected {
        CommandKind::Prefix => "command",
        CommandKind::Declarative => "declarative",
        CommandKind::Infix => "infix",
    };
    reject(
        ConformanceRule::CommandKindMismatch,
        &format!("node kind disagrees with the command record; use the {kind} constructor"),
    )
}

pub(super) fn check_prime_count(count: usize) -> Check {
    if count == 0 {
        return reject(
            ConformanceRule::InvalidPrimeCount,
            "prime count must be a positive integer",
        );
    }
    Ok(())
}

/// Whether the parser marks the control sequence `name` as known in `mode`:
/// it has a command record there, or it is a math delimiter control.
pub(super) fn command_known(kb: &KnowledgeBase, name: &str, mode: ContentMode) -> bool {
    kb.lookup_command(name, mode).is_some()
        || (mode == ContentMode::Math && kb.lookup_delimiter(name, true, mode).is_some())
}

/// The argument signature of a command-like node in `mode`, if it has a record.
pub(super) fn signature(
    node: &Node,
    mode: ContentMode,
    kb: &KnowledgeBase,
) -> Option<ParsedArgSpec> {
    match node {
        Node::Command { name, .. } | Node::Infix { name, .. } | Node::Declarative { name, .. } => {
            kb.lookup_command(name, mode).map(|record| record.argspec)
        }
        Node::Environment { name, .. } => kb.lookup_env(name, mode).map(|record| record.argspec),
        _ => None,
    }
}

/// Match control words and control symbols without interpreting escaped symbols.
pub(crate) fn valid_control_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.bytes().all(|c| c.is_ascii_alphabetic()) {
        return true;
    }
    let mut chars = name.chars();
    let c = chars.next().expect("nonempty name");
    chars.next().is_none() && !matches!(c, '\r' | '\n')
}

/// Match names that survive the parser's escaped-symbol branch as commands.
pub(crate) fn valid_command_name(name: &str) -> bool {
    valid_control_name(name) && !matches!(name, "%" | "$" | "&" | "#" | "_" | "{" | "}")
}

/// Environment names accept only lexer Char tokens and stars.
pub(crate) fn valid_environment_name(name: &str) -> bool {
    !name.is_empty()
        && Token::lexer(name).all(|token| matches!(token, Ok(Token::Char(_) | Token::Star)))
        && !name.contains('%')
}

fn valid_char(c: char, mode: ContentMode) -> bool {
    !matches!(c, '\\' | '^' | '~' | '\u{2019}' | '\x00'..='\x08' | '\x0b'..='\x1f' | '\x7f')
        && (!is_whitespace_char(c) || (mode == ContentMode::Text && c == ' '))
        && !(mode == ContentMode::Math && c == '\'')
}

fn valid_text(value: &str) -> bool {
    !value.contains("  ")
        && value.chars().all(|c| {
            valid_char(c, ContentMode::Text)
                && !matches!(c, '%' | '$' | '&' | '#' | '_' | '{' | '}')
        })
}

fn valid_delimiter(delimiter: &Delimiter, kb: &KnowledgeBase) -> bool {
    match delimiter {
        Delimiter::None => true,
        Delimiter::Char('.') => true,
        Delimiter::Char(c) => kb
            .lookup_delimiter(&c.to_string(), false, ContentMode::Math)
            .is_some(),
        Delimiter::Control(name) => kb.lookup_delimiter(name, true, ContentMode::Math).is_some(),
    }
}

fn matches_delimiter(value: &Delimiter, expected: &DelimiterToken) -> bool {
    match (value, expected) {
        (Delimiter::Char(a), DelimiterToken::Char(b)) => a == b,
        (Delimiter::None, DelimiterToken::Char('.')) => true,
        (Delimiter::Control(a), DelimiterToken::ControlSeq(b)) => a == b.as_ref(),
        _ => false,
    }
}

fn opposite(mode: ContentMode) -> ContentMode {
    match mode {
        ContentMode::Math => ContentMode::Text,
        ContentMode::Text => ContentMode::Math,
    }
}

/// Determine the context assigned to a direct slot by the owning node.
pub(super) fn child_mode(
    node: &Node,
    slot: Slot,
    context: ContentMode,
    kb: &KnowledgeBase,
) -> ContentMode {
    match (node, slot) {
        (Node::Root { mode, .. } | Node::Group { mode, .. }, Slot::GroupChild(_)) => *mode,
        (Node::Environment { name, .. }, Slot::EnvBody) => kb
            .lookup_env(name, context)
            .map_or(context, |record| record.body_mode),
        // The value category of a content argument fixes its mode; the
        // signature check keeps that category equal to the argspec's.
        (_, Slot::Argument(index)) => node.arg_slots()[index]
            .as_ref()
            .and_then(|argument| argument.value.content())
            .map_or(context, |(_, mode)| mode),
        _ => context,
    }
}

/// The part of a node that its parent's local check reads.
pub(super) fn parent_view(node: &Node) -> (NodeKind, Option<(ContentMode, bool)>) {
    match node {
        Node::Group { mode, kind, .. } => (
            NodeKind::Group,
            Some((*mode, matches!(kind, GroupKind::InlineMath))),
        ),
        _ => (node.kind(), None),
    }
}

/// Check a node against its context mode and direct slots.
///
/// `child` returns a direct child and, when known, the context that child
/// currently has; a known context must equal the one this node assigns.
pub(super) fn check_node<'a>(
    node: &Node,
    mode: ContentMode,
    kb: &KnowledgeBase,
    child: impl Fn(NodeId) -> (&'a Node, Option<ContentMode>),
) -> Check {
    use ConformanceRule as Rule;
    let check_child = |id, expected, slot: NodeSlot| -> Check {
        if child(id).1.is_some_and(|actual| actual != expected) {
            return Err(ConformanceError::new(
                Rule::ModeMismatch,
                "subtree context differs from the destination slot",
            )
            .under(&slot.to_string()));
        }
        Ok(())
    };
    match node {
        Node::Root {
            children,
            mode: own,
        }
        | Node::Group {
            children,
            mode: own,
            ..
        } => {
            let inline = matches!(
                node,
                Node::Group {
                    kind: GroupKind::InlineMath,
                    ..
                }
            );
            if (inline && (mode != ContentMode::Text || *own != ContentMode::Math))
                || (!inline && *own != mode)
            {
                return reject(Rule::ModeMismatch, "group mode differs from its context");
            }
            if let Node::Group {
                kind: GroupKind::Delimited { left, right },
                ..
            } = node
            {
                if mode != ContentMode::Math {
                    return reject(Rule::ModeMismatch, "delimited groups require math mode");
                }
                if !valid_delimiter(left, kb) || !valid_delimiter(right, kb) {
                    return reject(
                        Rule::InvalidDelimiter,
                        "group delimiter is not in the knowledge base",
                    );
                }
            }
            for (index, id) in children.iter().enumerate() {
                if children.len() != 1 && matches!(child(*id).0, Node::Infix { .. }) {
                    return reject(
                        Rule::InfixPlacement,
                        "an infix must be its container's only child",
                    );
                }
                check_child(*id, *own, NodeSlot::Child(index))?;
            }
        }
        Node::Command { name, args, .. }
        | Node::Infix { name, args, .. }
        | Node::Declarative { name, args } => {
            let record = kb.lookup_command(name, mode);
            // The parser reads these names as syntax in the given mode, never
            // as a command node (a `begin` record applies only in text mode).
            if !valid_command_name(name)
                || name == "end"
                || (name == "begin" && (mode == ContentMode::Math || record.is_none()))
                || (mode == ContentMode::Math && matches!(name.as_str(), "left" | "right"))
            {
                return reject(
                    Rule::InvalidName,
                    "command name cannot be represented in this context",
                );
            }
            if matches!(node, Node::Infix { .. }) && mode != ContentMode::Math {
                return reject(Rule::ModeMismatch, "infix nodes require math mode");
            }
            let known = command_known(kb, name, mode);
            if !known && kb.lookup_command(name, opposite(mode)).is_some() {
                return reject(Rule::ModeMismatch, "command is not available in this mode");
            }
            if !known && !args.is_empty() {
                return reject(
                    Rule::UnknownWithArguments,
                    "unknown commands cannot carry arguments",
                );
            }
            let actual_kind = match node {
                Node::Infix { .. } => CommandKind::Infix,
                Node::Declarative { .. } => CommandKind::Declarative,
                _ => CommandKind::Prefix,
            };
            check_command_kind(
                actual_kind,
                record.map_or(CommandKind::Prefix, |record| record.kind),
            )?;
            if matches!(node, Node::Command { known: flag, .. } if *flag != known) {
                return reject(
                    Rule::KnownFlagMismatch,
                    "known flag disagrees with the knowledge base",
                );
            }
            check_arguments(args, record.map_or(&[], |record| record.argspec.args), kb)?;
            if let Node::Infix { left, right, .. } = node {
                check_child(*left, mode, NodeSlot::InfixLeft)?;
                check_child(*right, mode, NodeSlot::InfixRight)?;
            }
        }
        Node::Environment {
            name,
            args,
            known,
            body,
        } => {
            if !valid_environment_name(name) {
                return reject(Rule::InvalidName, "environment name cannot be represented");
            }
            let record = kb.lookup_env(name, mode);
            if record.is_none() && kb.lookup_env(name, opposite(mode)).is_some() {
                return reject(
                    Rule::ModeMismatch,
                    "environment is not available in this mode",
                );
            }
            if record.is_none() && !args.is_empty() {
                return reject(
                    Rule::UnknownWithArguments,
                    "unknown environments cannot carry arguments",
                );
            }
            if *known != record.is_some() {
                return reject(
                    Rule::KnownFlagMismatch,
                    "known flag disagrees with the knowledge base",
                );
            }
            check_arguments(args, record.map_or(&[], |record| record.argspec.args), kb)?;
            let expected = record.map_or(mode, |record| record.body_mode);
            if !matches!(child(*body).0, Node::Group { mode: actual, kind, .. } if *actual == expected && !matches!(kind, GroupKind::InlineMath))
            {
                return reject(
                    Rule::EnvironmentBodyMode,
                    "environment body must be a group in the declared body mode",
                );
            }
            check_child(*body, expected, NodeSlot::EnvBody)?;
        }
        Node::Scripted {
            base,
            subscript,
            superscript,
        } => {
            if mode != ContentMode::Math {
                return reject(Rule::ModeMismatch, "scripts require math mode");
            }
            if subscript.is_none() && superscript.is_none() {
                return reject(
                    Rule::ScriptedWithoutScript,
                    "scripted nodes require a subscript or superscript",
                );
            }
            if matches!(
                child(*base).0,
                Node::Scripted { .. } | Node::Infix { .. } | Node::AlignmentTab
            ) {
                return reject(
                    Rule::InvalidScriptedBase,
                    "this node cannot be the base of a script",
                );
            }
            check_child(*base, mode, NodeSlot::ScriptBase)?;
            if let Some(id) = subscript {
                check_child(*id, mode, NodeSlot::Subscript)?;
            }
            if let Some(id) = superscript {
                check_child(*id, mode, NodeSlot::Superscript)?;
            }
        }
        Node::Prime { count } => {
            check_prime_count(*count)?;
            if mode != ContentMode::Math {
                return reject(Rule::ModeMismatch, "primes require math mode");
            }
        }
        Node::Char(c) if !valid_char(*c, mode) => {
            return reject(
                Rule::InvalidChar,
                "character cannot be represented in this mode",
            );
        }
        Node::Text(_) if mode != ContentMode::Text => {
            return reject(Rule::ModeMismatch, "text requires text mode");
        }
        Node::Text(value) if !valid_text(value) => {
            return reject(
                Rule::InvalidText,
                "text contains characters that require distinct syntax nodes",
            );
        }
        Node::AlignmentTab if mode != ContentMode::Math => {
            return reject(Rule::ModeMismatch, "alignment tabs require math mode");
        }
        Node::Error { .. } => {
            return reject(
                Rule::ErrorNode,
                "error nodes cannot be introduced into editable documents",
            );
        }
        Node::Text(_) | Node::Char(_) | Node::AlignmentTab | Node::ActiveSpace => {}
    }
    for (index, argument) in node.arg_slots().iter().enumerate() {
        if let Some((id, expected)) = argument.as_ref().and_then(|arg| arg.value.content()) {
            check_child(id, expected, NodeSlot::Arg(index))?;
        }
    }
    Ok(())
}

fn check_arguments(args: &[ArgumentSlot], specs: &'static [ArgSpec], kb: &KnowledgeBase) -> Check {
    if args.len() != specs.len() {
        return reject(
            ConformanceRule::ArgumentCount,
            "argument slot count differs from the signature",
        );
    }
    for (index, (argument, spec)) in args.iter().zip(specs).enumerate() {
        check_argument(argument.as_ref(), spec, kb)
            .map_err(|error| error.under(&format!("arg.{index}")))?;
    }
    Ok(())
}

fn check_argument(
    argument: Option<&crate::ast::Argument>,
    spec: &'static ArgSpec,
    kb: &KnowledgeBase,
) -> Check {
    use ConformanceRule as Rule;
    let Some(argument) = argument else {
        if spec.required || matches!(spec.form, ArgForm::Star) {
            return reject(
                Rule::MissingRequiredArgument,
                "required argument slot is absent",
            );
        }
        return Ok(());
    };
    let form_matches = match (&argument.kind, &spec.form) {
        (ArgumentKind::Mandatory, ArgForm::Standard) => spec.required,
        (ArgumentKind::Optional, ArgForm::Standard) => !spec.required,
        (ArgumentKind::Star, ArgForm::Star) | (ArgumentKind::Group, ArgForm::Group) => true,
        (ArgumentKind::Until { close }, ArgForm::Until { close: expected }) => {
            matches_delimiter(close, expected)
        }
        (ArgumentKind::Delimited { open, close }, ArgForm::Delimited { open: a, close: b }) => {
            matches_delimiter(open, a) && matches_delimiter(close, b)
        }
        (ArgumentKind::Paired { open, close }, ArgForm::Paired { pairs }) => {
            if !pairs
                .iter()
                .any(|(a, b)| matches_delimiter(open, a) && matches_delimiter(close, b))
            {
                return reject(
                    Rule::PairedDelimiter,
                    "paired argument boundaries are not a declared candidate pair",
                );
            }
            true
        }
        _ => false,
    };
    if !form_matches || argument.no_leading_space != spec.no_leading_space {
        return reject(
            Rule::ArgumentForm,
            "argument form differs from the signature",
        );
    }
    let value_matches = matches!(
        (&argument.value, spec.kind),
        (
            ArgumentValue::MathContent(_),
            ValueKind::Content {
                mode: ContentMode::Math
            }
        ) | (
            ArgumentValue::TextContent(_),
            ValueKind::Content {
                mode: ContentMode::Text
            }
        ) | (
            ArgumentValue::OperatorNameContent(_),
            ValueKind::OperatorName
        ) | (ArgumentValue::Delimiter(_), ValueKind::Delimiter)
            | (ArgumentValue::CSName(_), ValueKind::CSName)
            | (ArgumentValue::Dimension(_), ValueKind::Dimension)
            | (ArgumentValue::Integer(_), ValueKind::Integer)
            | (ArgumentValue::KeyVal(_), ValueKind::KeyVal)
            | (ArgumentValue::Column(_), ValueKind::Column)
            | (ArgumentValue::Boolean(_), ValueKind::Star)
    );
    if !value_matches {
        return reject(
            Rule::ArgumentValueType,
            "argument value type differs from the signature",
        );
    }
    if let ArgumentValue::Delimiter(delimiter) = &argument.value
        && !valid_delimiter(delimiter, kb)
    {
        return reject(
            Rule::InvalidDelimiter,
            "argument delimiter is not in the knowledge base",
        );
    }
    crate::parse::grammar::validate_scalar_argument(kb, argument, spec)
        .map_err(|message| ConformanceError::new(Rule::InvalidArgumentValue, message))
}

/// Validate every node of a subtree using local rules only.
///
/// Error paths start at `label`, which names the subtree root.
pub(super) fn check_tree(
    ast: &Ast,
    root: NodeId,
    mode: ContentMode,
    kb: &KnowledgeBase,
    label: &str,
) -> Check {
    let mut pending = vec![(root, mode)];
    while let Some((id, mode)) = pending.pop() {
        let node = ast.node(id);
        check_node(node, mode, kb, |child| (ast.node(child), None))
            .map_err(|error| error.under(&super::path::path_below(ast, root, id, label)))?;
        for (child, slot) in ast.edges(id).into_iter().rev() {
            pending.push((child, child_mode(node, slot, mode, kb)));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "conformance_tests.rs"]
mod tests;
