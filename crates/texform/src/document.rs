pub use texform_core::document::{
    ArgRef, ArgValue, DelimiterRef, DelimiterValue, DocumentId, EditError, FromSyntaxError,
    GroupKindRef, NodeId, NodeKind, NodeRef,
};
pub use texform_core::serialize::SerializeOptions;

pub use crate::serialize::SerializeError;

#[derive(Clone, Debug)]
pub struct Document {
    inner: texform_core::document::Document,
}

impl Document {
    pub fn new() -> Self {
        Self::from_core(texform_core::document::Document::new())
    }

    pub fn with_mode(mode: texform_core::parse::ContentMode) -> Self {
        Self::from_core(texform_core::document::Document::with_mode(mode))
    }

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

    pub fn root(&self) -> NodeRef<'_> {
        self.inner.root()
    }

    pub fn node(&self, id: NodeId) -> Result<NodeRef<'_>, EditError> {
        self.inner.node(id)
    }

    pub fn has_errors(&self) -> bool {
        self.inner.has_errors()
    }

    pub fn is_read_only(&self) -> bool {
        self.inner.is_read_only()
    }

    pub fn errors(&self) -> impl Iterator<Item = NodeRef<'_>> + '_ {
        self.inner.errors()
    }

    pub fn find<'a>(
        &'a self,
        start: NodeRef<'a>,
        pred: impl Fn(NodeRef<'a>) -> bool + 'a,
    ) -> Option<NodeRef<'a>> {
        self.inner.find(start, pred)
    }

    pub fn find_all<'a>(
        &'a self,
        start: NodeRef<'a>,
        pred: impl Fn(NodeRef<'a>) -> bool + 'a,
    ) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.inner.find_all(start, pred)
    }

    pub fn find_commands<'a>(&'a self, name: &'a str) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.inner.find_commands(name)
    }

    pub fn find_environments<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.inner.find_environments(name)
    }

    pub fn create_char(&mut self, c: char) -> Result<NodeId, EditError> {
        self.inner.create_char(c)
    }

    pub fn create_text(&mut self, s: impl Into<String>) -> Result<NodeId, EditError> {
        self.inner.create_text(s)
    }

    pub fn create_active_space(&mut self) -> Result<NodeId, EditError> {
        self.inner.create_active_space()
    }

    pub fn create_group(
        &mut self,
        mode: texform_core::parse::ContentMode,
    ) -> Result<NodeId, EditError> {
        self.inner.create_group(mode)
    }

    pub fn create_command(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_command(name, args)
    }

    pub fn create_declarative(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
    ) -> Result<NodeId, EditError> {
        self.inner.create_declarative(name, args)
    }

    pub fn create_environment(
        &mut self,
        name: impl Into<String>,
        args: Vec<ArgValue>,
        body: NodeId,
    ) -> Result<NodeId, EditError> {
        self.inner.create_environment(name, args, body)
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), EditError> {
        self.inner.append_child(parent, child)
    }

    pub fn insert_before(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.inner.insert_before(anchor, new)
    }

    pub fn insert_after(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.inner.insert_after(anchor, new)
    }

    pub fn insert_child(
        &mut self,
        parent: NodeId,
        index: usize,
        child: NodeId,
    ) -> Result<(), EditError> {
        self.inner.insert_child(parent, index, child)
    }

    pub fn replace_with(&mut self, target: NodeId, replacement: NodeId) -> Result<(), EditError> {
        self.inner.replace_with(target, replacement)
    }

    pub fn wrap(&mut self, target: NodeId, wrapper: NodeId) -> Result<NodeId, EditError> {
        self.inner.wrap(target, wrapper)
    }

    pub fn unwrap(&mut self, group: NodeId) -> Result<Vec<NodeId>, EditError> {
        self.inner.unwrap(group)
    }

    pub fn extract(&mut self, id: NodeId) -> Result<NodeId, EditError> {
        self.inner.extract(id)
    }

    pub fn remove(&mut self, id: NodeId) -> Result<(), EditError> {
        self.inner.remove(id)
    }

    pub fn clear(&mut self, container: NodeId) -> Result<(), EditError> {
        self.inner.clear(container)
    }

    pub fn set_command_name(
        &mut self,
        id: NodeId,
        name: impl Into<String>,
    ) -> Result<(), EditError> {
        self.inner.set_command_name(id, name)
    }

    pub fn set_text(&mut self, id: NodeId, s: impl Into<String>) -> Result<(), EditError> {
        self.inner.set_text(id, s)
    }

    pub fn set_char(&mut self, id: NodeId, c: char) -> Result<(), EditError> {
        self.inner.set_char(id, c)
    }

    pub fn set_arg(&mut self, id: NodeId, index: usize, value: ArgValue) -> Result<(), EditError> {
        self.inner.set_arg(id, index, value)
    }

    pub fn to_syntax(&self) -> texform_interface::syntax_node::SyntaxNode {
        self.inner.to_syntax()
    }

    pub fn to_latex(&self) -> Result<String, SerializeError> {
        self.inner.to_latex()
    }

    pub fn to_latex_with(&self, options: &SerializeOptions) -> Result<String, SerializeError> {
        self.inner.to_latex_with(options)
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
