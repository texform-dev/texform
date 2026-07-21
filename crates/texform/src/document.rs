//! The editable LaTeX document tree.
//!
//! [`Document`] is the public, DOM-style tree users read, edit, serialize, and
//! transform. Reads go through read-only [`NodeRef`] handles; edits are fallible
//! and return [`EditError`], so no internal panic ever reaches a caller. A tree
//! that [`has_errors`](Document::has_errors) is read-only: every editing method
//! returns [`EditError::ReadOnlyDocument`].
//!
//! Nodes are built with the `create_*` methods, which stage detached subtrees
//! identified by [`NodeId`], then attached into the tree with
//! [`append_child`](Document::append_child), [`insert_before`](Document::insert_before),
//! [`wrap`](Document::wrap), and friends. A [`NodeId`] carries the identity of
//! its owning document, so an edit referencing a node from another document
//! fails with [`EditError::ForeignNode`] instead of corrupting an unrelated tree.

pub use texform_core::document::{
    ArgRef, ArgValue, DelimiterRef, DelimiterValue, DocumentId, EditError, FromSyntaxError,
    GroupKindRef, NodeId, NodeKind, NodeRef,
};
pub use texform_core::serialize::SerializeOptions;

pub use crate::serialize::{SerializeError, TokenizedLatex};

/// Parse-time source span of one tree node, addressed by its tree path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSpanEntry {
    /// Tree path such as `root.child.0.arg.1.content` (see [`Document::node_spans`]).
    pub id: String,
    /// Source byte span recorded by the parser.
    pub span: texform_core::parse::Span,
}

/// Editable LaTeX document tree.
///
/// Documents extracted from a [`ParseResult`](crate::ParseResult) remember the
/// parser context that produced them. [`TransformEngine::transform`](crate::TransformEngine::transform)
/// uses that parser identity to accept only documents parsed through the same
/// engine. Documents created with [`Document::new`], [`Document::with_mode`],
/// or [`Document::from_syntax`] can still be edited and serialized, but they
/// cannot be transformed in place by a `TransformEngine`.
///
/// # Examples
///
/// Build a tree from scratch by staging detached nodes and attaching them:
///
/// ```
/// use texform::Document;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut doc = Document::new();
/// let root = doc.root().id();
/// let x = doc.create_char('x')?;
/// doc.append_child(root, x)?;
/// assert_eq!(doc.to_latex()?, "x");
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Document {
    inner: texform_core::document::Document,
}

impl Document {
    /// Create an empty math-mode document.
    ///
    /// The document is editable and serializable, but it is not associated
    /// with any parser context and cannot be passed to
    /// [`TransformEngine::transform`](crate::TransformEngine::transform).
    pub fn new() -> Self {
        Self::from_core(texform_core::document::Document::new())
    }

    /// Create an empty document with an explicit root content mode.
    ///
    /// The document is not associated with any parser context and cannot be
    /// passed to [`TransformEngine::transform`](crate::TransformEngine::transform).
    pub fn with_mode(mode: texform_core::parse::ContentMode) -> Self {
        Self::from_core(texform_core::document::Document::with_mode(mode))
    }

    /// Build a document from a syntax tree.
    ///
    /// This path validates and imports the tree, but it does not attach the
    /// parser context required by [`TransformEngine::transform`](crate::TransformEngine::transform).
    pub fn from_syntax(
        node: &texform_interface::syntax_node::SyntaxNode,
    ) -> Result<Self, FromSyntaxError> {
        Ok(Self::from_core(
            texform_core::document::Document::from_syntax(node)?,
        ))
    }

    pub(crate) fn from_core(inner: texform_core::document::Document) -> Self {
        Self { inner }
    }

    pub(crate) fn core_mut(&mut self) -> &mut texform_core::document::Document {
        &mut self.inner
    }

    pub(crate) fn parse_context_id(&self) -> Option<texform_core::parse::ParseContextId> {
        self.inner.parse_context_id()
    }

    /// The root node of the tree.
    ///
    /// The root is unique and parentless; it is the document's top-level
    /// container node — a `Root` node, not a `Group` — and its children are the
    /// top-level content.
    pub fn root(&self) -> NodeRef<'_> {
        self.inner.root()
    }

    /// Resolve a [`NodeId`] to a read-only handle.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ForeignNode`] if the id belongs to another document,
    /// or [`EditError::NodeNotFound`] if it has been removed.
    pub fn node(&self, id: NodeId) -> Result<NodeRef<'_>, EditError> {
        self.inner.node(id)
    }

    /// Whether the tree contains any parse-error placeholder nodes.
    ///
    /// This is an O(1) query and is independent of parse strictness. A document
    /// with errors is read-only; see the module-level documentation.
    pub fn has_errors(&self) -> bool {
        self.inner.has_errors()
    }

    /// Whether the document rejects edits.
    ///
    /// Read-only-ness is fixed at construction and is equivalent to
    /// [`has_errors`](Self::has_errors): a tree containing error nodes cannot be
    /// edited, so its error count cannot change.
    pub fn is_read_only(&self) -> bool {
        self.inner.is_read_only()
    }

    /// Iterate over the parse-error placeholder nodes in the tree.
    pub fn errors(&self) -> impl Iterator<Item = NodeRef<'_>> + '_ {
        self.inner.errors()
    }

    /// Find the first node at or under `start` that satisfies `pred`, in
    /// document order.
    pub fn find<'a>(
        &'a self,
        start: NodeRef<'a>,
        pred: impl Fn(NodeRef<'a>) -> bool + 'a,
    ) -> Option<NodeRef<'a>> {
        self.inner.find(start, pred)
    }

    /// Iterate over every node at or under `start` that satisfies `pred`, in
    /// document order.
    pub fn find_all<'a>(
        &'a self,
        start: NodeRef<'a>,
        pred: impl Fn(NodeRef<'a>) -> bool + 'a,
    ) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.inner.find_all(start, pred)
    }

    /// Iterate over every command node in the tree whose name equals `name`.
    pub fn find_commands<'a>(&'a self, name: &'a str) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.inner.find_commands(name)
    }

    /// Iterate over every environment node in the tree whose name equals `name`.
    pub fn find_environments<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.inner.find_environments(name)
    }

    /// Create a detached single-character node.
    ///
    /// The node is staged but not attached; attach it with
    /// [`append_child`](Self::append_child) or a sibling-insert method.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] if the document has errors.
    pub fn create_char(&mut self, c: char) -> Result<NodeId, EditError> {
        self.inner.create_char(c)
    }

    /// Create a detached text node.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] if the document has errors.
    pub fn create_text(&mut self, s: impl Into<String>) -> Result<NodeId, EditError> {
        self.inner.create_text(s)
    }

    /// Create a detached active-space node (an explicit space token).
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] if the document has errors.
    pub fn create_active_space(&mut self) -> Result<NodeId, EditError> {
        self.inner.create_active_space()
    }

    /// Create a detached, empty brace group with the given content mode.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] if the document has errors.
    pub fn create_group(
        &mut self,
        mode: texform_core::parse::ContentMode,
    ) -> Result<NodeId, EditError> {
        self.inner.create_group(mode)
    }

    /// Create a detached command node with the given name and arguments.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] if the document has errors, or an
    /// argument-shape error if `args` do not match a valid slot layout.
    pub fn create_command(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_command(name, args)
    }

    /// Create a detached declarative command node (such as a font declaration)
    /// with the given name and arguments.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] if the document has errors, or an
    /// argument-shape error for invalid `args`.
    pub fn create_declarative(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_declarative(name, args)
    }

    /// Create a detached environment node wrapping `body` (which must be a group).
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] if the document has errors,
    /// [`EditError::ForeignNode`] if `body` belongs to another document, or an
    /// argument/container-shape error for invalid inputs.
    pub fn create_environment(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
        body: NodeId,
    ) -> Result<NodeId, EditError> {
        self.inner.create_environment(name, args, body)
    }

    /// Append `child` as the last child of `parent`.
    ///
    /// `child` must be a detached node and `parent` a container (a `Root` or
    /// `Group`).
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, either node is
    /// foreign or missing, `parent` is not a container, or the move would create
    /// a cycle.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), EditError> {
        self.inner.append_child(parent, child)
    }

    /// Insert `new` immediately before the sibling `anchor`.
    ///
    /// `new` must be a detached node and `anchor` an attached group child.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, either node is
    /// foreign or missing, `anchor` has no parent (such as the root), or the
    /// move would create a cycle.
    pub fn insert_before(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.inner.insert_before(anchor, new)
    }

    /// Insert `new` immediately after the sibling `anchor`.
    ///
    /// # Errors
    ///
    /// Same conditions as [`insert_before`](Self::insert_before).
    pub fn insert_after(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.inner.insert_after(anchor, new)
    }

    /// Insert `child` at position `index` among `parent`'s children.
    ///
    /// `child` must be a detached node and `parent` a container (a `Root` or
    /// `Group`).
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, either node is
    /// foreign or missing, `parent` is not a container, `index` is out of
    /// range, or the move would create a cycle.
    pub fn insert_child(
        &mut self,
        parent: NodeId,
        index: usize,
        child: NodeId,
    ) -> Result<(), EditError> {
        self.inner.insert_child(parent, index, child)
    }

    /// Replace `target` in place with `replacement`.
    ///
    /// `target` must be an attached non-root node and `replacement` a detached
    /// node.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, either node is
    /// foreign or missing, `target` is the root, or the move would create a
    /// cycle.
    pub fn replace_with(&mut self, target: NodeId, replacement: NodeId) -> Result<(), EditError> {
        self.inner.replace_with(target, replacement)
    }

    /// Wrap `target` in the container `wrapper`, returning the wrapper's id.
    ///
    /// `target` must be an attached group child and `wrapper` a detached
    /// container (a staged `Root` or `Group`). The wrapper takes `target`'s
    /// place in the tree and `target` becomes its child.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, either node is
    /// foreign or missing, `wrapper` is not a container, or `target` is the root.
    pub fn wrap(&mut self, target: NodeId, wrapper: NodeId) -> Result<NodeId, EditError> {
        self.inner.wrap(target, wrapper)
    }

    /// Unwrap a group, splicing its children into its parent in place.
    ///
    /// Returns the ids of the spliced-in children.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, `group` is
    /// foreign, missing, not a group, or the root.
    pub fn unwrap(&mut self, group: NodeId) -> Result<Vec<NodeId>, EditError> {
        self.inner.unwrap(group)
    }

    /// Detach the subtree rooted at `id` from the tree, returning its id.
    ///
    /// The subtree is removed from its parent but kept alive, so it can be
    /// re-attached elsewhere.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, or `id` is
    /// foreign, missing, or the root.
    pub fn extract(&mut self, id: NodeId) -> Result<NodeId, EditError> {
        self.inner.extract(id)
    }

    /// Remove the subtree rooted at `id` from the tree and discard it.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, or `id` is
    /// foreign, missing, or the root.
    pub fn remove(&mut self, id: NodeId) -> Result<(), EditError> {
        self.inner.remove(id)
    }

    /// Remove all children of the container `container`.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, or `container` is
    /// foreign, missing, or not a container.
    pub fn clear(&mut self, container: NodeId) -> Result<(), EditError> {
        self.inner.clear(container)
    }

    /// Rename the command node `id` to `name`.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, or `id` is
    /// foreign, missing, or not a command node.
    pub fn set_command_name(
        &mut self,
        id: NodeId,
        name: impl Into<String>,
    ) -> Result<(), EditError> {
        self.inner.set_command_name(id, name)
    }

    /// Set the content of the text node `id`.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, or `id` is
    /// foreign, missing, or not a text node.
    pub fn set_text(&mut self, id: NodeId, s: impl Into<String>) -> Result<(), EditError> {
        self.inner.set_text(id, s)
    }

    /// Set the character of the char node `id`.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, or `id` is
    /// foreign, missing, or not a char node.
    pub fn set_char(&mut self, id: NodeId, c: char) -> Result<(), EditError> {
        self.inner.set_char(id, c)
    }

    /// Set the argument at `index` of the command or environment node `id`.
    ///
    /// # Errors
    ///
    /// Returns an [`EditError`] if the document is read-only, `id` is foreign,
    /// missing, or has no argument slot at `index`, or `value` does not match
    /// the slot shape.
    pub fn set_arg(&mut self, id: NodeId, index: usize, value: ArgValue) -> Result<(), EditError> {
        self.inner.set_arg(id, index, value)
    }

    /// Export the parse-time span side table as a list of `(path, span)` entries.
    ///
    /// Paths follow the parser's tree-path scheme rooted at `root`:
    /// `.child.N` for container children, `.arg.N.content` for content-carrying
    /// argument slots, `.left` / `.right` for infix operands, `.body` for
    /// environment bodies, and `.base` / `.sub` / `.sup` for script slots.
    /// Nodes without a recorded span (e.g. created by edits, or any node of a
    /// document built without parser spans) are omitted. Spans reflect the
    /// original parse and are not updated by document edits.
    pub fn node_spans(&self) -> Vec<NodeSpanEntry> {
        self.inner
            .node_spans()
            .into_iter()
            .map(|(id, span)| NodeSpanEntry { id, span })
            .collect()
    }

    /// Convert the tree to a [`SyntaxNode`](texform_interface::syntax_node::SyntaxNode),
    /// the single serde wire format.
    ///
    /// Use this for structured-data output (JSON, transport across a binding);
    /// for LaTeX text use [`to_latex`](Self::to_latex).
    pub fn to_syntax(&self) -> texform_interface::syntax_node::SyntaxNode {
        self.inner.to_syntax()
    }

    /// Serialize the tree to canonical LaTeX text.
    ///
    /// The canonical serializer guarantees text idempotency: re-parsing and
    /// re-serializing the output yields the same string. Error nodes round-trip
    /// their captured source snippet.
    ///
    /// # Errors
    ///
    /// Returns [`SerializeError`] if a node cannot be serialized.
    pub fn to_latex(&self) -> Result<String, SerializeError> {
        self.inner.to_latex()
    }

    /// Serialize the tree to LaTeX text with explicit [`SerializeOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`SerializeError`] if a node cannot be serialized.
    pub fn to_latex_with(&self, options: &SerializeOptions) -> Result<String, SerializeError> {
        self.inner.to_latex_with(options)
    }

    /// Serialize to canonical LaTeX and record typed output tokens in the same pass.
    ///
    /// Every token has a closed semantic kind and a UTF-8 byte span into `latex`.
    /// The token stream is not an error-node inventory: use [`Self::has_errors`]
    /// because an empty error snippet deliberately produces no zero-width token.
    ///
    /// # Errors
    ///
    /// Returns [`SerializeError`] if a node cannot be serialized.
    pub fn to_tokenized_latex(&self) -> Result<TokenizedLatex, SerializeError> {
        self.inner.to_tokenized_latex()
    }

    /// Serialize with explicit options and record typed output tokens in the same pass.
    ///
    /// The returned `latex` is byte-for-byte identical to [`Self::to_latex_with`]
    /// with the same options. Token spans are UTF-8 byte offsets.
    ///
    /// # Errors
    ///
    /// Returns [`SerializeError`] if a node cannot be serialized.
    pub fn to_tokenized_latex_with(
        &self,
        options: &SerializeOptions,
    ) -> Result<TokenizedLatex, SerializeError> {
        self.inner.to_tokenized_latex_with(options)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.to_latex() {
            Ok(latex) => f.write_str(&latex),
            Err(_) => Err(std::fmt::Error),
        }
    }
}
