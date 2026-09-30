use super::*;
use crate::knowledge::{ArgForm, ArgSpec, CommandKind, DelimiterToken, ParsedArgSpec, ValueKind};
use crate::parse::{ParseConfig, parse_with_context_mode};
use std::collections::HashSet;

/// Input for a content or typed argument slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Node(NodeId),
    Source(String),
    Star(bool),
    Absent,
    Paired {
        value: Box<Arg>,
        open: DelimiterValue,
        close: DelimiterValue,
    },
}
impl From<NodeId> for Arg {
    fn from(value: NodeId) -> Self {
        Self::Node(value)
    }
}
impl From<String> for Arg {
    fn from(value: String) -> Self {
        Self::Source(value)
    }
}
impl From<&str> for Arg {
    fn from(value: &str) -> Self {
        Self::Source(value.into())
    }
}
impl From<bool> for Arg {
    fn from(value: bool) -> Self {
        Self::Star(value)
    }
}
impl<T: Into<Arg>> From<Option<T>> for Arg {
    fn from(value: Option<T>) -> Self {
        value.map(Into::into).unwrap_or(Self::Absent)
    }
}
impl Arg {
    pub fn paired(
        value: impl Into<Arg>,
        open: impl AsRef<str>,
        close: impl AsRef<str>,
    ) -> Result<Self, EditError> {
        Ok(Self::Paired {
            value: Box::new(value.into()),
            open: open.as_ref().parse()?,
            close: close.as_ref().parse()?,
        })
    }
}

impl std::str::FromStr for DelimiterValue {
    type Err = EditError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "." {
            return Ok(Self::None);
        }
        if let Some(name) = value.strip_prefix('\\') {
            if !name.is_empty() {
                return Ok(Self::Control(name.into()));
            }
        } else {
            let mut chars = value.chars();
            if let Some(ch) = chars.next()
                && chars.next().is_none()
            {
                return Ok(Self::Char(ch));
            }
        }
        Err(ConformanceError::new(
            ConformanceRule::InvalidDelimiter,
            "expected one character or a control sequence",
        )
        .into())
    }
}
/// Parse the one-character string that language bindings accept for a `Char`.
///
/// Like an invalid delimiter string, a rejected input has an empty path.
pub fn parse_char(value: &str) -> Result<char, EditError> {
    let mut chars = value.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(ch),
        _ => Err(ConformanceError::new(
            ConformanceRule::InvalidChar,
            "expected exactly one character",
        )
        .into()),
    }
}

impl std::fmt::Display for DelimiterValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        DelimiterRef::from(self).fmt(f)
    }
}

/// A temporary construction view with an explicit context mode.
pub struct InMode<'a> {
    doc: &'a mut Document,
    mode: ContentMode,
}

/// The argument kind a slot built for `spec` starts with; Paired slots
/// default to the first candidate pair.
pub(super) fn spec_kind(spec: &ArgSpec) -> ArgumentKind {
    let delimiter = |token: &DelimiterToken| match token {
        DelimiterToken::Char(ch) => Delimiter::Char(*ch),
        DelimiterToken::ControlSeq(name) => Delimiter::Control(name.to_string()),
    };
    match &spec.form {
        ArgForm::Standard if spec.required => ArgumentKind::Mandatory,
        ArgForm::Standard => ArgumentKind::Optional,
        ArgForm::Star => ArgumentKind::Star,
        ArgForm::Group => ArgumentKind::Group,
        ArgForm::Delimited { open, close } => ArgumentKind::Delimited {
            open: delimiter(open),
            close: delimiter(close),
        },
        ArgForm::Paired { pairs } => {
            let (open, close) = pairs
                .first()
                .expect("the argspec parser requires at least one candidate pair");
            ArgumentKind::Paired {
                open: delimiter(open),
                close: delimiter(close),
            }
        }
    }
}

impl Document {
    /// Run a construction or edit that may stage source arguments.
    ///
    /// `op` records every detached root it allocates in the staging list. If
    /// it fails, those roots are removed, so failed operations leave the
    /// document unchanged. `op` must not adopt input nodes before its last
    /// fallible step.
    pub(super) fn transact<T>(
        &mut self,
        op: impl FnOnce(&mut Self, &mut Vec<RawNodeId>) -> Result<T, EditError>,
    ) -> Result<T, EditError> {
        let mut staged = Vec::new();
        let result = op(self, &mut staged);
        if result.is_err() {
            for id in staged {
                self.ast.remove_detached(id);
                self.detached_modes.remove(id);
            }
        }
        result
    }

    /// Resolve content input to a detached root: an existing detached node, or
    /// source parsed in `mode` and staged in the arena.
    pub(super) fn content(
        &mut self,
        staged: &mut Vec<RawNodeId>,
        value: Arg,
        mode: ContentMode,
    ) -> Result<RawNodeId, EditError> {
        match value {
            Arg::Node(id) => {
                let raw = self.check_node_owner(id)?;
                self.check_detached(raw)?;
                Ok(raw)
            }
            Arg::Source(src) => Ok(self.stage_source(staged, &src, mode, false)?[0]),
            _ => Err(ConformanceError::new(
                ConformanceRule::ArgumentValueType,
                "expected a node or source text",
            )
            .into()),
        }
    }

    /// Resolve group-child inputs, splicing the nodes each source parses to.
    fn group_children(
        &mut self,
        staged: &mut Vec<RawNodeId>,
        values: impl IntoIterator<Item = Arg>,
        mode: ContentMode,
    ) -> Result<Vec<RawNodeId>, EditError> {
        let mut children = Vec::new();
        for value in values {
            match value {
                Arg::Source(src) => children.extend(self.stage_source(staged, &src, mode, true)?),
                value => children.push(self.content(staged, value, mode)?),
            }
        }
        Ok(children)
    }

    /// Parse `src` in `mode` and stage detached copies: the fragment as one
    /// implicit group, or with `splice` each of its top-level nodes.
    fn stage_source(
        &mut self,
        staged: &mut Vec<RawNodeId>,
        src: &str,
        mode: ContentMode,
        splice: bool,
    ) -> Result<Vec<RawNodeId>, EditError> {
        let (parsed, diagnostics) =
            parse_with_context_mode(&self.knowledge_base, src, &ParseConfig::default(), mode)
                .into_parts();
        let parsed = match parsed {
            Some(parsed) if diagnostics.is_empty() && !parsed.has_errors() => parsed,
            _ => return Err(EditError::InvalidSource(diagnostics)),
        };
        let ast = &parsed.ast;
        let roots = if splice {
            ast.children(ast.root()).to_vec()
        } else {
            vec![ast.root()]
        };
        // Parser output is conformant; only the copied roots' contexts are recorded.
        Ok(roots
            .into_iter()
            .map(|id| {
                let raw = self.ast.copy_subtree_from(ast, id);
                self.detached_modes.insert(raw, mode);
                staged.push(raw);
                raw
            })
            .collect())
    }

    /// Build the slot for `spec` from `value`, keeping `old_kind`'s Paired
    /// boundaries for ordinary values.
    pub(super) fn argument(
        &mut self,
        staged: &mut Vec<RawNodeId>,
        value: Arg,
        spec: &ArgSpec,
        old_kind: Option<&ArgumentKind>,
    ) -> Result<ArgumentSlot, EditError> {
        let paired = matches!(spec.form, ArgForm::Paired { .. });
        let mut kind = match old_kind {
            Some(kind @ ArgumentKind::Paired { .. }) if paired => kind.clone(),
            _ => spec_kind(spec),
        };
        let value = match value {
            Arg::Paired { value, open, close } => {
                if !paired {
                    return Err(ConformanceError::new(
                        ConformanceRule::ArgumentForm,
                        "explicit boundary pairs require a paired slot",
                    )
                    .into());
                }
                kind = ArgumentKind::Paired {
                    open: open.into_ast(),
                    close: close.into_ast(),
                };
                *value
            }
            value => value,
        };
        let value =
            match (spec.kind, value) {
                (ValueKind::Star, Arg::Absent) => ArgumentValue::Boolean(false),
                (ValueKind::Star, Arg::Star(value)) => ArgumentValue::Boolean(value),
                (_, Arg::Absent) => return Ok(None),
                (ValueKind::Content { mode }, value) => {
                    let id = self.content(staged, value, mode)?;
                    match mode {
                        ContentMode::Math => ArgumentValue::MathContent(id),
                        ContentMode::Text => ArgumentValue::TextContent(id),
                    }
                }
                (ValueKind::OperatorName, value) => ArgumentValue::OperatorNameContent(
                    self.content(staged, value, ContentMode::Math)?,
                ),
                (ValueKind::Delimiter, Arg::Source(value)) => {
                    ArgumentValue::Delimiter(value.parse::<DelimiterValue>()?.into_ast())
                }
                (ValueKind::CSName, Arg::Source(value)) => ArgumentValue::CSName(value),
                (ValueKind::Dimension, Arg::Source(value)) => ArgumentValue::Dimension(value),
                (ValueKind::Integer, Arg::Source(value)) => ArgumentValue::Integer(value),
                (ValueKind::KeyVal, Arg::Source(value)) => ArgumentValue::KeyVal(value),
                (ValueKind::Column, Arg::Source(value)) => ArgumentValue::Column(value),
                _ => {
                    return Err(ConformanceError::new(
                        ConformanceRule::ArgumentValueType,
                        "input does not match the argument value type",
                    )
                    .into());
                }
            };
        Ok(Some(Argument {
            kind,
            value,
            no_leading_space: spec.no_leading_space,
        }))
    }

    /// Build all slots of `signature` from a complete or required-only list.
    fn arguments(
        &mut self,
        staged: &mut Vec<RawNodeId>,
        values: Vec<Arg>,
        signature: ParsedArgSpec,
    ) -> Result<Vec<ArgumentSlot>, EditError> {
        let specs = signature.args;
        let required = specs.iter().filter(|spec| spec.required).count();
        if values.len() != specs.len() && values.len() != required {
            return Err(ConformanceError::new(
                ConformanceRule::ArgumentCount,
                format!(
                    "expected {} complete slots or {required} required slots for signature {}",
                    specs.len(),
                    signature.source
                ),
            )
            .into());
        }
        let full = values.len() == specs.len();
        let mut values = values.into_iter();
        specs
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                let value = if full || spec.required {
                    values.next().expect("argument count checked")
                } else {
                    Arg::Absent
                };
                self.argument(staged, value, spec, None)
                    .map_err(|error| error.under(&format!("arg.{index}")))
            })
            .collect()
    }

    /// Check a new node in its construction context and allocate it as a
    /// detached root adopting its direct children. Errors are relative to the
    /// new node.
    fn finish(&mut self, node: Node, mode: ContentMode) -> Result<NodeId, EditError> {
        let children = Ast::node_edges(&node);
        check_unique(children.iter().map(|(child, _)| *child))?;
        self.check_local(&node, mode, None)?;
        for (child, _) in children {
            self.detached_modes.remove(child);
        }
        let raw = self.ast.new_node(node);
        self.detached_modes.insert(raw, mode);
        self.debug_assert_conformance();
        Ok(NodeId::new(self.id, raw))
    }
}

fn check_unique(ids: impl IntoIterator<Item = RawNodeId>) -> Result<(), EditError> {
    let mut seen = HashSet::new();
    if ids.into_iter().all(|id| seen.insert(id)) {
        Ok(())
    } else {
        Err(EditError::DuplicateChild)
    }
}

impl Document {
    /// Construct nodes in an explicit context without changing the document.
    pub fn in_mode(&mut self, mode: ContentMode) -> InMode<'_> {
        InMode { doc: self, mode }
    }

    /// Parse source into a detached implicit group.
    pub fn parse_fragment(
        &mut self,
        source: &str,
        mode: Option<ContentMode>,
    ) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let mode = mode.unwrap_or_else(|| self.root_mode());
        // Nothing can fail after parsing, so the staged root needs no rollback.
        let raw = self.content(&mut Vec::new(), Arg::Source(source.into()), mode)?;
        self.debug_assert_conformance();
        Ok(NodeId::new(self.id, raw))
    }
}

impl InMode<'_> {
    fn build(
        &mut self,
        build: impl FnOnce(&mut Document, &mut Vec<RawNodeId>) -> Result<Node, EditError>,
    ) -> Result<NodeId, EditError> {
        let mode = self.mode;
        self.doc.check_writable()?;
        self.doc
            .transact(|doc, staged| {
                let node = build(doc, staged)?;
                doc.finish(node, mode)
            })
            .map_err(|error| error.under("detached"))
    }
    pub fn create_char(&mut self, value: char) -> Result<NodeId, EditError> {
        self.build(|_, _| Ok(Node::Char(value)))
    }
    pub fn create_text(&mut self, value: impl Into<String>) -> Result<NodeId, EditError> {
        let value = value.into();
        self.build(|_, _| Ok(Node::Text(value)))
    }
    pub fn create_active_space(&mut self) -> Result<NodeId, EditError> {
        self.build(|_, _| Ok(Node::ActiveSpace))
    }
    pub fn create_alignment_tab(&mut self) -> Result<NodeId, EditError> {
        self.build(|_, _| Ok(Node::AlignmentTab))
    }
    pub fn create_prime(&mut self, count: usize) -> Result<NodeId, EditError> {
        self.build(|_, _| Ok(Node::Prime { count }))
    }
    fn group(
        &mut self,
        mode: ContentMode,
        kind: GroupKind,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.build(|doc, staged| {
            let children = doc.group_children(staged, children, mode)?;
            Ok(Node::Group {
                children,
                kind,
                mode,
            })
        })
    }
    pub fn create_group(
        &mut self,
        mode: ContentMode,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.group(mode, GroupKind::Explicit, children)
    }
    pub fn create_inline_math(
        &mut self,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.group(ContentMode::Math, GroupKind::InlineMath, children)
    }
    pub fn create_delimited_group(
        &mut self,
        left: DelimiterValue,
        right: DelimiterValue,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.group(
            ContentMode::Math,
            GroupKind::Delimited {
                left: left.into_ast(),
                right: right.into_ast(),
            },
            children,
        )
    }
    pub fn create_scripted(
        &mut self,
        base: impl Into<Arg>,
        sub: Option<Arg>,
        sup: Option<Arg>,
    ) -> Result<NodeId, EditError> {
        let base = base.into();
        self.build(|doc, staged| {
            let mut script = |value: Option<Arg>| {
                value
                    .map(|value| doc.content(staged, value, ContentMode::Math))
                    .transpose()
            };
            let (subscript, superscript) = (script(sub)?, script(sup)?);
            Ok(Node::Scripted {
                base: doc.content(staged, base, ContentMode::Math)?,
                subscript,
                superscript,
            })
        })
    }
    pub fn create_command(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.command(CommandKind::Prefix, name.into(), args, None)
    }
    pub fn create_declarative(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.command(CommandKind::Declarative, name.into(), args, None)
    }
    pub fn create_infix(
        &mut self,
        name: impl Into<String>,
        left: impl Into<Arg>,
        right: impl Into<Arg>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        let operands = Some((left.into(), right.into()));
        self.command(CommandKind::Infix, name.into(), args, operands)
    }
    fn command(
        &mut self,
        kind: CommandKind,
        name: String,
        values: impl IntoIterator<Item = Arg>,
        operands: Option<(Arg, Arg)>,
    ) -> Result<NodeId, EditError> {
        let mode = self.mode;
        let values = values.into_iter().collect::<Vec<_>>();
        self.build(|doc, staged| {
            let record = doc
                .knowledge_base
                .lookup_command(&name, mode)
                .map(|record| (record.kind, record.argspec));
            let args = match record {
                Some((expected, signature)) => {
                    // A constructor mismatch would otherwise surface as a
                    // less helpful signature error.
                    conformance::check_command_kind(kind, expected)?;
                    doc.arguments(staged, values, signature)?
                }
                // Unknown names have no signature; placeholder slots let the
                // local check reject any arguments.
                None => vec![None; values.len()],
            };
            Ok(match kind {
                CommandKind::Prefix => Node::Command {
                    known: conformance::command_known(&doc.knowledge_base, &name, mode),
                    name,
                    args,
                },
                CommandKind::Declarative => Node::Declarative { name, args },
                CommandKind::Infix => {
                    let (left, right) = operands.expect("infix construction supplies operands");
                    Node::Infix {
                        left: doc.content(staged, left, ContentMode::Math)?,
                        right: doc.content(staged, right, ContentMode::Math)?,
                        name,
                        args,
                    }
                }
            })
        })
    }
    pub fn create_environment(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
        body: impl Into<Arg>,
    ) -> Result<NodeId, EditError> {
        // An absent body is an empty implicit body group.
        let body = Some(body.into()).filter(|body| *body != Arg::Absent);
        self.environment(name.into(), args.into_iter().collect(), body, Vec::new())
    }
    #[doc(hidden)]
    pub fn create_environment_with_children(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
        children: Vec<Arg>,
    ) -> Result<NodeId, EditError> {
        self.environment(name.into(), args.into_iter().collect(), None, children)
    }
    fn environment(
        &mut self,
        name: String,
        values: Vec<Arg>,
        body: Option<Arg>,
        children: Vec<Arg>,
    ) -> Result<NodeId, EditError> {
        let mode = self.mode;
        self.doc.check_writable()?;
        self.doc
            .transact(|doc, staged| {
                let record = doc
                    .knowledge_base
                    .lookup_env(&name, mode)
                    .map(|record| (record.argspec, record.body_mode));
                let body_mode = record.map_or(mode, |(_, body_mode)| body_mode);
                let args = match record {
                    Some((signature, _)) => doc.arguments(staged, values, signature)?,
                    None => vec![None; values.len()],
                };
                let mut children_body = None;
                let body = match body {
                    Some(body) => doc.content(staged, body, body_mode)?,
                    None => {
                        let children = doc.group_children(staged, children, body_mode)?;
                        let contents = args.iter().flatten().filter_map(|arg| arg.value.content());
                        check_unique(contents.map(|(id, _)| id).chain(children.iter().copied()))?;
                        let group = Node::Group {
                            children,
                            mode: body_mode,
                            kind: GroupKind::Implicit,
                        };
                        doc.check_local(&group, body_mode, None)
                            .map_err(|error| error.under("body"))?;
                        // An empty stand-in lets the environment be checked
                        // before the body adopts any input node.
                        let body = doc.ast.new_node(Node::Group {
                            children: Vec::new(),
                            mode: body_mode,
                            kind: GroupKind::Implicit,
                        });
                        doc.detached_modes.insert(body, body_mode);
                        staged.push(body);
                        children_body = Some((body, group));
                        body
                    }
                };
                let node = Node::Environment {
                    name,
                    args,
                    known: record.is_some(),
                    body,
                };
                let id = doc.finish(node, mode)?;
                if let Some((body, group)) = children_body {
                    for (child, _) in Ast::node_edges(&group) {
                        doc.detached_modes.remove(child);
                    }
                    doc.ast.replace_node(body, group);
                    doc.debug_assert_conformance();
                }
                Ok(id)
            })
            .map_err(|error| error.under("detached"))
    }
    pub fn parse_fragment(&mut self, source: &str) -> Result<NodeId, EditError> {
        self.doc.parse_fragment(source, Some(self.mode))
    }
}

impl Document {
    /// Create a detached char node.
    pub fn create_char(&mut self, value: char) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode()).create_char(value)
    }
    /// Create a detached text node.
    pub fn create_text(&mut self, value: impl Into<String>) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode()).create_text(value)
    }
    /// Create a detached active space node.
    pub fn create_active_space(&mut self) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode()).create_active_space()
    }
    /// Create a detached alignment tab node.
    pub fn create_alignment_tab(&mut self) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode()).create_alignment_tab()
    }
    /// Create a detached prime node.
    pub fn create_prime(&mut self, count: usize) -> Result<NodeId, EditError> {
        self.in_mode(ContentMode::Math).create_prime(count)
    }
    /// Create a detached group node.
    pub fn create_group(
        &mut self,
        mode: ContentMode,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(mode).create_group(mode, children)
    }
    /// Create a detached inline math node.
    pub fn create_inline_math(
        &mut self,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(ContentMode::Text).create_inline_math(children)
    }
    /// Create a detached delimited group node.
    pub fn create_delimited_group(
        &mut self,
        left: DelimiterValue,
        right: DelimiterValue,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(ContentMode::Math)
            .create_delimited_group(left, right, children)
    }
    /// Create a detached scripted node.
    pub fn create_scripted(
        &mut self,
        base: impl Into<Arg>,
        sub: Option<Arg>,
        sup: Option<Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(ContentMode::Math)
            .create_scripted(base, sub, sup)
    }
    /// Create a detached command node.
    pub fn create_command(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode()).create_command(name, args)
    }
    /// Create a detached declarative node.
    pub fn create_declarative(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode())
            .create_declarative(name, args)
    }
    /// Create a detached infix node.
    pub fn create_infix(
        &mut self,
        name: impl Into<String>,
        left: impl Into<Arg>,
        right: impl Into<Arg>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(ContentMode::Math)
            .create_infix(name, left, right, args)
    }
    /// Create a detached environment node.
    pub fn create_environment(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
        body: impl Into<Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode())
            .create_environment(name, args, body)
    }
    /// Create a detached environment with children node.
    pub fn create_environment_with_children(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
        children: Vec<Arg>,
    ) -> Result<NodeId, EditError> {
        self.in_mode(self.root_mode())
            .create_environment_with_children(name, args, children)
    }
}
