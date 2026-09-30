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
    Arg, ArgKindRef, ArgRef, ConformanceError, ConformanceRule, DelimiterRef, DelimiterValue,
    DocumentId, EditError, FromSyntaxError, GroupKindRef, InMode, NodeId, NodeKind, NodeRef,
    NodeSlot,
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
/// Every document shares an immutable [`KnowledgeBase`](crate::KnowledgeBase).
/// A transform engine accepts complete documents sharing its knowledge-base instance.
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
    /// The document uses the shared default knowledge base.
    pub fn new() -> Self {
        Self::from_core(texform_core::document::Document::new())
    }

    /// Create an empty document with an explicit root content mode.
    ///
    /// The document uses the shared default knowledge base.
    pub fn with_mode(mode: texform_core::parse::ContentMode) -> Self {
        Self::from_core(texform_core::document::Document::with_mode(mode))
    }

    /// Build a document from a syntax tree.
    ///
    /// This path validates the tree structure and uses the shared default knowledge base.
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

    /// The immutable knowledge base bound to this document.
    pub fn knowledge_base(&self) -> &crate::KnowledgeBase {
        self.inner.knowledge_base()
    }

    /// Create an empty document bound to an existing knowledge-base instance.
    pub fn with_knowledge_base(
        knowledge_base: &crate::KnowledgeBase,
        mode: crate::ContentMode,
    ) -> Self {
        Self::from_core(texform_core::document::Document::with_knowledge_base(
            knowledge_base,
            mode,
        ))
    }

    /// Import a syntax tree using an existing knowledge-base instance.
    pub fn from_syntax_with(
        knowledge_base: &crate::KnowledgeBase,
        node: &texform_interface::syntax_node::SyntaxNode,
    ) -> Result<Self, FromSyntaxError> {
        Ok(Self::from_core(
            texform_core::document::Document::from_syntax_with(knowledge_base, node)?,
        ))
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

    /// Create a detached character leaf in the root's mode.
    ///
    /// `'&'` is a literal ampersand written `\&`; use
    /// [`create_alignment_tab`](Self::create_alignment_tab) for a cell separator.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] for a document with errors, or a
    /// conformance error when the character cannot be written in that mode,
    /// such as `\`, `^`, `~`, or a math-mode space.
    pub fn create_char(&mut self, value: char) -> Result<NodeId, EditError> {
        self.inner.create_char(value)
    }

    /// Create a detached text run in the root's mode, which must be text.
    ///
    /// # Errors
    ///
    /// Returns a conformance error in math mode, for any of `% $ & # _ { }`
    /// (which are separate [`create_char`](Self::create_char) nodes), or for
    /// characters and runs of spaces the lexer would not read back.
    pub fn create_text(&mut self, value: impl Into<String>) -> Result<NodeId, EditError> {
        self.inner.create_text(value)
    }

    /// Create a detached `~` active space in the root's mode.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::ReadOnlyDocument`] for a document with errors.
    pub fn create_active_space(&mut self) -> Result<NodeId, EditError> {
        self.inner.create_active_space()
    }

    /// Create a detached alignment tab, the unescaped `&` separating cells.
    ///
    /// # Errors
    ///
    /// Returns a conformance error unless the root is in math mode.
    pub fn create_alignment_tab(&mut self) -> Result<NodeId, EditError> {
        self.inner.create_alignment_tab()
    }

    /// Create a detached run of `count` primes in math mode.
    ///
    /// # Errors
    ///
    /// Returns a conformance error when `count` is zero.
    pub fn create_prime(&mut self, count: usize) -> Result<NodeId, EditError> {
        self.inner.create_prime(count)
    }

    /// Create a detached brace group of `mode`, in a `mode` context.
    ///
    /// Each child is a detached node or source text parsed in `mode`.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::InvalidSource`] for unparsable source, an error for
    /// attached, foreign, or repeated children, or a conformance error for a
    /// child built in another mode or an infix with siblings.
    pub fn create_group(
        &mut self,
        mode: texform_core::parse::ContentMode,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_group(mode, children)
    }

    /// Create a detached `$...$` group of math `children` in a text context.
    ///
    /// # Errors
    ///
    /// Same conditions as [`create_group`](Self::create_group).
    pub fn create_inline_math(
        &mut self,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_inline_math(children)
    }

    /// Create a detached `\left...\right` group in math mode.
    ///
    /// # Errors
    ///
    /// Same conditions as [`create_group`](Self::create_group), plus a
    /// conformance error for a delimiter that is neither `.` nor registered in
    /// the knowledge base.
    pub fn create_delimited_group(
        &mut self,
        left: DelimiterValue,
        right: DelimiterValue,
        children: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_delimited_group(left, right, children)
    }

    /// Create a detached scripted node in math mode.
    ///
    /// # Errors
    ///
    /// Returns a conformance error when both scripts are absent or when the
    /// base is a scripted node, an infix, or an alignment tab.
    pub fn create_scripted(
        &mut self,
        base: impl Into<Arg>,
        sub: Option<Arg>,
        sup: Option<Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_scripted(base, sub, sup)
    }

    /// Create a detached prefix command in the root's mode.
    ///
    /// `args` covers every signature slot, or only the required slots. Names
    /// missing from the knowledge base create unknown commands without
    /// arguments.
    ///
    /// # Errors
    ///
    /// Returns a conformance error when the name's record is not a prefix
    /// command (naming the constructor to use instead, such as
    /// [`create_declarative`](Self::create_declarative) for `bf`), for a wrong
    /// argument count, or for an argument that does not fit its slot, and
    /// [`EditError::InvalidSource`] for unparsable source arguments.
    pub fn create_command(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_command(name, args)
    }

    /// Create a detached declarative command such as `\bf` in the root's mode.
    ///
    /// # Errors
    ///
    /// Same conditions as [`create_command`](Self::create_command), except the
    /// record must be declarative.
    pub fn create_declarative(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_declarative(name, args)
    }

    /// Create a detached infix command such as `\over` in math mode.
    ///
    /// `args` fills extra slots, such as the dimension of `\above`.
    ///
    /// # Errors
    ///
    /// Same conditions as [`create_command`](Self::create_command), except the
    /// record must be infix.
    pub fn create_infix(
        &mut self,
        name: impl Into<String>,
        left: impl Into<Arg>,
        right: impl Into<Arg>,
        args: impl IntoIterator<Item = Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_infix(name, left, right, args)
    }

    /// Create a detached environment in the root's mode.
    ///
    /// `body` is a detached group, source parsed in the recorded body mode, or
    /// [`Arg::Absent`] for an empty body; unknown environments take no
    /// arguments and use the surrounding mode.
    ///
    /// # Errors
    ///
    /// Returns a conformance error for an argument that does not fit the
    /// signature or a body that is not a group in the body mode, and
    /// [`EditError::InvalidSource`] for unparsable source.
    pub fn create_environment(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
        body: impl Into<Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_environment(name, args, body)
    }

    /// Create a detached environment whose implicit body group holds `children`.
    ///
    /// Language bindings use this for a list body.
    #[doc(hidden)]
    pub fn create_environment_with_children(
        &mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = Arg>,
        children: Vec<Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner
            .create_environment_with_children(name, args, children)
    }

    /// Parse `source` into a detached Implicit group, in `mode` or the root's mode.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::InvalidSource`] with the parser diagnostics when the
    /// source does not parse cleanly.
    pub fn parse_fragment(
        &mut self,
        source: &str,
        mode: Option<texform_core::parse::ContentMode>,
    ) -> Result<NodeId, EditError> {
        self.inner.parse_fragment(source, mode)
    }

    /// Construct nodes in an explicit context mode without changing the document.
    ///
    /// The view's mode replaces each constructor's default context; a node
    /// that cannot appear in that mode is rejected.
    pub fn in_mode(&mut self, mode: texform_core::parse::ContentMode) -> InMode<'_> {
        self.inner.in_mode(mode)
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

    /// Rename an environment while preserving its existing signature and body mode.
    pub fn set_env_name(&mut self, id: NodeId, name: impl Into<String>) -> Result<(), EditError> {
        self.inner.set_env_name(id, name)
    }

    #[doc(hidden)]
    pub fn __validate_conformance(&self) -> Result<(), ConformanceError> {
        self.inner.__validate_conformance()
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
    pub fn set_arg(&mut self, id: NodeId, index: usize, value: Arg) -> Result<(), EditError> {
        self.inner.set_arg(id, index, value)
    }

    /// Resolve a current rooted tree path.
    pub fn node_at(&self, path: &str) -> Result<NodeRef<'_>, EditError> {
        self.inner.node_at(path)
    }

    /// Deep-copy a subtree within this document.
    pub fn clone_node(&mut self, node: NodeId) -> Result<NodeId, EditError> {
        self.inner.clone_node(node)
    }

    /// Import a subtree, validating it against this document's knowledge base.
    pub fn import_node(&mut self, other: &Document, node: NodeId) -> Result<NodeId, EditError> {
        self.inner.import_node(&other.inner, node)
    }

    /// Replace a subscript, resolving an existing scripted base automatically.
    pub fn set_subscript(
        &mut self,
        target: NodeId,
        value: Option<Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.set_subscript(target, value)
    }

    /// Replace a superscript, returning the wrapper or its collapsed base.
    pub fn set_superscript(
        &mut self,
        target: NodeId,
        value: Option<Arg>,
    ) -> Result<NodeId, EditError> {
        self.inner.set_superscript(target, value)
    }

    /// Change only the boundaries of a filled paired argument.
    pub fn set_arg_delimiters(
        &mut self,
        node: NodeId,
        index: usize,
        open: impl AsRef<str>,
        close: impl AsRef<str>,
    ) -> Result<(), EditError> {
        self.inner.set_arg_delimiters(node, index, open, close)
    }

    /// Change the boundaries of a delimited group.
    pub fn set_delimiters(
        &mut self,
        node: NodeId,
        left: impl AsRef<str>,
        right: impl AsRef<str>,
    ) -> Result<(), EditError> {
        self.inner.set_delimiters(node, left, right)
    }

    /// Set a positive prime count.
    pub fn set_prime_count(&mut self, node: NodeId, count: usize) -> Result<(), EditError> {
        self.inner.set_prime_count(node, count)
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
    /// Parsed and normalized output retains the existing text-idempotency
    /// contract. Arbitrarily constructed or edited trees need not serialize
    /// to a parse/serialize fixed point. Error nodes preserve their snippet.
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
