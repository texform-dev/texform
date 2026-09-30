use super::*;

impl Document {
    /// Deep-copy a subtree into this document, preserving its context mode.
    ///
    /// The returned subtree is detached. Copying the document root produces an
    /// Implicit group containing its children rather than a second root.
    pub fn clone_node(&mut self, node: NodeId) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(node)?;
        let mode = self.context_mode(raw);
        let copied = self.ast.clone_subtree(raw);
        self.detached_modes.insert(copied, mode);
        self.debug_assert_conformance();
        Ok(NodeId::new(self.id, copied))
    }

    /// Deep-copy a subtree from another document, preserving its context mode.
    ///
    /// Error nodes are rejected. Unless the source is complete and shares this
    /// document's knowledge base, the entire subtree is checked against the
    /// destination knowledge before any nodes are allocated. Importing a root
    /// produces a detached Implicit group.
    pub fn import_node(&mut self, other: &Document, node: NodeId) -> Result<NodeId, EditError> {
        self.check_writable()?;
        let raw = other.check_node_owner(node)?;
        let mode = other.context_mode(raw);
        if other.has_errors
            && let Some(error) = other
                .ast
                .find(raw, |node| matches!(node, Node::Error { .. }))
        {
            return Err(ConformanceError::new(
                ConformanceRule::ErrorNode,
                "error nodes cannot be imported into an editable document",
            )
            .under(&path::path_below(&other.ast, raw, error, "detached"))
            .into());
        }
        if other.has_errors || !self.knowledge_base.ptr_eq(&other.knowledge_base) {
            conformance::check_tree(&other.ast, raw, mode, &self.knowledge_base, "detached")?;
        }
        let copied = self.ast.copy_subtree_from(&other.ast, raw);
        self.detached_modes.insert(copied, mode);
        self.debug_assert_conformance();
        Ok(NodeId::new(self.id, copied))
    }
}
