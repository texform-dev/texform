//! Transformed-tree round-trip checks shared by the fast round-trip tests and
//! the corpus round-trip check.
//!
//! The transform output contract is that the output text reparses to the
//! transformed tree: for every transformed tree `t` and both output styles,
//! `parse(serialize(t)) == t` under [`canonical`] equality. It follows from
//! two properties. Parsing and serialization round-trip every tree the parser
//! can produce, and the transform only produces such trees. Adding
//! transform stability gives idempotence as a corollary.
//!
//! | [`Check`] | Property |
//! | --- | --- |
//! | `parser_round_trip` | `parse(serialize(parse(x))) == parse(x)` and the text is stable |
//! | `reparse` | `parse(serialize(t)) == t` |
//! | `reparse_error` | `serialize(t)` parses without errors; such a failure is also a `reparse` failure |
//! | `retransform` | `transform(t) == t`, without serialization |
//! | `reparse_retransform` | `transform(parse(serialize(t))) == t` and its text equals `serialize(t)` |
//!
//! Inputs whose parse is incomplete or whose transform fails are skipped by
//! these checks; callers count them separately.
//!
//! Unstable: this module is internal test support and may change without
//! notice.

pub mod adversarial;

use std::fmt;

use serde::Serialize;
use texform_core::ast::Ast;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_core::serialize::{Style, serialize_with};
use texform_interface::syntax_node::{ArgumentSlot, ArgumentValue, GroupKind, SyntaxNode};

use crate::{Profile, TransformContext};

/// The library default packages.
pub const DEFAULT_PACKAGES: &[&str] = &[
    "base",
    "ams",
    "braket",
    "textmacros",
    "bboldx",
    "boldsymbol",
    "color",
];
/// The packages that introduce most command captures.
pub const PHYSICS_PACKAGES: &[&str] = &["base", "ams", "physics"];
/// The library defaults with `physics` enabled.
pub const MIXED_PACKAGES: &[&str] = &[
    "base",
    "ams",
    "braket",
    "textmacros",
    "bboldx",
    "boldsymbol",
    "color",
    "physics",
];

/// Named package selections checked by the fast tests and the report. They
/// are fixed here rather than taken from the library defaults.
pub const PACKAGE_SETS: [(&str, &[&str]); 3] = [
    ("default", DEFAULT_PACKAGES),
    ("physics", PHYSICS_PACKAGES),
    ("mixed", MIXED_PACKAGES),
];

pub const PROFILES: [Profile; 4] = [
    Profile::Authoring,
    Profile::Faithful,
    Profile::Corpus,
    Profile::Equiv,
];

pub const STYLES: [Style; 2] = [Style::Spaced, Style::Compact];

pub fn profile_name(profile: Profile) -> &'static str {
    match profile {
        Profile::Authoring => "authoring",
        Profile::Faithful => "faithful",
        Profile::Corpus => "corpus",
        Profile::Equiv => "equiv",
    }
}

pub fn style_name(style: Style) -> &'static str {
    match style {
        Style::Spaced => "spaced",
        Style::Compact => "compact",
    }
}

/// One round-trip check; [`Check::label`] gives its name in reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Check {
    /// Parsing and serialization round-trip the parsed input.
    ParserRoundTrip,
    /// The output text reparses to the transformed tree.
    Reparse,
    /// The output text parses without errors.
    ReparseError,
    /// Transforming the transformed tree again leaves it unchanged.
    Retransform,
    /// Transforming the reparsed output gives the transformed tree and text.
    ReparseRetransform,
}

impl Check {
    pub fn label(self) -> &'static str {
        match self {
            Check::ParserRoundTrip => "parser_round_trip",
            Check::Reparse => "reparse",
            Check::ReparseError => "reparse_error",
            Check::Retransform => "retransform",
            Check::ReparseRetransform => "reparse_retransform",
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The first place where two canonical trees differ.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Difference {
    /// Edge path from the root, such as `children[1].args[0]`.
    pub path: String,
    /// The node on the expected side, or `-` when it is absent.
    pub expected: String,
    /// The node on the actual side, or `-` when it is absent.
    pub actual: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {}: {} vs {}", self.path, self.expected, self.actual)
    }
}

/// One failed check.
#[derive(Clone, Debug)]
pub struct Failure {
    pub check: Check,
    /// `None` for `Retransform`, which does not serialize.
    pub style: Option<Style>,
    /// The serialized text under `style`, or of the transformed tree for `Retransform`.
    pub output: String,
    /// The first tree difference, when both trees exist.
    pub difference: Option<Difference>,
    /// Further context: a second output or an error message.
    pub detail: Option<String>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.check)?;
        if let Some(style) = self.style {
            write!(f, " {}", style_name(style))?;
        }
        write!(f, ": {}", self.output)?;
        if let Some(difference) = &self.difference {
            write!(f, " ({difference})")?;
        }
        if let Some(detail) = &self.detail {
            write!(f, " [{detail}]")?;
        }
        Ok(())
    }
}

/// The result of checking one complete input under one profile.
#[derive(Debug)]
pub enum Outcome {
    /// The transform returned an error; no check ran.
    TransformFailed(String),
    /// The checks ran; an empty list means every check passed.
    Checked(Vec<Failure>),
}

/// Parse configuration used for inputs and reparses: unknown names are kept,
/// and an input with errors counts as incomplete.
pub const PARSE_CONFIG: ParseConfig = ParseConfig {
    reject_unknown: false,
    abort_on_error: true,
    max_group_depth: ParseConfig::DEFAULT_MAX_GROUP_DEPTH,
};

/// Parses `source`, returning `None` unless the result is complete.
pub fn parse(parse_ctx: &ParseContext, source: &str) -> Option<Ast> {
    parse_ctx
        .parse(source, &PARSE_CONFIG)
        .try_into_document()
        .ok()
        .map(|(document, _)| Ast::from_syntax_root(&document.to_syntax()))
}

pub fn print(ast: &Ast, style: Style) -> String {
    serialize_with(ast, &style.into())
}

/// `ParserRoundTrip`: parsing and serialization round-trip the parsed tree `original`.
pub fn check_parser(parse_ctx: &ParseContext, original: &Ast, style: Style) -> Option<Failure> {
    let output = print(original, style);
    let failure = |difference, detail| Failure {
        check: Check::ParserRoundTrip,
        style: Some(style),
        output: output.clone(),
        difference,
        detail,
    };
    let Some(reparsed) = parse(parse_ctx, &output) else {
        return Some(failure(None, Some("reparse is incomplete".into())));
    };
    if let Some(difference) = tree_difference(original, &reparsed) {
        return Some(failure(Some(difference), None));
    }
    let again = print(&reparsed, style);
    (again != output).then(|| failure(None, Some(format!("reserialized as {again}"))))
}

/// Transforms a copy of `original` and runs the transformed-tree checks on the
/// result.
pub fn check_transform(
    parse_ctx: &ParseContext,
    engine: &TransformContext,
    original: &Ast,
) -> Outcome {
    let mut transformed = original.clone();
    if let Err(error) = engine.run(&mut transformed, parse_ctx) {
        return Outcome::TransformFailed(error.to_string());
    }
    Outcome::Checked(check_transformed(parse_ctx, engine, &transformed))
}

/// Runs the transformed-tree checks on a transformed tree.
pub fn check_transformed(
    parse_ctx: &ParseContext,
    engine: &TransformContext,
    transformed: &Ast,
) -> Vec<Failure> {
    let mut failures = Vec::new();
    let mut again = transformed.clone();
    match engine.run(&mut again, parse_ctx) {
        Ok(()) => {
            if let Some(difference) = tree_difference(transformed, &again) {
                failures.push(Failure {
                    check: Check::Retransform,
                    style: None,
                    output: print(transformed, Style::Spaced),
                    difference: Some(difference),
                    detail: Some(format!("retransformed as {}", print(&again, Style::Spaced))),
                });
            }
        }
        Err(error) => failures.push(Failure {
            check: Check::Retransform,
            style: None,
            output: print(transformed, Style::Spaced),
            difference: None,
            detail: Some(format!("retransform failed: {error}")),
        }),
    }

    for style in STYLES {
        let output = print(transformed, style);
        let failure = |check, difference, detail| Failure {
            check,
            style: Some(style),
            output: output.clone(),
            difference,
            detail,
        };
        let Some(reparsed) = parse(parse_ctx, &output) else {
            // `ReparseRetransform` needs the reparsed tree; `ReparseError` already
            // records this output.
            failures.push(failure(
                Check::Reparse,
                None,
                Some("reparse is incomplete".into()),
            ));
            failures.push(failure(Check::ReparseError, None, None));
            continue;
        };
        if let Some(difference) = tree_difference(transformed, &reparsed) {
            failures.push(failure(Check::Reparse, Some(difference), None));
        }
        let mut stable = reparsed;
        match engine.run(&mut stable, parse_ctx) {
            Ok(()) => {
                let text = print(&stable, style);
                let difference = tree_difference(transformed, &stable);
                if difference.is_some() || text != output {
                    failures.push(failure(
                        Check::ReparseRetransform,
                        difference,
                        Some(format!("retransformed as {text}")),
                    ));
                }
            }
            Err(error) => failures.push(failure(
                Check::ReparseRetransform,
                None,
                Some(format!("retransform failed: {error}")),
            )),
        }
    }
    failures
}

/// The first difference between the canonical forms of two trees.
pub fn tree_difference(expected: &Ast, actual: &Ast) -> Option<Difference> {
    let expected = canonical(expected.to_syntax_root(), false);
    let actual = canonical(actual.to_syntax_root(), false);
    first_difference(&expected, &actual, String::new())
}

/// Removes representation-only differences before tree comparison.
///
/// Two trees may differ here only when the parser cannot tell their
/// serializations apart and the reference renderer reads them the same by
/// definition. This strips the one container that an argument, script, or
/// environment body slot owns, and erases the explicit/implicit group
/// distinction. It never ignores a group in a sequence, including an empty
/// one, since that group changes spacing and binding.
///
/// A `Prime` node outside a superscript serializes as one `\prime` command
/// per mark, which is exactly what reparsing that text yields, so it compares
/// as those commands. A superscript `Prime` serializes as `'` marks and stays.
pub fn canonical(node: SyntaxNode, slot: bool) -> SyntaxNode {
    match canonical_node(node, slot) {
        SyntaxNode::Prime { count: 1 } => prime_command(),
        SyntaxNode::Prime { count } => SyntaxNode::Group {
            mode: texform_interface::syntax_node::ContentMode::Math,
            kind: GroupKind::Implicit,
            children: vec![prime_command(); count],
        },
        node => node,
    }
}

fn prime_command() -> SyntaxNode {
    SyntaxNode::Command {
        name: "prime".into(),
        args: Vec::new(),
        known: true,
    }
}

fn canonical_node(mut node: SyntaxNode, slot: bool) -> SyntaxNode {
    if slot
        && let SyntaxNode::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            children,
            ..
        } = &mut node
        && children.len() == 1
    {
        // Strip exactly the slot's own container, never its child's braces.
        node = children.remove(0);
    }
    fn args(args: &mut [ArgumentSlot]) {
        for arg in args.iter_mut().flatten() {
            match &mut arg.value {
                ArgumentValue::MathContent(n)
                | ArgumentValue::TextContent(n)
                | ArgumentValue::OperatorNameContent(n) => {
                    *n = canonical(std::mem::replace(n, placeholder()), true)
                }
                _ => {}
            }
        }
    }
    fn slot_child(child: &mut SyntaxNode) {
        *child = canonical(std::mem::replace(child, placeholder()), true);
    }
    match &mut node {
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => {
            *children = std::mem::take(children)
                .into_iter()
                .flat_map(|child| match canonical_node(child, false) {
                    SyntaxNode::Prime { count } => vec![prime_command(); count],
                    child => vec![child],
                })
                .collect();
        }
        SyntaxNode::Command { args: a, .. } | SyntaxNode::Declarative { args: a, .. } => args(a),
        SyntaxNode::Environment { args: a, body, .. } => {
            args(a);
            slot_child(body);
        }
        SyntaxNode::Infix {
            args: a,
            left,
            right,
            ..
        } => {
            args(a);
            slot_child(left);
            slot_child(right);
        }
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            slot_child(base);
            if let Some(child) = subscript {
                slot_child(child);
            }
            if let Some(child) = superscript {
                **child = canonical_node(std::mem::replace(child, placeholder()), true);
            }
        }
        _ => {}
    }
    if let SyntaxNode::Group {
        kind: kind @ (GroupKind::Explicit | GroupKind::Implicit),
        ..
    } = &mut node
    {
        *kind = GroupKind::Implicit;
    }
    node
}

fn placeholder() -> SyntaxNode {
    SyntaxNode::Char(' ')
}

fn first_difference(
    expected: &SyntaxNode,
    actual: &SyntaxNode,
    path: String,
) -> Option<Difference> {
    if expected == actual {
        return None;
    }
    let (expected_edges, actual_edges) = (edges(expected), edges(actual));
    let here = || Difference {
        path: if path.is_empty() {
            "root".into()
        } else {
            path.clone()
        },
        expected: label(expected),
        actual: label(actual),
    };
    if shallow(expected) != shallow(actual) {
        return Some(here());
    }
    for index in 0..expected_edges.len().max(actual_edges.len()) {
        match (
            expected_edges.get(index).cloned(),
            actual_edges.get(index).cloned(),
        ) {
            (Some((name, Some(left))), Some((_, Some(right)))) => {
                let child_path = if path.is_empty() {
                    name
                } else {
                    format!("{path}.{name}")
                };
                if let Some(difference) = first_difference(left, right, child_path) {
                    return Some(difference);
                }
            }
            (Some((_, None)), Some((_, None))) => {}
            (left, right) => {
                let name = left
                    .as_ref()
                    .or(right.as_ref())
                    .map(|(name, _)| name.clone());
                let side = |edge: Option<(String, Option<&SyntaxNode>)>| {
                    edge.and_then(|(_, node)| node)
                        .map_or_else(|| "-".into(), label)
                };
                return Some(Difference {
                    path: match (path.is_empty(), name) {
                        (_, None) => path.clone(),
                        (true, Some(name)) => name,
                        (false, Some(name)) => format!("{path}.{name}"),
                    },
                    expected: side(left),
                    actual: side(right),
                });
            }
        }
    }
    Some(here())
}

/// Child edges in serialization order. Absent argument values and scripts
/// appear as `None` so that their presence is compared too.
fn edges(node: &SyntaxNode) -> Vec<(String, Option<&SyntaxNode>)> {
    fn args(args: &[ArgumentSlot]) -> Vec<(String, Option<&SyntaxNode>)> {
        args.iter()
            .enumerate()
            .filter_map(|(index, arg)| {
                let content = match arg.as_ref().map(|arg| &arg.value) {
                    Some(
                        ArgumentValue::MathContent(n)
                        | ArgumentValue::TextContent(n)
                        | ArgumentValue::OperatorNameContent(n),
                    ) => Some(n),
                    _ => None,
                };
                content.map(|n| (format!("args[{index}]"), Some(n)))
            })
            .collect()
    }
    match node {
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => children
            .iter()
            .enumerate()
            .map(|(index, child)| (format!("children[{index}]"), Some(child)))
            .collect(),
        SyntaxNode::Command { args: a, .. } | SyntaxNode::Declarative { args: a, .. } => args(a),
        SyntaxNode::Environment { args: a, body, .. } => {
            let mut edges = args(a);
            edges.push(("body".into(), Some(body)));
            edges
        }
        SyntaxNode::Infix {
            args: a,
            left,
            right,
            ..
        } => {
            let mut edges = vec![("left".into(), Some(&**left))];
            edges.extend(args(a));
            edges.push(("right".into(), Some(right)));
            edges
        }
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => vec![
            ("base".into(), Some(base)),
            ("sub".into(), subscript.as_deref()),
            ("sup".into(), superscript.as_deref()),
        ],
        _ => Vec::new(),
    }
}

/// Everything a node compares on except its content children.
fn shallow(node: &SyntaxNode) -> String {
    let args = |args: &[ArgumentSlot]| {
        args.iter()
            .map(|arg| match arg {
                None => "-".to_string(),
                Some(arg) => match &arg.value {
                    ArgumentValue::MathContent(_) => format!("{:?}:math", arg.kind),
                    ArgumentValue::TextContent(_) => format!("{:?}:text", arg.kind),
                    ArgumentValue::OperatorNameContent(_) => format!("{:?}:opname", arg.kind),
                    value => format!("{:?}:{value:?}", arg.kind),
                },
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    match node {
        SyntaxNode::Root { mode, .. } => format!("Root {mode:?}"),
        SyntaxNode::Group { mode, kind, .. } => format!("Group {mode:?} {kind:?}"),
        SyntaxNode::Command {
            name,
            args: a,
            known,
        } => format!("Command {name} {known} {}", args(a)),
        SyntaxNode::Declarative { name, args: a } => format!("Declarative {name} {}", args(a)),
        SyntaxNode::Environment {
            name,
            args: a,
            known,
            ..
        } => format!("Environment {name} {known} {}", args(a)),
        SyntaxNode::Infix { name, args: a, .. } => format!("Infix {name} {}", args(a)),
        SyntaxNode::Scripted {
            subscript,
            superscript,
            ..
        } => format!("Scripted {} {}", subscript.is_some(), superscript.is_some()),
        other => format!("{other:?}"),
    }
}

/// A short node name for reports.
pub fn label(node: &SyntaxNode) -> String {
    match node {
        SyntaxNode::Root { .. } => "Root".into(),
        SyntaxNode::Group { kind, children, .. } => match kind {
            GroupKind::Explicit | GroupKind::Implicit => format!("Group[{}]", children.len()),
            GroupKind::Delimited { .. } => format!("Delimited[{}]", children.len()),
            GroupKind::InlineMath => format!("InlineMath[{}]", children.len()),
        },
        SyntaxNode::Command { name, .. } | SyntaxNode::Declarative { name, .. } => {
            format!("\\{name}")
        }
        SyntaxNode::Infix { name, .. } => format!("\\{name} (infix)"),
        SyntaxNode::Environment { name, .. } => format!("{{{name}}}"),
        SyntaxNode::Scripted { .. } => "Scripted".into(),
        SyntaxNode::Error { .. } => "Error".into(),
        SyntaxNode::Prime { count } => format!("Prime({count})"),
        SyntaxNode::Text(text) => format!("Text({text:?})"),
        SyntaxNode::Char(c) => format!("Char({c:?})"),
        other => format!("{other:?}"),
    }
}

/// Builds one transform context per profile in [`PROFILES`].
pub fn engines(parse_ctx: &ParseContext) -> Vec<TransformContext> {
    PROFILES
        .into_iter()
        .map(|profile| {
            TransformContext::from_build_config(crate::BuildConfig::profile(profile), parse_ctx)
                .expect("profile should build")
        })
        .collect()
}

#[cfg(test)]
mod tests;
