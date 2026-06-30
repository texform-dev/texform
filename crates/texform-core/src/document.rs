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

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use slotmap::SecondaryMap;
use texform_interface::syntax_node::{self, SyntaxNode};

pub use crate::ast::NodeKind;
use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind,
    Node, NodeId as RawNodeId, Slot,
};
use crate::parse::{ParseContextId, Span};
use crate::serialize::{SerializeError, SerializeOptions, serialize, serialize_with};

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
    /// A `Prime` node had `count == 0`.
    InvalidPrimeCount,
    /// A `Prime` node appeared in text-mode content.
    PrimeInTextMode,
}

impl std::fmt::Display for FromSyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FromSyntaxError::NotARoot => f.write_str("from_syntax expects a SyntaxNode::Root"),
            FromSyntaxError::InvalidPrimeCount => {
                f.write_str("Prime count must be greater than zero")
            }
            FromSyntaxError::PrimeInTextMode => {
                f.write_str("Prime nodes are only valid in math mode")
            }
        }
    }
}

impl std::error::Error for FromSyntaxError {}

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
        }
    }
}

impl std::error::Error for EditError {}

/// Write-side command/environment argument value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgValue {
    /// Math-mode content argument referencing a child subtree.
    Math(NodeId),
    /// Text-mode content argument referencing a child subtree.
    Text(NodeId),
    /// Delimiter argument such as the opener of a paired form.
    Delimiter(DelimiterValue),
    /// Control-sequence name argument, stored without the leading backslash.
    CSName(String),
    /// Raw dimension argument, kept as its source text (e.g. `2pt`).
    Dimension(String),
    /// Raw integer argument, kept as its source text.
    Integer(String),
    /// Raw key-value argument, kept as its source text.
    KeyVal(String),
    /// Column-specification argument, kept as its source text.
    Column(String),
    /// Boolean argument, primarily backing a star slot.
    Boolean(bool),
}

impl ArgValue {
    pub fn math(id: NodeId) -> Self {
        Self::Math(id)
    }

    pub fn text(id: NodeId) -> Self {
        Self::Text(id)
    }

    pub fn delimiter(delimiter: DelimiterValue) -> Self {
        Self::Delimiter(delimiter)
    }

    pub fn cs_name(value: impl Into<String>) -> Self {
        Self::CSName(value.into())
    }

    pub fn dimension(value: impl Into<String>) -> Self {
        Self::Dimension(value.into())
    }

    pub fn integer(value: impl Into<String>) -> Self {
        Self::Integer(value.into())
    }

    pub fn key_val(value: impl Into<String>) -> Self {
        Self::KeyVal(value.into())
    }

    pub fn column(value: impl Into<String>) -> Self {
        Self::Column(value.into())
    }

    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
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
/// Documents produced by the parser remember the [`ParseContextId`] of the
/// context that parsed them. Transform engines use that parser identity before
/// mutating a live document in place. Documents created directly with
/// [`Document::new`] or [`Document::from_syntax`] have no parser context id.
pub struct Document {
    ast: Ast,
    spans: SecondaryMap<RawNodeId, Span>,
    has_errors: bool,
    id: DocumentId,
    parse_context_id: Option<ParseContextId>,
}

impl Document {
    /// Create an empty document containing only an empty math-mode root.
    ///
    /// The document has no source parser context id.
    pub fn new() -> Self {
        Self::with_mode(ContentMode::Math)
    }

    /// Like [`Document::new`] but with an explicit root content mode.
    ///
    /// The document has no source parser context id.
    pub fn with_mode(mode: ContentMode) -> Self {
        Document {
            ast: Ast::with_root_mode(mode),
            spans: SecondaryMap::new(),
            has_errors: false,
            id: next_document_id(),
            parse_context_id: None,
        }
    }

    /// Build a document from a parsed syntax tree.
    ///
    /// This imports the tree but does not attach a parser context id. Only the
    /// parser bridge attaches that id for freshly parsed documents.
    pub fn from_syntax(node: &SyntaxNode) -> Result<Document, FromSyntaxError> {
        Self::validate_syntax(node, None, true)?;
        let ast = Ast::from_syntax_root(node);
        let has_errors = ast.contains_error();
        Ok(Document {
            ast,
            spans: SecondaryMap::new(),
            has_errors,
            id: next_document_id(),
            parse_context_id: None,
        })
    }

    /// Internal: build from syntax plus parser path spans.
    #[allow(dead_code)]
    pub(crate) fn from_syntax_with_spans(
        node: &SyntaxNode,
        path_spans: &[(String, Span)],
    ) -> Result<Document, FromSyntaxError> {
        let mut doc = Document::from_syntax(node)?;
        let lookup: HashMap<&str, &Span> = path_spans
            .iter()
            .map(|(path, span)| (path.as_str(), span))
            .collect();
        let mut spans = SecondaryMap::new();
        Self::assign_spans(&doc.ast, node, doc.ast.root(), "root", &lookup, &mut spans);
        doc.spans = spans;
        Ok(doc)
    }

    /// Process-wide unique id of this document.
    pub fn id(&self) -> DocumentId {
        self.id
    }

    /// Parser context that produced this document, when it came from parsing.
    ///
    /// `None` means the document was constructed directly or rebuilt from a
    /// syntax tree, so a transform engine cannot verify that it came from its
    /// own parser.
    pub fn parse_context_id(&self) -> Option<ParseContextId> {
        self.parse_context_id
    }

    /// Internal parser bridge: attach the source parser context to a freshly parsed document.
    pub(crate) fn set_parse_context_id(&mut self, id: ParseContextId) {
        self.parse_context_id = Some(id);
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

    /// Create a detached character node.
    pub fn create_char(&mut self, c: char) -> Result<NodeId, EditError> {
        self.create_node(Node::Char(c))
    }

    /// Create a detached text node.
    pub fn create_text(&mut self, s: impl Into<String>) -> Result<NodeId, EditError> {
        self.create_node(Node::Text(s.into()))
    }

    /// Create a detached active `~` space node.
    pub fn create_active_space(&mut self) -> Result<NodeId, EditError> {
        self.create_node(Node::ActiveSpace)
    }

    /// Create a detached group node.
    pub fn create_group(&mut self, mode: ContentMode) -> Result<NodeId, EditError> {
        self.create_node(Node::Group {
            children: Vec::new(),
            kind: GroupKind::Explicit,
            mode,
        })
    }

    /// Create a detached command node.
    pub fn create_command(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
    ) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let args = self.build_arg_slots(args)?;
        self.create_node(Node::Command {
            name: name.into(),
            args,
            known: false,
        })
    }

    /// Create a detached declarative command node.
    pub fn create_declarative(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
    ) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let args = self.build_arg_slots(args)?;
        self.create_node(Node::Declarative {
            name: name.into(),
            args,
        })
    }

    /// Create a detached environment node.
    pub fn create_environment(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
        body: NodeId,
    ) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let body = self.check_node_owner(body)?;
        self.check_detached(body)?;
        if !matches!(self.ast.node_opt(body), Some(Node::Group { .. })) {
            return Err(EditError::SlotShapeMismatch { expected: "group" });
        }
        let args = self.build_arg_slots(args)?;
        self.create_node(Node::Environment {
            name: name.into(),
            args,
            known: false,
            body,
        })
    }

    /// Append a detached node to a root/group container.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        self.insert_child_at(parent, self.child_len(parent)?, child)
    }

    /// Insert a detached node before an attached group child.
    pub fn insert_before(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let (parent, index) = self.group_child_position(anchor)?;
        self.insert_child_at(parent, index, new)
    }

    /// Insert a detached node after an attached group child.
    pub fn insert_after(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let (parent, index) = self.group_child_position(anchor)?;
        self.insert_child_at(parent, index + 1, new)
    }

    /// Insert a detached node at `index` in a root/group container.
    pub fn insert_child(
        &mut self,
        parent: NodeId,
        index: usize,
        child: NodeId,
    ) -> Result<(), EditError> {
        self.check_writable()?;
        self.insert_child_at(parent, index, child)
    }

    /// Detach an attached group child and return it as a detached root.
    pub fn extract(&mut self, id: NodeId) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        if raw == self.ast.root() {
            return Err(EditError::CannotEditRoot);
        }
        if !matches!(self.ast.slot(raw), Some(Slot::GroupChild(_))) {
            return Err(EditError::SlotShapeMismatch {
                expected: "group child",
            });
        }
        Ok(NodeId::new(self.id, self.ast.detach(raw)))
    }

    /// Remove an attached group child and its subtree.
    pub fn remove(&mut self, id: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.extract(id)?.raw();
        self.ast.remove_detached(raw);
        Ok(())
    }

    /// Replace `target` with a detached `replacement`.
    pub fn replace_with(&mut self, target: NodeId, replacement: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let target = self.check_node_owner(target)?;
        let replacement = self.check_node_owner(replacement)?;
        if target == self.ast.root() {
            return Err(EditError::CannotEditRoot);
        }
        self.check_detached(replacement)?;
        self.check_no_cycle(replacement, target)?;
        match self.ast.slot(target) {
            Some(Slot::GroupChild(index)) => {
                let parent = self.ast.parent_id(target).ok_or(EditError::NodeNotFound)?;
                let detached = self.ast.detach(target);
                self.ast.insert_child(parent, index, replacement);
                self.ast.remove_detached(detached);
                Ok(())
            }
            Some(_) => {
                let slot = self.ast.slot(target).ok_or(EditError::NodeNotFound)?;
                self.check_slot_shape(slot, replacement)?;
                self.ast.replace_content_child(target, replacement);
                if self.ast.contains(target)
                    && self.ast.parent_id(target).is_none()
                    && target != self.ast.root()
                {
                    self.ast.remove_detached(target);
                }
                Ok(())
            }
            None => Err(EditError::NodeNotFound),
        }
    }

    /// Remove all direct children from a root/group container.
    pub fn clear(&mut self, container: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(container)?;
        self.check_container(raw)?;
        let len = self.ast.children(raw).len();
        let removed = self.ast.detach_children_range(raw, 0..len);
        for child in removed {
            self.ast.remove_detached(child);
        }
        Ok(())
    }

    /// Set the name of a command/infix/declarative/environment node.
    pub fn set_command_name(
        &mut self,
        id: NodeId,
        name: impl Into<String>,
    ) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        match self.ast.node_opt_mut(raw) {
            Some(Node::Command { name: current, .. })
            | Some(Node::Infix { name: current, .. })
            | Some(Node::Declarative { name: current, .. })
            | Some(Node::Environment { name: current, .. }) => {
                *current = name.into();
                Ok(())
            }
            Some(_) => Err(EditError::SlotShapeMismatch {
                expected: "command-like node",
            }),
            None => Err(EditError::NodeNotFound),
        }
    }

    /// Set the payload of a text node.
    pub fn set_text(&mut self, id: NodeId, s: impl Into<String>) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        match self.ast.node_opt_mut(raw) {
            Some(Node::Text(text)) => {
                *text = s.into();
                Ok(())
            }
            Some(_) => Err(EditError::SlotShapeMismatch {
                expected: "text node",
            }),
            None => Err(EditError::NodeNotFound),
        }
    }

    /// Set the character of a char node.
    pub fn set_char(&mut self, id: NodeId, c: char) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        match self.ast.node_opt_mut(raw) {
            Some(Node::Char(ch)) => {
                *ch = c;
                Ok(())
            }
            Some(_) => Err(EditError::SlotShapeMismatch {
                expected: "char node",
            }),
            None => Err(EditError::NodeNotFound),
        }
    }

    /// Replace the value at argument slot `index` of a command-like node.
    pub fn set_arg(&mut self, id: NodeId, index: usize, value: ArgValue) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        let kind = match self.ast.node_opt(raw) {
            Some(
                Node::Command { args, .. }
                | Node::Infix { args, .. }
                | Node::Declarative { args, .. }
                | Node::Environment { args, .. },
            ) => {
                let Some(slot) = args.get(index) else {
                    return Err(EditError::IndexOutOfBounds);
                };
                let Some(argument) = slot else {
                    return Err(EditError::SlotShapeMismatch {
                        expected: "filled argument slot",
                    });
                };
                argument.kind.clone()
            }
            Some(_) => {
                return Err(EditError::SlotShapeMismatch {
                    expected: "command-like node",
                });
            }
            None => return Err(EditError::NodeNotFound),
        };
        let value = self.build_arg_value_for_kind(value, &kind)?;
        self.replace_single_arg_value(raw, index, value)
    }

    /// Wrap a group-child target with a detached root/group wrapper.
    pub fn wrap(&mut self, target: NodeId, wrapper: NodeId) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let target = self.check_node_owner(target)?;
        let wrapper = self.check_node_owner(wrapper)?;
        if target == self.ast.root() {
            return Err(EditError::CannotEditRoot);
        }
        self.check_container(wrapper)?;
        self.check_detached(wrapper)?;
        let (parent, index) = match self.ast.slot(target) {
            Some(Slot::GroupChild(index)) => (
                self.ast.parent_id(target).ok_or(EditError::NodeNotFound)?,
                index,
            ),
            Some(_) => {
                return Err(EditError::SlotShapeMismatch {
                    expected: "group child",
                });
            }
            None => return Err(EditError::NodeNotFound),
        };
        self.check_no_cycle(wrapper, parent)?;
        let target = self.ast.detach(target);
        self.ast.insert_child(parent, index, wrapper);
        self.ast.append_child(wrapper, target);
        Ok(NodeId::new(self.id, wrapper))
    }

    /// Remove a group-child group and splice its children into the parent.
    pub fn unwrap(&mut self, group: NodeId) -> Result<Vec<NodeId>, EditError> {
        self.check_writable()?;
        let group = self.check_node_owner(group)?;
        if group == self.ast.root() {
            return Err(EditError::CannotEditRoot);
        }
        if !matches!(self.ast.node_opt(group), Some(Node::Group { .. })) {
            return Err(EditError::SlotShapeMismatch { expected: "group" });
        }
        let (parent, index) = match self.ast.slot(group) {
            Some(Slot::GroupChild(index)) => (
                self.ast.parent_id(group).ok_or(EditError::NodeNotFound)?,
                index,
            ),
            Some(_) => {
                return Err(EditError::SlotShapeMismatch {
                    expected: "group child",
                });
            }
            None => return Err(EditError::NodeNotFound),
        };
        let count = self.ast.children(group).len();
        let children = self.ast.detach_children_range(group, 0..count);
        let detached_group = self.ast.detach(group);
        self.ast.remove_detached(detached_group);
        for (offset, child) in children.iter().copied().enumerate() {
            self.ast.insert_child(parent, index + offset, child);
        }
        Ok(children
            .into_iter()
            .map(|raw| NodeId::new(self.id, raw))
            .collect())
    }

    fn create_node(&mut self, node: Node) -> Result<NodeId, EditError> {
        self.check_writable()?;
        self.check_unique_direct_children(&node)?;
        Ok(NodeId::new(self.id, self.ast.new_node(node)))
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
        if self.ast.subtree_contains_node(child, new_parent) {
            Err(EditError::WouldCreateCycle)
        } else {
            Ok(())
        }
    }

    fn check_slot_shape(&self, slot: Slot, child: RawNodeId) -> Result<(), EditError> {
        if matches!(slot, Slot::EnvBody)
            && !matches!(self.ast.node_opt(child), Some(Node::Group { .. }))
        {
            Err(EditError::SlotShapeMismatch { expected: "group" })
        } else {
            Ok(())
        }
    }

    fn check_unique_direct_children(&self, node: &Node) -> Result<(), EditError> {
        let mut seen = HashSet::new();
        for child in Self::direct_children(node) {
            if !seen.insert(child) {
                return Err(EditError::DuplicateChild);
            }
        }
        Ok(())
    }

    fn direct_children(node: &Node) -> Vec<RawNodeId> {
        let mut children = Vec::new();
        match node {
            Node::Root {
                children: group_children,
                ..
            }
            | Node::Group {
                children: group_children,
                ..
            } => children.extend(group_children.iter().copied()),
            Node::Command { args, .. } | Node::Declarative { args, .. } => {
                Self::push_arg_children(args, &mut children);
            }
            Node::Infix {
                args, left, right, ..
            } => {
                children.push(*left);
                Self::push_arg_children(args, &mut children);
                children.push(*right);
            }
            Node::Environment { args, body, .. } => {
                Self::push_arg_children(args, &mut children);
                children.push(*body);
            }
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => {
                children.push(*base);
                children.extend(*subscript);
                children.extend(*superscript);
            }
            Node::Prime { .. }
            | Node::Text(_)
            | Node::Char(_)
            | Node::ActiveSpace
            | Node::Error { .. } => {}
        }
        children
    }

    fn validate_syntax(
        node: &SyntaxNode,
        current_mode: Option<ContentMode>,
        is_top_level: bool,
    ) -> Result<(), FromSyntaxError> {
        if is_top_level && !matches!(node, SyntaxNode::Root { .. }) {
            return Err(FromSyntaxError::NotARoot);
        }

        match node {
            SyntaxNode::Root { mode, children } => {
                if !is_top_level {
                    return Err(FromSyntaxError::NotARoot);
                }
                for child in children {
                    Self::validate_syntax(child, Some(*mode), false)?;
                }
            }
            SyntaxNode::Group { mode, children, .. } => {
                for child in children {
                    Self::validate_syntax(child, Some(*mode), false)?;
                }
            }
            SyntaxNode::Command { args, .. } | SyntaxNode::Declarative { args, .. } => {
                Self::validate_syntax_args(args)?;
            }
            SyntaxNode::Infix {
                args, left, right, ..
            } => {
                Self::validate_syntax_args(args)?;
                Self::validate_syntax(left, current_mode, false)?;
                Self::validate_syntax(right, current_mode, false)?;
            }
            SyntaxNode::Environment { args, body, .. } => {
                Self::validate_syntax_args(args)?;
                Self::validate_syntax(body, None, false)?;
            }
            SyntaxNode::Scripted {
                base,
                subscript,
                superscript,
            } => {
                Self::validate_syntax(base, current_mode, false)?;
                if let Some(subscript) = subscript {
                    Self::validate_syntax(subscript, current_mode, false)?;
                }
                if let Some(superscript) = superscript {
                    Self::validate_syntax(superscript, current_mode, false)?;
                }
            }
            SyntaxNode::Prime { count } => {
                if *count == 0 {
                    return Err(FromSyntaxError::InvalidPrimeCount);
                }
                if current_mode != Some(ContentMode::Math) {
                    return Err(FromSyntaxError::PrimeInTextMode);
                }
            }
            SyntaxNode::Error { .. }
            | SyntaxNode::Text(_)
            | SyntaxNode::Char(_)
            | SyntaxNode::ActiveSpace => {}
        }

        Ok(())
    }

    fn validate_syntax_args(args: &[syntax_node::ArgumentSlot]) -> Result<(), FromSyntaxError> {
        for arg in args.iter().flatten() {
            match &arg.value {
                syntax_node::ArgumentValue::MathContent(node) => {
                    Self::validate_syntax(node, Some(ContentMode::Math), false)?;
                }
                syntax_node::ArgumentValue::TextContent(node) => {
                    Self::validate_syntax(node, Some(ContentMode::Text), false)?;
                }
                syntax_node::ArgumentValue::OperatorNameContent(node) => {
                    Self::validate_syntax(node, Some(ContentMode::Math), false)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn push_arg_children(args: &[ArgumentSlot], out: &mut Vec<RawNodeId>) {
        for arg in args.iter().flatten() {
            match &arg.value {
                ArgumentValue::MathContent(child)
                | ArgumentValue::TextContent(child)
                | ArgumentValue::OperatorNameContent(child) => {
                    out.push(*child);
                }
                _ => {}
            }
        }
    }

    fn group_child_position(&self, anchor: NodeId) -> Result<(NodeId, usize), EditError> {
        let raw = self.check_node_owner(anchor)?;
        match self.ast.slot(raw) {
            Some(Slot::GroupChild(index)) => {
                let parent = self.ast.parent_id(raw).ok_or(EditError::NodeNotFound)?;
                Ok((NodeId::new(self.id, parent), index))
            }
            Some(_) => Err(EditError::SlotShapeMismatch {
                expected: "group child",
            }),
            None => Err(EditError::NodeNotFound),
        }
    }

    fn insert_child_at(
        &mut self,
        parent: NodeId,
        index: usize,
        child: NodeId,
    ) -> Result<(), EditError> {
        self.check_writable()?;
        let parent = self.check_node_owner(parent)?;
        let child = self.check_node_owner(child)?;
        self.check_container(parent)?;
        self.check_detached(child)?;
        self.check_no_cycle(child, parent)?;
        if index > self.ast.children(parent).len() {
            return Err(EditError::IndexOutOfBounds);
        }
        self.ast.insert_child(parent, index, child);
        Ok(())
    }

    fn build_arg_slots(&mut self, args: Vec<ArgValue>) -> Result<Vec<ArgumentSlot>, EditError> {
        args.into_iter()
            .map(|arg| self.build_arg_slot(arg))
            .collect()
    }

    fn build_arg_slot(&mut self, value: ArgValue) -> Result<ArgumentSlot, EditError> {
        let (kind, value) = match value {
            ArgValue::Math(id) => {
                let raw = self.check_node_owner(id)?;
                self.check_detached(raw)?;
                (ArgumentKind::Mandatory, ArgumentValue::MathContent(raw))
            }
            ArgValue::Text(id) => {
                let raw = self.check_node_owner(id)?;
                self.check_detached(raw)?;
                (ArgumentKind::Mandatory, ArgumentValue::TextContent(raw))
            }
            ArgValue::Delimiter(d) => (
                ArgumentKind::Mandatory,
                ArgumentValue::Delimiter(d.into_ast()),
            ),
            ArgValue::CSName(s) => (ArgumentKind::Mandatory, ArgumentValue::CSName(s)),
            ArgValue::Dimension(s) => (ArgumentKind::Mandatory, ArgumentValue::Dimension(s)),
            ArgValue::Integer(s) => (ArgumentKind::Mandatory, ArgumentValue::Integer(s)),
            ArgValue::KeyVal(s) => (ArgumentKind::Mandatory, ArgumentValue::KeyVal(s)),
            ArgValue::Column(s) => (ArgumentKind::Mandatory, ArgumentValue::Column(s)),
            ArgValue::Boolean(b) => (ArgumentKind::Star, ArgumentValue::Boolean(b)),
        };
        Ok(Some(Argument::from_value(kind, value)))
    }

    fn build_arg_value_for_kind(
        &self,
        value: ArgValue,
        kind: &ArgumentKind,
    ) -> Result<ArgumentValue, EditError> {
        match (kind, value) {
            (ArgumentKind::Star, ArgValue::Boolean(value)) => Ok(ArgumentValue::Boolean(value)),
            (ArgumentKind::Star, _) => Err(EditError::SlotShapeMismatch {
                expected: "boolean value",
            }),
            (_, ArgValue::Boolean(_)) => Err(EditError::SlotShapeMismatch {
                expected: "non-boolean argument value",
            }),
            (_, ArgValue::Math(id)) => {
                let raw = self.check_node_owner(id)?;
                self.check_detached(raw)?;
                Ok(ArgumentValue::MathContent(raw))
            }
            (_, ArgValue::Text(id)) => {
                let raw = self.check_node_owner(id)?;
                self.check_detached(raw)?;
                Ok(ArgumentValue::TextContent(raw))
            }
            (_, ArgValue::Delimiter(delimiter)) => {
                Ok(ArgumentValue::Delimiter(delimiter.into_ast()))
            }
            (_, ArgValue::CSName(value)) => Ok(ArgumentValue::CSName(value)),
            (_, ArgValue::Dimension(value)) => Ok(ArgumentValue::Dimension(value)),
            (_, ArgValue::Integer(value)) => Ok(ArgumentValue::Integer(value)),
            (_, ArgValue::KeyVal(value)) => Ok(ArgumentValue::KeyVal(value)),
            (_, ArgValue::Column(value)) => Ok(ArgumentValue::Column(value)),
        }
    }

    fn replace_single_arg_value(
        &mut self,
        id: RawNodeId,
        index: usize,
        value: ArgumentValue,
    ) -> Result<(), EditError> {
        let old_content = self
            .ast
            .arg_slots(id)
            .get(index)
            .and_then(|slot| slot.as_ref())
            .and_then(|arg| match &arg.value {
                ArgumentValue::MathContent(content)
                | ArgumentValue::TextContent(content)
                | ArgumentValue::OperatorNameContent(content) => Some(*content),
                _ => None,
            });

        let mut node = self.ast.node(id).clone();
        match &mut node {
            Node::Command { args, .. }
            | Node::Infix { args, .. }
            | Node::Declarative { args, .. }
            | Node::Environment { args, .. } => {
                let Some(Some(argument)) = args.get_mut(index) else {
                    return Err(EditError::SlotShapeMismatch {
                        expected: "filled argument slot",
                    });
                };
                argument.value = value;
            }
            _ => {
                return Err(EditError::SlotShapeMismatch {
                    expected: "command-like node",
                });
            }
        }
        self.ast.replace_node(id, node);

        if let Some(old) = old_content
            && self.ast.contains(old)
            && self.ast.parent_id(old).is_none()
            && old != self.ast.root()
        {
            self.ast.remove_detached(old);
        }
        Ok(())
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
        match self.ast.node(id) {
            Node::Root { children, .. } | Node::Group { children, .. } => {
                for (index, child) in children.iter().enumerate() {
                    self.collect_node_spans(*child, &format!("{path}.child.{index}"), out);
                }
            }
            Node::Command { args, .. } | Node::Declarative { args, .. } => {
                self.collect_arg_node_spans(args, path, out);
            }
            Node::Infix {
                args, left, right, ..
            } => {
                self.collect_node_spans(*left, &format!("{path}.left"), out);
                self.collect_arg_node_spans(args, path, out);
                self.collect_node_spans(*right, &format!("{path}.right"), out);
            }
            Node::Environment { args, body, .. } => {
                self.collect_arg_node_spans(args, path, out);
                self.collect_node_spans(*body, &format!("{path}.body"), out);
            }
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => {
                self.collect_node_spans(*base, &format!("{path}.base"), out);
                if let Some(sub) = subscript {
                    self.collect_node_spans(*sub, &format!("{path}.sub"), out);
                }
                if let Some(sup) = superscript {
                    self.collect_node_spans(*sup, &format!("{path}.sup"), out);
                }
            }
            _ => {}
        }
    }

    fn collect_arg_node_spans(
        &self,
        args: &[ArgumentSlot],
        path: &str,
        out: &mut Vec<(String, Span)>,
    ) {
        for (index, slot) in args.iter().enumerate() {
            let Some(arg) = slot else { continue };
            if let ArgumentValue::MathContent(id)
            | ArgumentValue::TextContent(id)
            | ArgumentValue::OperatorNameContent(id) = &arg.value
            {
                self.collect_node_spans(*id, &format!("{path}.arg.{index}.content"), out);
            }
        }
    }

    #[allow(dead_code)]
    fn assign_spans(
        ast: &Ast,
        syntax: &SyntaxNode,
        id: RawNodeId,
        path: &str,
        lookup: &HashMap<&str, &Span>,
        out: &mut SecondaryMap<RawNodeId, Span>,
    ) {
        if let Some(span) = lookup.get(path) {
            out.insert(id, (*span).clone());
        }

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
                    Self::assign_spans(
                        ast,
                        syntax_child,
                        *ast_child,
                        &format!("{path}.child.{index}"),
                        lookup,
                        out,
                    );
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
            ) => Self::assign_arg_spans(ast, syntax_args, ast_args, path, lookup, out),
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
                Self::assign_spans(
                    ast,
                    syntax_left,
                    *left,
                    &format!("{path}.left"),
                    lookup,
                    out,
                );
                Self::assign_arg_spans(ast, syntax_args, ast_args, path, lookup, out);
                Self::assign_spans(
                    ast,
                    syntax_right,
                    *right,
                    &format!("{path}.right"),
                    lookup,
                    out,
                );
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
                Self::assign_arg_spans(ast, syntax_args, ast_args, path, lookup, out);
                Self::assign_spans(
                    ast,
                    syntax_body,
                    *body,
                    &format!("{path}.body"),
                    lookup,
                    out,
                );
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
                Self::assign_spans(
                    ast,
                    syntax_base,
                    *base,
                    &format!("{path}.base"),
                    lookup,
                    out,
                );
                if let (Some(syntax), Some(ast_id)) = (syntax_subscript, subscript) {
                    Self::assign_spans(ast, syntax, *ast_id, &format!("{path}.sub"), lookup, out);
                }
                if let (Some(syntax), Some(ast_id)) = (syntax_superscript, superscript) {
                    Self::assign_spans(ast, syntax, *ast_id, &format!("{path}.sup"), lookup, out);
                }
            }
            _ => {}
        }
    }

    #[allow(dead_code)]
    fn assign_arg_spans(
        ast: &Ast,
        syntax_args: &[syntax_node::ArgumentSlot],
        ast_args: &[ArgumentSlot],
        path: &str,
        lookup: &HashMap<&str, &Span>,
        out: &mut SecondaryMap<RawNodeId, Span>,
    ) {
        for (index, (syntax_slot, ast_slot)) in syntax_args.iter().zip(ast_args.iter()).enumerate()
        {
            let (Some(syntax_arg), Some(ast_arg)) = (syntax_slot, ast_slot) else {
                continue;
            };
            let content_path = format!("{path}.arg.{index}.content");
            if let (
                syntax_node::ArgumentValue::MathContent(syntax_node)
                | syntax_node::ArgumentValue::TextContent(syntax_node)
                | syntax_node::ArgumentValue::OperatorNameContent(syntax_node),
                ArgumentValue::MathContent(ast_id)
                | ArgumentValue::TextContent(ast_id)
                | ArgumentValue::OperatorNameContent(ast_id),
            ) = (&syntax_arg.value, &ast_arg.value)
            {
                Self::assign_spans(ast, syntax_node, *ast_id, &content_path, lookup, out);
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
            parse_context_id: self.parse_context_id,
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

    /// Prime mark count for a `Prime` node.
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
        match self.node() {
            Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
            _ => None,
        }
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
            ArgumentValue::OperatorNameContent(id) => ArgRef::Math(self.sibling(*id)),
            ArgumentValue::Delimiter(delimiter) => {
                ArgRef::Delimiter(Self::delimiter_ref(delimiter))
            }
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
                left: Self::delimiter_ref(left),
                right: Self::delimiter_ref(right),
            },
            GroupKind::InlineMath => GroupKindRef::InlineMath,
        }
    }

    fn delimiter_ref(delimiter: &'a Delimiter) -> DelimiterRef<'a> {
        match delimiter {
            Delimiter::None => DelimiterRef::None,
            Delimiter::Char(ch) => DelimiterRef::Char(*ch),
            Delimiter::Control(name) => DelimiterRef::Control(name),
        }
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
            ArgRef::Math(node) | ArgRef::Text(node) => Some(node),
            _ => None,
        }
    }
}

#[cfg(test)]
impl Document {
    fn from_ast_for_test(build: impl FnOnce(&mut Ast)) -> Self {
        let mut ast = Ast::new();
        build(&mut ast);
        ast.assert_invariants();
        Document {
            ast,
            spans: SecondaryMap::new(),
            has_errors: false,
            id: next_document_id(),
            parse_context_id: None,
        }
    }

    fn from_ast_with_errors_for_test(build: impl FnOnce(&mut Ast)) -> Self {
        let mut ast = Ast::new();
        build(&mut ast);
        ast.assert_invariants();
        let has_errors = ast.contains_error();
        Document {
            ast,
            spans: SecondaryMap::new(),
            has_errors,
            id: next_document_id(),
            parse_context_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_is_empty_and_editable() {
        let doc = Document::new();
        assert_eq!(doc.root().kind(), NodeKind::Root);
        assert!(!doc.has_errors());
        assert!(!doc.is_read_only());
        assert_eq!(doc.errors().count(), 0);
    }

    #[test]
    fn document_ids_are_unique_across_construction_and_clone() {
        let a = Document::new();
        let b = Document::new();
        assert_ne!(a.id(), b.id());

        let c = a.clone();
        assert_ne!(a.id(), c.id());
    }

    #[test]
    fn node_ref_reads_synthesized_chars() {
        let doc = Document::from_ast_for_test(|ast| {
            let a = ast.new_node(Node::Char('a'));
            let b = ast.new_node(Node::Char('b'));
            ast.append_child(ast.root(), a);
            ast.append_child(ast.root(), b);
        });

        let mut children = doc.root().children();
        let first = children.next().unwrap();
        let second = children.next().unwrap();
        assert!(first.is_char('a'));
        assert!(second.is_char('b'));
        assert_eq!(first.next_sibling().map(|n| n.id()), Some(second.id()));
        assert_eq!(second.prev_sibling().map(|n| n.id()), Some(first.id()));
        assert_eq!(doc.root().children().count(), 2);
    }

    #[test]
    fn find_all_collects_matching_nodes() {
        let doc = Document::from_ast_for_test(|ast| {
            let a = ast.new_node(Node::Char('a'));
            let group = ast.new_node(Node::Group {
                children: vec![a],
                kind: crate::ast::GroupKind::Explicit,
                mode: ContentMode::Math,
            });
            let b = ast.new_node(Node::Char('b'));
            ast.append_child(ast.root(), group);
            ast.append_child(ast.root(), b);
        });

        let chars: Vec<_> = doc
            .find_all(doc.root(), |node| node.char().is_some())
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b']);
    }

    #[test]
    fn edit_error_implements_display_and_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<EditError>();

        assert_eq!(
            EditError::CannotEditRoot.to_string(),
            "cannot edit the root node"
        );
        assert_eq!(
            EditError::SlotShapeMismatch { expected: "group" }.to_string(),
            "slot shape mismatch: expected group"
        );
    }

    #[test]
    fn create_and_append_children() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();

        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();

        let chars: Vec<_> = doc
            .root()
            .children()
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b']);
    }

    #[test]
    fn insert_before_and_after() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let c = doc.create_char('c').unwrap();
        let b = doc.create_char('b').unwrap();
        let d = doc.create_char('d').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();
        doc.append_child(root, c).unwrap();

        doc.insert_before(c, b).unwrap();
        doc.insert_after(c, d).unwrap();

        let chars: Vec<_> = doc
            .root()
            .children()
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b', 'c', 'd']);
    }

    #[test]
    fn remove_and_extract() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();

        let extracted = doc.extract(a).unwrap();
        assert_eq!(extracted, a);
        doc.remove(b).unwrap();

        assert_eq!(doc.root().children().count(), 0);
        doc.append_child(root, extracted).unwrap();
        assert_eq!(
            doc.root().children().next().and_then(|node| node.char()),
            Some('a')
        );
    }

    #[test]
    fn replace_with_swaps_node() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();

        doc.replace_with(a, b).unwrap();

        assert_eq!(
            doc.root().children().next().and_then(|node| node.char()),
            Some('b')
        );
    }

    #[test]
    fn set_text_and_char_and_command_name() {
        let mut doc = Document::new();
        let text = doc.create_text("old").unwrap();
        let ch = doc.create_char('a').unwrap();
        let cmd = doc.create_command("alpha", Vec::new()).unwrap();

        doc.set_text(text, "new").unwrap();
        doc.set_char(ch, 'b').unwrap();
        doc.set_command_name(cmd, "beta").unwrap();

        assert_eq!(doc.node(text).unwrap().text(), Some("new"));
        assert_eq!(doc.node(ch).unwrap().char(), Some('b'));
        assert_eq!(doc.node(cmd).unwrap().command_name(), Some("beta"));
    }

    #[test]
    fn cannot_edit_root() {
        let mut doc = Document::new();
        let root = doc.root().id();
        assert_eq!(doc.remove(root), Err(EditError::CannotEditRoot));
        assert_eq!(doc.extract(root), Err(EditError::CannotEditRoot));
    }

    #[test]
    fn append_to_non_container_fails() {
        let mut doc = Document::new();
        let parent = doc.create_char('a').unwrap();
        let child = doc.create_char('b').unwrap();

        assert_eq!(
            doc.append_child(parent, child),
            Err(EditError::NotAContainer)
        );
    }

    #[test]
    fn create_command_with_args_and_read_back() {
        let mut doc = Document::new();
        let numerator = doc.create_char('a').unwrap();
        let denominator = doc.create_char('b').unwrap();
        let frac = doc
            .create_command(
                "frac",
                vec![ArgValue::math(numerator), ArgValue::math(denominator)],
            )
            .unwrap();

        let node = doc.node(frac).unwrap();
        assert_eq!(node.command_name(), Some("frac"));
        assert_eq!(node.arg_count(), 2);
        assert_eq!(
            node.arg(0)
                .and_then(|arg| arg.as_node())
                .and_then(|node| node.char()),
            Some('a')
        );
        assert_eq!(
            node.arg(1)
                .and_then(|arg| arg.as_node())
                .and_then(|node| node.char()),
            Some('b')
        );
    }

    #[test]
    fn duplicate_child_is_rejected() {
        let mut doc = Document::new();
        let child = doc.create_char('x').unwrap();

        assert_eq!(
            doc.create_command("dup", vec![ArgValue::math(child), ArgValue::math(child)]),
            Err(EditError::DuplicateChild)
        );
    }

    #[test]
    fn set_arg_replaces_content() {
        let mut doc = Document::new();
        let old = doc.create_char('a').unwrap();
        let cmd = doc
            .create_command("sqrt", vec![ArgValue::math(old)])
            .unwrap();
        let new = doc.create_char('b').unwrap();

        doc.set_arg(cmd, 0, ArgValue::math(new)).unwrap();

        let node = doc.node(cmd).unwrap();
        assert_eq!(
            node.arg(0)
                .and_then(|arg| arg.as_node())
                .and_then(|node| node.char()),
            Some('b')
        );
    }

    #[test]
    fn set_arg_preserves_optional_slot_kind() {
        use texform_interface::syntax_node::{
            Argument as SyntaxArgument, ArgumentKind as SyntaxArgumentKind,
            ArgumentValue as SyntaxArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "sqrt".to_string(),
                args: vec![Some(SyntaxArgument {
                    kind: SyntaxArgumentKind::Optional,
                    no_leading_space: false,
                    value: SyntaxArgumentValue::MathContent(SyntaxNode::Char('a')),
                })],
                known: true,
            }],
        };
        let mut doc = Document::from_syntax(&syntax).unwrap();
        let command = doc.root().children().next().unwrap().id();
        let replacement = doc.create_char('b').unwrap();

        doc.set_arg(command, 0, ArgValue::math(replacement))
            .unwrap();

        assert_eq!(doc.to_latex().unwrap(), r"\sqrt [ b ]");
    }

    #[test]
    fn set_arg_preserves_no_leading_space_flag() {
        use texform_interface::syntax_node::{
            Argument as SyntaxArgument, ArgumentKind as SyntaxArgumentKind,
            ArgumentValue as SyntaxArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "probe".to_string(),
                args: vec![Some(SyntaxArgument {
                    kind: SyntaxArgumentKind::Optional,
                    no_leading_space: true,
                    value: SyntaxArgumentValue::MathContent(SyntaxNode::Char('a')),
                })],
                known: true,
            }],
        };
        let mut doc = Document::from_syntax(&syntax).unwrap();
        let command = doc.root().children().next().unwrap().id();
        let replacement = doc.create_char('b').unwrap();

        doc.set_arg(command, 0, ArgValue::math(replacement))
            .unwrap();

        let roundtrip = doc.to_syntax();
        let SyntaxNode::Root { children, .. } = roundtrip else {
            panic!("expected root");
        };
        let SyntaxNode::Command { args, .. } = &children[0] else {
            panic!("expected command");
        };

        assert!(args[0].as_ref().unwrap().no_leading_space);
    }

    #[test]
    fn set_arg_preserves_star_boolean_slot_kind() {
        use texform_interface::syntax_node::{
            Argument as SyntaxArgument, ArgumentKind as SyntaxArgumentKind,
            ArgumentValue as SyntaxArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "operatorname".to_string(),
                args: vec![
                    Some(SyntaxArgument {
                        kind: SyntaxArgumentKind::Star,
                        no_leading_space: false,
                        value: SyntaxArgumentValue::Boolean(false),
                    }),
                    Some(SyntaxArgument {
                        kind: SyntaxArgumentKind::Mandatory,
                        no_leading_space: false,
                        value: SyntaxArgumentValue::MathContent(SyntaxNode::Char('x')),
                    }),
                ],
                known: true,
            }],
        };
        let mut doc = Document::from_syntax(&syntax).unwrap();
        let command = doc.root().children().next().unwrap().id();

        doc.set_arg(command, 0, ArgValue::boolean(true)).unwrap();

        assert_eq!(doc.to_latex().unwrap(), r"\operatorname* { x }");
    }

    #[test]
    fn read_only_editing_checks_precede_invalid_node_checks() {
        let mut read_only = Document::from_ast_with_errors_for_test(|ast| {
            let err = ast.new_node(Node::Error {
                message: "bad".to_string(),
                snippet: "x".to_string(),
            });
            ast.append_child(ast.root(), err);
        });
        let mut other = Document::new();
        let foreign = other.create_char('x').unwrap();

        assert_eq!(
            read_only.append_child(foreign, foreign),
            Err(EditError::ReadOnlyDocument)
        );
        assert_eq!(
            read_only.insert_before(foreign, foreign),
            Err(EditError::ReadOnlyDocument)
        );
    }

    #[test]
    fn wrap_moves_target_inside_wrapper() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let wrapper = doc.create_group(ContentMode::Math).unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();

        let wrapped = doc.wrap(a, wrapper).unwrap();

        let group = doc.root().children().next().unwrap();
        assert_eq!(group.id(), wrapped);
        assert_eq!(
            group.children().next().and_then(|node| node.char()),
            Some('a')
        );
    }

    #[test]
    fn unwrap_splices_group_children_into_parent() {
        let mut doc = Document::new();
        let group = doc.create_group(ContentMode::Math).unwrap();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        doc.append_child(group, a).unwrap();
        doc.append_child(group, b).unwrap();
        let root = doc.root().id();
        doc.append_child(root, group).unwrap();

        let children = doc.unwrap(group).unwrap();

        assert_eq!(children.len(), 2);
        let chars: Vec<_> = doc
            .root()
            .children()
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b']);
    }

    #[test]
    fn error_tree_is_read_only() {
        let doc = Document::from_ast_with_errors_for_test(|ast| {
            let err = ast.new_node(Node::Error {
                message: "bad".to_string(),
                snippet: "x".to_string(),
            });
            ast.append_child(ast.root(), err);
        });
        assert!(doc.has_errors());
        assert!(doc.is_read_only());
        assert_eq!(doc.errors().count(), 1);

        let mut doc = doc;
        assert_eq!(doc.create_char('z'), Err(EditError::ReadOnlyDocument));
    }

    #[test]
    fn from_syntax_round_trips_clean_tree() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Char('a'), SyntaxNode::Char('b')],
        };
        let doc = Document::from_syntax(&syntax).unwrap();
        assert!(!doc.has_errors());

        assert_eq!(doc.to_syntax(), syntax);
    }

    #[test]
    fn from_syntax_rejects_zero_count_prime() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Prime { count: 0 }],
        };

        assert_eq!(
            Document::from_syntax(&syntax).expect_err("expected invalid prime count"),
            FromSyntaxError::InvalidPrimeCount
        );
    }

    #[test]
    fn from_syntax_rejects_text_mode_prime() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Text,
            children: vec![SyntaxNode::Prime { count: 1 }],
        };

        assert_eq!(
            Document::from_syntax(&syntax).expect_err("expected text-mode prime rejection"),
            FromSyntaxError::PrimeInTextMode
        );
    }

    #[test]
    fn from_syntax_rejects_text_mode_scripted_prime() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Text,
            children: vec![SyntaxNode::Scripted {
                base: Box::new(SyntaxNode::Prime { count: 1 }),
                subscript: None,
                superscript: None,
            }],
        };

        assert_eq!(
            Document::from_syntax(&syntax)
                .expect_err("expected text-mode scripted prime rejection"),
            FromSyntaxError::PrimeInTextMode
        );
    }

    #[test]
    fn from_syntax_rejects_text_content_scripted_prime() {
        use texform_interface::syntax_node::{
            Argument, ArgumentKind, ArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "text".to_string(),
                args: vec![Some(Argument {
                    kind: ArgumentKind::Mandatory,
                    no_leading_space: false,
                    value: ArgumentValue::TextContent(SyntaxNode::Scripted {
                        base: Box::new(SyntaxNode::Prime { count: 1 }),
                        subscript: None,
                        superscript: None,
                    }),
                })],
                known: true,
            }],
        };

        assert_eq!(
            Document::from_syntax(&syntax)
                .expect_err("expected text-content scripted prime rejection"),
            FromSyntaxError::PrimeInTextMode
        );
    }

    #[test]
    fn from_syntax_marks_error_tree_read_only() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Error {
                message: "bad".to_string(),
                snippet: "x".to_string(),
            }],
        };
        let doc = Document::from_syntax(&syntax).unwrap();
        assert!(doc.has_errors());
        assert!(doc.is_read_only());
    }

    #[test]
    fn from_syntax_alone_has_no_spans() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Char('a')],
        };
        let doc = Document::from_syntax(&syntax).unwrap();
        assert_eq!(doc.root().children().next().unwrap().span(), None);
    }

    #[test]
    fn span_mapping_aligns_paths_to_node_ids() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Char('a'), SyntaxNode::Char('b')],
        };
        let path_spans = vec![
            ("root".to_string(), Span { start: 0, end: 2 }),
            ("root.child.0".to_string(), Span { start: 0, end: 1 }),
            ("root.child.1".to_string(), Span { start: 1, end: 2 }),
        ];

        let doc = Document::from_syntax_with_spans(&syntax, &path_spans).unwrap();
        let mut kids = doc.root().children();
        let a = kids.next().unwrap();
        let b = kids.next().unwrap();
        assert_eq!(a.span(), Some(Span { start: 0, end: 1 }));
        assert_eq!(b.span(), Some(Span { start: 1, end: 2 }));
        assert_eq!(doc.root().span(), Some(Span { start: 0, end: 2 }));
    }

    #[test]
    fn to_latex_default_and_with_options() {
        use crate::serialize::SerializeOptions;

        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();

        assert_eq!(doc.to_latex().unwrap(), "a b");
        assert_eq!(format!("{doc}"), "a b");

        let opts = SerializeOptions::default();
        assert_eq!(doc.to_latex_with(&opts).unwrap(), "a b");
    }
}
