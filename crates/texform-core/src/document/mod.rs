//! Public, fallible DOM layer over the internal panic-contract [`Ast`].
//!
//! [`Document`] is the single public, editable tree entry point. It wraps an
//! internal [`crate::ast::Ast`], exposes read access through [`NodeRef`]
//! handles, and edits through fallible methods returning [`EditError`] -- no
//! panic from the `Ast` layer ever reaches a `Document` caller on
//! user-input-driven paths.
//!
//! # Structural validity vs semantic completeness
//!
//! The wrapped `Ast` is always structurally valid. Whether the tree contains
//! [`crate::ast::Node::Error`] placeholders is a separate, O(1) property
//! exposed by [`Document::has_errors`]. A `Document` produced from a partial
//! parse (containing `Error` nodes) is read-only.

pub(crate) mod conformance;
mod construct;
mod path;
mod table;
pub use conformance::{ConformanceError, ConformanceRule};
pub use construct::{Arg, InMode, parse_char};
pub use table::{ArgumentTable, ColumnarTree, NodeTable};
mod edit;
mod import;
pub use path::NodeSlot;
#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU64, Ordering};

use slotmap::SecondaryMap;
use texform_interface::syntax_node::{self, SyntaxNode};

pub use crate::ast::NodeKind;
use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind,
    Node, NodeId as RawNodeId, ParentLink, Slot,
};
use crate::parse::grammar::SpanTree;
use crate::parse::{KnowledgeBase, ParseDiagnostic, Span};
use crate::serialize::{
    SerializeError, SerializeOptions, TokenizedLatex, serialize, serialize_tokenized,
    serialize_tokenized_with, serialize_with,
};

/// Process-wide unique identity for a [`Document`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocumentId(u64);

static NEXT_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

fn next_document_id() -> DocumentId {
    DocumentId(NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed))
}

/// Public node handle.
///
/// This is intentionally not the raw arena key: it carries the owning
/// [`DocumentId`] so core can reject cross-document node mixing before touching
/// the arena. Users can copy it but cannot construct one directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    document: DocumentId,
    raw: RawNodeId,
}

impl NodeId {
    fn new(document: DocumentId, raw: RawNodeId) -> Self {
        Self { document, raw }
    }

    fn raw(self) -> RawNodeId {
        self.raw
    }

    fn document(self) -> DocumentId {
        self.document
    }
}

/// Error from [`Document::from_syntax`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FromSyntaxError {
    /// The provided `SyntaxNode` was not a `Root`.
    NotARoot,
    /// The syntax violates the document knowledge or representation rules.
    Conformance(ConformanceError),
}

impl std::fmt::Display for FromSyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FromSyntaxError::NotARoot => f.write_str("expected a SyntaxNode::Root"),
            FromSyntaxError::Conformance(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FromSyntaxError {}

impl FromSyntaxError {
    fn under(self, base: &str) -> Self {
        match self {
            Self::Conformance(error) => Self::Conformance(error.under(base)),
            error => error,
        }
    }
}

/// Fallible editing error from [`Document`] mutation APIs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// The referenced node does not exist in this document.
    NodeNotFound,
    /// The document is read-only because it contains `Error` nodes, so no
    /// mutation is allowed.
    ReadOnlyDocument,
    /// The edit targets the root node, which cannot be detached or replaced.
    CannotEditRoot,
    /// The target node cannot hold ordered children, so child operations
    /// (append, insert) do not apply.
    NotAContainer,
    /// A node was supplied for a typed slot whose required shape it does not
    /// match; `expected` names the shape the slot demands.
    SlotShapeMismatch { expected: &'static str },
    /// The edit would attach a node into its own subtree, forming a cycle.
    WouldCreateCycle,
    /// A child index lies outside the valid range for the container.
    IndexOutOfBounds,
    /// The same node would appear more than once in the tree.
    DuplicateChild,
    /// The operation expected a staged, detached subtree root but received an
    /// already-attached node.
    ExpectedDetachedRoot,
    /// The node belongs to a different document; cross-document edits are
    /// rejected before they can corrupt either tree.
    ForeignNode,
    /// The proposed shape violates the document knowledge or representation rules.
    Conformance(ConformanceError),
    /// A source fragment could not be parsed without diagnostics.
    InvalidSource(Vec<ParseDiagnostic>),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::NodeNotFound => f.write_str("node not found"),
            EditError::ReadOnlyDocument => f.write_str("document is read-only"),
            EditError::CannotEditRoot => f.write_str("cannot edit the root node"),
            EditError::NotAContainer => f.write_str("node is not a container"),
            EditError::SlotShapeMismatch { expected } => {
                write!(f, "slot shape mismatch: expected {expected}")
            }
            EditError::WouldCreateCycle => f.write_str("edit would create a cycle"),
            EditError::IndexOutOfBounds => f.write_str("index out of bounds"),
            EditError::DuplicateChild => f.write_str("node cannot appear more than once"),
            EditError::ExpectedDetachedRoot => f.write_str("expected a detached root"),
            EditError::ForeignNode => f.write_str("node belongs to a different document"),
            EditError::Conformance(error) => error.fmt(f),
            EditError::InvalidSource(_) => f.write_str("source fragment contains parse errors"),
        }
    }
}

impl std::error::Error for EditError {}

impl From<ConformanceError> for EditError {
    fn from(error: ConformanceError) -> Self {
        Self::Conformance(error)
    }
}

impl EditError {
    /// Prefix a relative conformance path; other errors carry no path.
    fn under(self, base: &str) -> Self {
        match self {
            Self::Conformance(error) => Self::Conformance(error.under(base)),
            error => error,
        }
    }
}

/// Public write-side delimiter value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DelimiterValue {
    /// No delimiter, corresponding to `.` in LaTeX.
    None,
    /// Single-character delimiter such as `(`, `)`, or `|`.
    Char(char),
    /// Control-sequence delimiter such as `\langle`, without the backslash.
    Control(String),
}

impl DelimiterValue {
    fn into_ast(self) -> Delimiter {
        match self {
            DelimiterValue::None => Delimiter::None,
            DelimiterValue::Char(ch) => Delimiter::Char(ch),
            DelimiterValue::Control(name) => Delimiter::Control(name),
        }
    }
}

/// Public, fallible, editable DOM over an internal [`Ast`].
///
/// Every document shares the immutable knowledge base defining its commands.
pub struct Document {
    ast: Ast,
    spans: SecondaryMap<RawNodeId, Span>,
    has_errors: bool,
    id: DocumentId,
    knowledge_base: KnowledgeBase,
    detached_modes: SecondaryMap<RawNodeId, ContentMode>,
}

impl Document {
    /// Create an empty document containing only an empty math-mode root.
    ///
    /// The document shares the process-wide default knowledge base.
    pub fn new() -> Self {
        Self::with_mode(ContentMode::Math)
    }

    /// Like [`Document::new`] but with an explicit root content mode.
    ///
    /// The document shares the process-wide default knowledge base.
    pub fn with_mode(mode: ContentMode) -> Self {
        Self::with_knowledge_base(&KnowledgeBase::default(), mode)
    }

    /// Create an empty document sharing the supplied knowledge base.
    pub fn with_knowledge_base(knowledge_base: &KnowledgeBase, mode: ContentMode) -> Self {
        Document {
            ast: Ast::with_root_mode(mode),
            spans: SecondaryMap::new(),
            has_errors: false,
            id: next_document_id(),
            knowledge_base: knowledge_base.clone(),
            detached_modes: SecondaryMap::new(),
        }
    }

    /// Build a document from syntax using the default knowledge base.
    pub fn from_syntax(node: &SyntaxNode) -> Result<Document, FromSyntaxError> {
        Self::from_syntax_with(&KnowledgeBase::default(), node)
    }

    /// Build a document from syntax sharing the supplied knowledge base.
    pub fn from_syntax_with(
        knowledge_base: &KnowledgeBase,
        node: &SyntaxNode,
    ) -> Result<Document, FromSyntaxError> {
        let ast = Self::structural_ast(node)?;
        let has_errors = ast.contains_error();
        let document = Document {
            ast,
            spans: SecondaryMap::new(),
            has_errors,
            id: next_document_id(),
            knowledge_base: knowledge_base.clone(),
            detached_modes: SecondaryMap::new(),
        };
        if !document.has_errors {
            document
                .__validate_conformance()
                .map_err(FromSyntaxError::Conformance)?;
        }
        Ok(document)
    }

    /// Serialize a syntax root after structural checks only.
    ///
    /// Unlike [`Document::from_syntax`], this needs no knowledge base and does
    /// not check knowledge conformance.
    pub fn serialize_syntax(
        node: &SyntaxNode,
        options: &SerializeOptions,
    ) -> Result<String, FromSyntaxError> {
        Ok(serialize_with(&Self::structural_ast(node)?, options))
    }

    /// Check the knowledge-independent shape of a syntax root and build its arena.
    fn structural_ast(node: &SyntaxNode) -> Result<Ast, FromSyntaxError> {
        let SyntaxNode::Root { children, .. } = node else {
            return Err(FromSyntaxError::NotARoot);
        };
        for (index, child) in children.iter().enumerate() {
            Self::validate_syntax(child)
                .map_err(|error| error.under(&format!("root.{}", NodeSlot::Child(index))))?;
        }
        Ok(Ast::from_syntax_root(node))
    }

    /// Internal: build from syntax plus the parser's positional span subtree.
    pub(crate) fn from_syntax_with_spans(
        knowledge_base: &KnowledgeBase,
        node: &SyntaxNode,
        span_tree: &SpanTree,
    ) -> Document {
        // Parser output is audited separately; do not add a second tree walk here.
        let ast = Ast::from_syntax_root(node);
        let mut doc = Document {
            has_errors: ast.contains_error(),
            ast,
            spans: SecondaryMap::new(),
            id: next_document_id(),
            knowledge_base: knowledge_base.clone(),
            detached_modes: SecondaryMap::new(),
        };
        let mut spans = SecondaryMap::new();
        Self::assign_spans(&doc.ast, node, doc.ast.root(), span_tree, &mut spans);
        doc.spans = spans;
        doc
    }

    fn root_mode(&self) -> ContentMode {
        self.ast
            .node(self.ast.root())
            .content_mode()
            .expect("the arena root is a Root")
    }

    /// Process-wide unique id of this document.
    pub fn id(&self) -> DocumentId {
        self.id
    }

    /// Knowledge base shared by this document and its clones.
    pub fn knowledge_base(&self) -> &KnowledgeBase {
        &self.knowledge_base
    }

    /// Root node handle.
    pub fn root(&self) -> NodeRef<'_> {
        NodeRef {
            doc: self,
            raw: self.ast.root(),
        }
    }

    /// Return a read-only handle for a public id.
    pub fn node(&self, id: NodeId) -> Result<NodeRef<'_>, EditError> {
        let raw = self.check_node_owner(id)?;
        Ok(NodeRef { doc: self, raw })
    }

    /// `true` when the tree contains one or more `Error` nodes.
    pub fn has_errors(&self) -> bool {
        self.has_errors
    }

    /// Iterate every `Error` node in the tree.
    pub fn errors(&self) -> impl Iterator<Item = NodeRef<'_>> + '_ {
        self.ast
            .find_all(self.ast.root(), |node| matches!(node, Node::Error { .. }))
            .into_iter()
            .map(move |raw| NodeRef { doc: self, raw })
    }

    /// `true` when this document is read-only.
    pub fn is_read_only(&self) -> bool {
        self.has_errors
    }

    /// Find the first matching node under `start`, including `start`.
    pub fn find<'a>(
        &'a self,
        start: NodeRef<'a>,
        pred: impl Fn(NodeRef<'a>) -> bool + 'a,
    ) -> Option<NodeRef<'a>> {
        if start.doc.id != self.id || !self.ast.contains(start.raw) {
            return None;
        }
        self.ast
            .find_all(start.raw, |_| true)
            .into_iter()
            .map(|raw| NodeRef { doc: self, raw })
            .find(|node| pred(*node))
    }

    /// Collect matching nodes under `start`, including `start`.
    pub fn find_all<'a>(
        &'a self,
        start: NodeRef<'a>,
        pred: impl Fn(NodeRef<'a>) -> bool + 'a,
    ) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let raws = if start.doc.id == self.id && self.ast.contains(start.raw) {
            self.ast.find_all(start.raw, |_| true)
        } else {
            Vec::new()
        };
        raws.into_iter().filter_map(move |raw| {
            let node = NodeRef { doc: self, raw };
            pred(node).then_some(node)
        })
    }

    /// Find commands by name.
    pub fn find_commands<'a>(&'a self, name: &'a str) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.find_all(self.root(), move |node| node.is_command(name))
    }

    /// Find environments by name.
    pub fn find_environments<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.find_all(self.root(), move |node| node.env_name() == Some(name))
    }

    /// Convert this document back into a lossless [`SyntaxNode`] tree.
    pub fn to_syntax(&self) -> SyntaxNode {
        self.ast.to_syntax_root()
    }

    /// Internal bridge for the `texform` facade engine integration.
    ///
    /// This is not a stable public editing API. It bypasses fallible document
    /// editing and must only be called after a `!has_errors()` gate.
    #[doc(hidden)]
    pub fn __texform_engine_ast_mut(&mut self) -> &mut Ast {
        &mut self.ast
    }

    /// Serialize to LaTeX using the default canonical style.
    pub fn to_latex(&self) -> Result<String, SerializeError> {
        Ok(serialize(&self.ast))
    }

    /// Serialize to LaTeX with explicit style options.
    pub fn to_latex_with(&self, options: &SerializeOptions) -> Result<String, SerializeError> {
        Ok(serialize_with(&self.ast, options))
    }

    /// Serialize to canonical LaTeX and record typed tokens from the same traversal.
    ///
    /// Token spans are UTF-8 byte offsets into the returned LaTeX. An empty error
    /// snippet does not produce a token; use [`Self::has_errors`] to detect error nodes.
    pub fn to_tokenized_latex(&self) -> Result<TokenizedLatex, SerializeError> {
        Ok(serialize_tokenized(&self.ast))
    }

    /// Serialize with explicit options and record typed tokens from the same traversal.
    ///
    /// Token spans are UTF-8 byte offsets into the returned LaTeX. An empty error
    /// snippet does not produce a token; use [`Self::has_errors`] to detect error nodes.
    pub fn to_tokenized_latex_with(
        &self,
        options: &SerializeOptions,
    ) -> Result<TokenizedLatex, SerializeError> {
        Ok(serialize_tokenized_with(&self.ast, options))
    }

    fn child_len(&self, parent: NodeId) -> Result<usize, EditError> {
        let raw = self.check_node_owner(parent)?;
        self.check_container(raw)?;
        Ok(self.ast.children(raw).len())
    }

    fn check_writable(&self) -> Result<(), EditError> {
        if self.has_errors {
            Err(EditError::ReadOnlyDocument)
        } else {
            Ok(())
        }
    }

    fn check_node_owner(&self, id: NodeId) -> Result<RawNodeId, EditError> {
        if id.document() != self.id {
            return Err(EditError::ForeignNode);
        }
        let raw = id.raw();
        if self.ast.contains(raw) {
            Ok(raw)
        } else {
            Err(EditError::NodeNotFound)
        }
    }

    fn check_container(&self, id: RawNodeId) -> Result<(), EditError> {
        match self.ast.node_opt(id) {
            Some(Node::Root { .. }) | Some(Node::Group { .. }) => Ok(()),
            Some(_) => Err(EditError::NotAContainer),
            None => Err(EditError::NodeNotFound),
        }
    }

    fn check_detached(&self, id: RawNodeId) -> Result<(), EditError> {
        if !self.ast.contains(id) {
            return Err(EditError::NodeNotFound);
        }
        if id == self.ast.root() {
            return Err(EditError::CannotEditRoot);
        }
        if self.ast.parent_id(id).is_some() || !self.ast.is_detached_root(id) {
            return Err(EditError::ExpectedDetachedRoot);
        }
        Ok(())
    }

    fn check_no_cycle(&self, child: RawNodeId, new_parent: RawNodeId) -> Result<(), EditError> {
        let mut current = Some(new_parent);
        while let Some(id) = current {
            if id == child {
                return Err(EditError::WouldCreateCycle);
            }
            current = self.ast.parent_id(id);
        }
        Ok(())
    }

    /// Reject syntax the arena cannot hold, with a path relative to `node`.
    ///
    /// These structural checks also run for incomplete input, which skips the
    /// conformance check.
    fn validate_syntax(node: &SyntaxNode) -> Result<(), FromSyntaxError> {
        let nested = |child: &SyntaxNode, slot: NodeSlot| {
            Self::validate_syntax(child).map_err(|error| error.under(&slot.to_string()))
        };
        let args = |args: &[syntax_node::ArgumentSlot]| {
            for (index, arg) in args.iter().enumerate() {
                if let Some(
                    syntax_node::ArgumentValue::MathContent(child)
                    | syntax_node::ArgumentValue::TextContent(child)
                    | syntax_node::ArgumentValue::OperatorNameContent(child),
                ) = arg.as_ref().map(|arg| &arg.value)
                {
                    nested(child, NodeSlot::Arg(index))?;
                }
            }
            Ok(())
        };
        match node {
            SyntaxNode::Root { .. } => return Err(FromSyntaxError::NotARoot),
            SyntaxNode::Group { children, .. } => {
                for (index, child) in children.iter().enumerate() {
                    nested(child, NodeSlot::Child(index))?;
                }
            }
            SyntaxNode::Command { args: slots, .. }
            | SyntaxNode::Declarative { args: slots, .. } => args(slots)?,
            SyntaxNode::Infix {
                args: slots,
                left,
                right,
                ..
            } => {
                nested(left, NodeSlot::InfixLeft)?;
                args(slots)?;
                nested(right, NodeSlot::InfixRight)?;
            }
            SyntaxNode::Environment {
                args: slots, body, ..
            } => {
                if !matches!(body.as_ref(), SyntaxNode::Group { .. }) {
                    return Err(FromSyntaxError::Conformance(ConformanceError::new(
                        ConformanceRule::EnvironmentBodyMode,
                        "environment body must be a group",
                    )));
                }
                args(slots)?;
                nested(body, NodeSlot::EnvBody)?;
            }
            SyntaxNode::Scripted {
                base,
                subscript,
                superscript,
            } => {
                nested(base, NodeSlot::ScriptBase)?;
                if let Some(subscript) = subscript {
                    nested(subscript, NodeSlot::Subscript)?;
                }
                if let Some(superscript) = superscript {
                    nested(superscript, NodeSlot::Superscript)?;
                }
            }
            SyntaxNode::Prime { count } => {
                conformance::check_prime_count(*count).map_err(FromSyntaxError::Conformance)?
            }
            SyntaxNode::Error { .. }
            | SyntaxNode::Text(_)
            | SyntaxNode::Char(_)
            | SyntaxNode::ActiveSpace
            | SyntaxNode::AlignmentTab => {}
        }
        Ok(())
    }

    /// Parent and index of an attached group child.
    fn group_child_position(&self, raw: RawNodeId) -> Result<(RawNodeId, usize), EditError> {
        match self.ast.parent(raw) {
            Some(ParentLink {
                parent,
                slot: Slot::GroupChild(index),
            }) => Ok((parent, index)),
            Some(_) => Err(EditError::SlotShapeMismatch {
                expected: "group child",
            }),
            None => Err(EditError::NodeNotFound),
        }
    }

    /// Export the parse-time span side table as `(path, span)` pairs.
    ///
    /// Paths follow the parser's tree-path scheme rooted at `root`:
    /// `.child.N` for container children, `.arg.N.content` for content-carrying
    /// argument slots, `.left` / `.right` for infix operands, `.body` for
    /// environment bodies, and `.base` / `.sub` / `.sup` for script slots.
    /// Nodes without a recorded span (e.g. created by edits, or any node of a
    /// document built without parser spans) are omitted. Spans reflect the
    /// original parse and are not updated by document edits.
    pub fn node_spans(&self) -> Vec<(String, Span)> {
        let mut out = Vec::new();
        self.collect_node_spans(self.ast.root(), "root", &mut out);
        out
    }

    fn collect_node_spans(&self, id: RawNodeId, path: &str, out: &mut Vec<(String, Span)>) {
        if let Some(span) = self.spans.get(id) {
            out.push((path.to_string(), span.clone()));
        }
        for (child, slot) in self.ast.edges(id) {
            self.collect_node_spans(child, &format!("{path}.{}", NodeSlot::from(slot)), out);
        }
    }

    /// Attach parser spans by walking the syntax tree, the arena, and the
    /// positional [`SpanTree`] in lockstep. The span subtree mirrors the syntax
    /// structure, so each node reads its span directly and recurses into the
    /// child subtrees in the order documented on [`SpanTree`] — no path strings
    /// and no hashing.
    #[allow(dead_code)]
    fn assign_spans(
        ast: &Ast,
        syntax: &SyntaxNode,
        id: RawNodeId,
        span_tree: &SpanTree,
        out: &mut SecondaryMap<RawNodeId, Span>,
    ) {
        out.insert(
            id,
            Span {
                start: span_tree.span.start,
                end: span_tree.span.end,
            },
        );

        match (syntax, ast.node(id)) {
            (
                SyntaxNode::Root {
                    children: syntax_children,
                    ..
                },
                Node::Root { children, .. },
            )
            | (
                SyntaxNode::Group {
                    children: syntax_children,
                    ..
                },
                Node::Group { children, .. },
            ) => {
                for (index, (syntax_child, ast_child)) in
                    syntax_children.iter().zip(children.iter()).enumerate()
                {
                    if let Some(kid) = span_tree.kids.get(index) {
                        Self::assign_spans(ast, syntax_child, *ast_child, kid, out);
                    }
                }
            }
            (
                SyntaxNode::Command {
                    args: syntax_args, ..
                },
                Node::Command { args: ast_args, .. },
            )
            | (
                SyntaxNode::Declarative {
                    args: syntax_args, ..
                },
                Node::Declarative { args: ast_args, .. },
            ) => {
                let mut cursor = 0;
                Self::assign_arg_spans(
                    ast,
                    syntax_args,
                    ast_args,
                    &span_tree.kids,
                    &mut cursor,
                    out,
                );
            }
            (
                SyntaxNode::Infix {
                    args: syntax_args,
                    left: syntax_left,
                    right: syntax_right,
                    ..
                },
                Node::Infix {
                    args: ast_args,
                    left,
                    right,
                    ..
                },
            ) => {
                let mut cursor = 0;
                if let Some(kid) = span_tree.kids.get(cursor) {
                    Self::assign_spans(ast, syntax_left, *left, kid, out);
                    cursor += 1;
                }
                Self::assign_arg_spans(
                    ast,
                    syntax_args,
                    ast_args,
                    &span_tree.kids,
                    &mut cursor,
                    out,
                );
                if let Some(kid) = span_tree.kids.get(cursor) {
                    Self::assign_spans(ast, syntax_right, *right, kid, out);
                }
            }
            (
                SyntaxNode::Environment {
                    args: syntax_args,
                    body: syntax_body,
                    ..
                },
                Node::Environment {
                    args: ast_args,
                    body,
                    ..
                },
            ) => {
                let mut cursor = 0;
                Self::assign_arg_spans(
                    ast,
                    syntax_args,
                    ast_args,
                    &span_tree.kids,
                    &mut cursor,
                    out,
                );
                if let Some(kid) = span_tree.kids.get(cursor) {
                    Self::assign_spans(ast, syntax_body, *body, kid, out);
                }
            }
            (
                SyntaxNode::Scripted {
                    base: syntax_base,
                    subscript: syntax_subscript,
                    superscript: syntax_superscript,
                },
                Node::Scripted {
                    base,
                    subscript,
                    superscript,
                },
            ) => {
                let mut cursor = 0;
                if let Some(kid) = span_tree.kids.get(cursor) {
                    Self::assign_spans(ast, syntax_base, *base, kid, out);
                    cursor += 1;
                }
                if let (Some(syntax), Some(ast_id)) = (syntax_subscript, subscript)
                    && let Some(kid) = span_tree.kids.get(cursor)
                {
                    Self::assign_spans(ast, syntax, *ast_id, kid, out);
                    cursor += 1;
                }
                if let (Some(syntax), Some(ast_id)) = (syntax_superscript, superscript)
                    && let Some(kid) = span_tree.kids.get(cursor)
                {
                    Self::assign_spans(ast, syntax, *ast_id, kid, out);
                }
            }
            _ => {}
        }
    }

    /// Recurse into each content-bearing argument slot, consuming one span
    /// subtree from `kids` per slot via `cursor`. The producer pushes exactly
    /// one subtree per content slot in slot order, so the cursor stays aligned.
    fn assign_arg_spans(
        ast: &Ast,
        syntax_args: &[syntax_node::ArgumentSlot],
        ast_args: &[ArgumentSlot],
        kids: &[SpanTree],
        cursor: &mut usize,
        out: &mut SecondaryMap<RawNodeId, Span>,
    ) {
        for (syntax_slot, ast_slot) in syntax_args.iter().zip(ast_args.iter()) {
            let (Some(syntax_arg), Some(ast_arg)) = (syntax_slot, ast_slot) else {
                continue;
            };
            if let (
                syntax_node::ArgumentValue::MathContent(syntax_node)
                | syntax_node::ArgumentValue::TextContent(syntax_node)
                | syntax_node::ArgumentValue::OperatorNameContent(syntax_node),
                ArgumentValue::MathContent(ast_id)
                | ArgumentValue::TextContent(ast_id)
                | ArgumentValue::OperatorNameContent(ast_id),
            ) = (&syntax_arg.value, &ast_arg.value)
                && let Some(kid) = kids.get(*cursor)
            {
                Self::assign_spans(ast, syntax_node, *ast_id, kid, out);
                *cursor += 1;
            }
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Document {
    fn clone(&self) -> Self {
        Document {
            ast: self.ast.clone(),
            spans: self.spans.clone(),
            has_errors: self.has_errors,
            id: next_document_id(),
            knowledge_base: self.knowledge_base.clone(),
            detached_modes: self.detached_modes.clone(),
        }
    }
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("id", &self.id)
            .field("root", &self.ast.root())
            .field("has_errors", &self.has_errors)
            .finish()
    }
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let latex = self.to_latex().map_err(|_| std::fmt::Error)?;
        f.write_str(&latex)
    }
}

/// Read-only borrowed handle to a node within a [`Document`].
#[derive(Clone, Copy)]
pub struct NodeRef<'a> {
    doc: &'a Document,
    raw: RawNodeId,
}

impl std::fmt::Debug for NodeRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeRef").field("id", &self.id()).finish()
    }
}

impl<'a> NodeRef<'a> {
    /// Opaque public handle for this node.
    pub fn id(&self) -> NodeId {
        NodeId::new(self.doc.id, self.raw)
    }

    /// Lightweight node discriminant.
    pub fn kind(&self) -> NodeKind {
        self.doc.ast.kind(self.raw)
    }

    /// `true` when this is a command named `name`.
    pub fn is_command(&self, name: &str) -> bool {
        matches!(self.node(), Node::Command { name: current, .. } if current == name)
    }

    /// `true` when this is a char node equal to `c`.
    pub fn is_char(&self, c: char) -> bool {
        matches!(self.node(), Node::Char(ch) if *ch == c)
    }

    /// `true` when this is an `Error` placeholder.
    pub fn is_error(&self) -> bool {
        matches!(self.node(), Node::Error { .. })
    }

    /// Parent handle, or `None` for the root / a detached node.
    pub fn parent(&self) -> Option<NodeRef<'a>> {
        self.doc
            .ast
            .parent_id(self.raw)
            .map(|raw| self.sibling(raw))
    }

    /// Direct children (root/group only; other kinds yield an empty iterator).
    pub fn children(&self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let doc = self.doc;
        doc.ast
            .children(self.raw)
            .to_vec()
            .into_iter()
            .map(move |raw| NodeRef { doc, raw })
    }

    /// Next sibling when attached as a group child.
    pub fn next_sibling(&self) -> Option<NodeRef<'a>> {
        self.doc
            .ast
            .next_sibling(self.raw)
            .map(|raw| self.sibling(raw))
    }

    /// Previous sibling when attached as a group child.
    pub fn prev_sibling(&self) -> Option<NodeRef<'a>> {
        self.doc
            .ast
            .prev_sibling(self.raw)
            .map(|raw| self.sibling(raw))
    }

    /// Ancestors from immediate parent up to the root.
    pub fn ancestors(&self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let doc = self.doc;
        let mut current = doc.ast.parent_id(self.raw);
        std::iter::from_fn(move || {
            let raw = current?;
            current = doc.ast.parent_id(raw);
            Some(NodeRef { doc, raw })
        })
    }

    /// All descendants in depth-first order, excluding self.
    pub fn descendants(&self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let doc = self.doc;
        let start = self.raw;
        doc.ast
            .find_all(start, |_| true)
            .into_iter()
            .filter(move |raw| *raw != start)
            .map(move |raw| NodeRef { doc, raw })
    }

    /// Command/infix/declarative name without leading backslash.
    pub fn command_name(&self) -> Option<&'a str> {
        match self.node() {
            Node::Command { name, .. }
            | Node::Infix { name, .. }
            | Node::Declarative { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Environment name without `begin`/`end`.
    pub fn env_name(&self) -> Option<&'a str> {
        match self.node() {
            Node::Environment { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Text payload for a `Text` node.
    pub fn text(&self) -> Option<&'a str> {
        match self.node() {
            Node::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Character for a `Char` node.
    pub fn char(&self) -> Option<char> {
        match self.node() {
            Node::Char(ch) => Some(*ch),
            _ => None,
        }
    }

    /// Prime symbol count for a `Prime` node.
    pub fn prime_count(&self) -> Option<usize> {
        match self.node() {
            Node::Prime { count } => Some(*count),
            _ => None,
        }
    }

    /// Error message + snippet for an `Error` node.
    pub fn error_parts(&self) -> Option<(&'a str, &'a str)> {
        match self.node() {
            Node::Error { message, snippet } => Some((message, snippet)),
            _ => None,
        }
    }

    /// Content mode for root/group nodes.
    pub fn content_mode(&self) -> Option<ContentMode> {
        self.node().content_mode()
    }

    /// Group kind for group nodes.
    pub fn group_kind(&self) -> Option<GroupKindRef<'a>> {
        match self.node() {
            Node::Group { kind, .. } => Some(self.group_kind_ref(kind)),
            _ => None,
        }
    }

    /// Number of argument slots on a command-like node.
    pub fn arg_count(&self) -> usize {
        self.doc.ast.arg_slots(self.raw).len()
    }

    /// Argument at `index`.
    pub fn arg(&self, index: usize) -> Option<ArgRef<'a>> {
        let arg = self.doc.ast.arg_slots(self.raw).get(index)?.as_ref()?;
        Some(self.arg_ref(arg))
    }

    /// The syntactic form of a present argument slot.
    pub fn arg_kind(&self, index: usize) -> Option<ArgKindRef<'a>> {
        let arg = self.doc.ast.arg_slots(self.raw).get(index)?.as_ref()?;
        Some((&arg.kind).into())
    }

    /// All argument slots.
    pub fn arg_slots(&self) -> impl Iterator<Item = Option<ArgRef<'a>>> + 'a {
        let this = *self;
        (0..self.arg_count()).map(move |index| this.arg(index))
    }

    /// Scripted base.
    pub fn script_base(&self) -> Option<NodeRef<'a>> {
        match self.node() {
            Node::Scripted { base, .. } => Some(self.sibling(*base)),
            _ => None,
        }
    }

    /// Scripted subscript.
    pub fn subscript(&self) -> Option<NodeRef<'a>> {
        match self.node() {
            Node::Scripted { subscript, .. } => subscript.map(|raw| self.sibling(raw)),
            _ => None,
        }
    }

    /// Scripted superscript.
    pub fn superscript(&self) -> Option<NodeRef<'a>> {
        match self.node() {
            Node::Scripted { superscript, .. } => superscript.map(|raw| self.sibling(raw)),
            _ => None,
        }
    }

    /// Infix left operand.
    pub fn infix_left(&self) -> Option<NodeRef<'a>> {
        match self.node() {
            Node::Infix { left, .. } => Some(self.sibling(*left)),
            _ => None,
        }
    }

    /// Infix right operand.
    pub fn infix_right(&self) -> Option<NodeRef<'a>> {
        match self.node() {
            Node::Infix { right, .. } => Some(self.sibling(*right)),
            _ => None,
        }
    }

    /// Environment body group.
    pub fn env_body(&self) -> Option<NodeRef<'a>> {
        match self.node() {
            Node::Environment { body, .. } => Some(self.sibling(*body)),
            _ => None,
        }
    }

    /// Parse-time byte span, if known.
    pub fn span(&self) -> Option<Span> {
        self.doc.spans.get(self.raw).cloned()
    }

    fn node(&self) -> &'a Node {
        self.doc.ast.node(self.raw)
    }

    fn sibling(&self, raw: RawNodeId) -> NodeRef<'a> {
        NodeRef { doc: self.doc, raw }
    }

    fn arg_ref(&self, arg: &'a Argument) -> ArgRef<'a> {
        match &arg.value {
            ArgumentValue::MathContent(id) => ArgRef::Math(self.sibling(*id)),
            ArgumentValue::TextContent(id) => ArgRef::Text(self.sibling(*id)),
            ArgumentValue::OperatorNameContent(id) => ArgRef::OperatorName(self.sibling(*id)),
            ArgumentValue::Delimiter(delimiter) => ArgRef::Delimiter(delimiter.into()),
            ArgumentValue::CSName(value) => ArgRef::CSName(value),
            ArgumentValue::Dimension(value) => ArgRef::Dimension(value),
            ArgumentValue::Integer(value) => ArgRef::Integer(value),
            ArgumentValue::KeyVal(value) => ArgRef::KeyVal(value),
            ArgumentValue::Column(value) => ArgRef::Column(value),
            ArgumentValue::Boolean(value) => ArgRef::Boolean(*value),
        }
    }

    fn group_kind_ref(&self, kind: &'a GroupKind) -> GroupKindRef<'a> {
        match kind {
            GroupKind::Explicit => GroupKindRef::Explicit,
            GroupKind::Implicit => GroupKindRef::Implicit,
            GroupKind::Delimited { left, right } => GroupKindRef::Delimited {
                left: left.into(),
                right: right.into(),
            },
            GroupKind::InlineMath => GroupKindRef::InlineMath,
        }
    }
}

/// Knowledge flag of a command-like or environment node.
fn known_flag(node: &Node) -> Option<bool> {
    match node {
        Node::Command { known, .. } | Node::Environment { known, .. } => Some(*known),
        Node::Infix { .. } | Node::Declarative { .. } => Some(true),
        _ => None,
    }
}

/// Read-side view of a [`DelimiterValue`], borrowing the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelimiterRef<'a> {
    /// No delimiter, corresponding to `.` in LaTeX.
    None,
    /// Single-character delimiter such as `(`, `)`, or `|`.
    Char(char),
    /// Control-sequence delimiter such as `\langle`, without the backslash.
    Control(&'a str),
}

impl<'a> From<&'a Delimiter> for DelimiterRef<'a> {
    fn from(delimiter: &'a Delimiter) -> Self {
        match delimiter {
            Delimiter::None => Self::None,
            Delimiter::Char(ch) => Self::Char(*ch),
            Delimiter::Control(name) => Self::Control(name),
        }
    }
}

impl<'a> From<&'a DelimiterValue> for DelimiterRef<'a> {
    fn from(delimiter: &'a DelimiterValue) -> Self {
        match delimiter {
            DelimiterValue::None => Self::None,
            DelimiterValue::Char(ch) => Self::Char(*ch),
            DelimiterValue::Control(name) => Self::Control(name),
        }
    }
}

/// Writes the string form parsed by [`DelimiterValue`]'s `FromStr`: `.`, a
/// single character, or a backslash-prefixed control sequence.
impl std::fmt::Display for DelimiterRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("."),
            Self::Char(ch) => write!(f, "{ch}"),
            Self::Control(name) => write!(f, "\\{name}"),
        }
    }
}

/// Read-side view of a [`GroupKind`], borrowing the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKindRef<'a> {
    /// Author-written brace group `{ ... }`.
    Explicit,
    /// Synthesized group with no source braces, produced by parsing or
    /// normalization.
    Implicit,
    /// Delimited group such as `\left( ... \right)`, carrying its delimiter pair.
    Delimited {
        /// Opening delimiter of the group.
        left: DelimiterRef<'a>,
        /// Closing delimiter of the group.
        right: DelimiterRef<'a>,
    },
    /// Inline math segment inside text mode, written `$ ... $`.
    InlineMath,
}

/// Read-side view of one command/environment argument value.
#[derive(Clone, Copy, Debug)]
pub enum ArgRef<'a> {
    /// Operator-name math content.
    OperatorName(NodeRef<'a>),
    /// Math-mode content argument, borrowing its child subtree.
    Math(NodeRef<'a>),
    /// Text-mode content argument, borrowing its child subtree.
    Text(NodeRef<'a>),
    /// Delimiter argument such as the opener of a paired form.
    Delimiter(DelimiterRef<'a>),
    /// Control-sequence name argument, without the leading backslash.
    CSName(&'a str),
    /// Raw dimension argument, kept as its source text (e.g. `2pt`).
    Dimension(&'a str),
    /// Raw integer argument, kept as its source text.
    Integer(&'a str),
    /// Raw key-value argument, kept as its source text.
    KeyVal(&'a str),
    /// Column-specification argument, kept as its source text.
    Column(&'a str),
    /// Boolean argument, primarily backing a star slot.
    Boolean(bool),
}

impl<'a> ArgRef<'a> {
    pub fn as_node(self) -> Option<NodeRef<'a>> {
        match self {
            ArgRef::Math(node) | ArgRef::Text(node) | ArgRef::OperatorName(node) => Some(node),
            _ => None,
        }
    }
}

/// Borrowed syntax form of a present argument slot.
#[derive(Clone, Copy, Debug)]
pub enum ArgKindRef<'a> {
    Mandatory,
    Optional,
    Star,
    Group,
    Delimited {
        open: DelimiterRef<'a>,
        close: DelimiterRef<'a>,
    },
    Paired {
        open: DelimiterRef<'a>,
        close: DelimiterRef<'a>,
    },
}

impl ArgKindRef<'_> {
    /// The snake_case form name used by bindings and columnar exports.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Mandatory => "mandatory",
            Self::Optional => "optional",
            Self::Star => "star",
            Self::Group => "group",
            Self::Delimited { .. } => "delimited",
            Self::Paired { .. } => "paired",
        }
    }
}

impl<'a> From<&'a ArgumentKind> for ArgKindRef<'a> {
    fn from(kind: &'a ArgumentKind) -> Self {
        match kind {
            ArgumentKind::Mandatory => Self::Mandatory,
            ArgumentKind::Optional => Self::Optional,
            ArgumentKind::Star => Self::Star,
            ArgumentKind::Group => Self::Group,
            ArgumentKind::Delimited { open, close } => Self::Delimited {
                open: open.into(),
                close: close.into(),
            },
            ArgumentKind::Paired { open, close } => Self::Paired {
                open: open.into(),
                close: close.into(),
            },
        }
    }
}
