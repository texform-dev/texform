use super::*;

impl Document {
    /// Append a detached node to a root/group container.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        self.insert_child_at(parent, self.child_len(parent)?, child)
    }

    /// Insert a detached node before an attached group child.
    pub fn insert_before(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let (parent, index) = self.group_child_position(self.check_node_owner(anchor)?)?;
        self.insert_child_at(NodeId::new(self.id, parent), index, new)
    }

    /// Insert a detached node after an attached group child.
    pub fn insert_after(&mut self, anchor: NodeId, new: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let (parent, index) = self.group_child_position(self.check_node_owner(anchor)?)?;
        self.insert_child_at(NodeId::new(self.id, parent), index + 1, new)
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
        let raw = self.detach_group_child(id)?;
        self.debug_assert_conformance();
        Ok(NodeId::new(self.id, raw))
    }

    /// Remove an attached group child and its subtree.
    pub fn remove(&mut self, id: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.detach_group_child(id)?;
        self.ast.remove_detached(raw);
        self.detached_modes.remove(raw);
        self.debug_assert_conformance();
        Ok(())
    }

    fn detach_group_child(&mut self, id: NodeId) -> Result<RawNodeId, EditError> {
        let raw = self.check_node_owner(id)?;
        if raw == self.ast.root() {
            return Err(EditError::CannotEditRoot);
        }
        let Some(ParentLink {
            parent,
            slot: Slot::GroupChild(_),
        }) = self.ast.parent(raw)
        else {
            return Err(EditError::SlotShapeMismatch {
                expected: "group child",
            });
        };
        let mut proposed = self.ast.node(parent).clone();
        Self::container_children_mut(&mut proposed).retain(|child| *child != raw);
        self.check_edited_node(parent, &proposed)?;
        let mode = self.context_mode(raw);
        self.ast.detach(raw);
        self.detached_modes.insert(raw, mode);
        Ok(raw)
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
        let link = self.ast.parent(target).ok_or(EditError::NodeNotFound)?;
        let mut proposed = self.ast.node(link.parent).clone();
        *proposed.child_mut(link.slot) = replacement;
        self.check_edited_node(link.parent, &proposed)?;
        self.ast.replace_content_child(target, replacement);
        self.ast.remove_detached(target);
        self.detached_modes.remove(replacement);
        self.debug_assert_conformance();
        Ok(())
    }

    /// Remove all direct children from a root/group container.
    pub fn clear(&mut self, container: NodeId) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(container)?;
        self.check_container(raw)?;
        let mut proposed = self.ast.node(raw).clone();
        Self::container_children_mut(&mut proposed).clear();
        self.check_edited_node(raw, &proposed)?;
        let len = self.ast.children(raw).len();
        for child in self.ast.detach_children_range(raw, 0..len) {
            self.ast.remove_detached(child);
        }
        self.debug_assert_conformance();
        Ok(())
    }

    /// Set the name of a command, infix, or declarative node.
    pub fn set_command_name(
        &mut self,
        id: NodeId,
        name: impl Into<String>,
    ) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        let name = name.into();
        let mut proposed = self.ast.node(raw).clone();
        match &mut proposed {
            Node::Command {
                name: current,
                known,
                ..
            } => {
                *known =
                    conformance::command_known(&self.knowledge_base, &name, self.context_mode(raw));
                *current = name;
            }
            Node::Infix { name: current, .. } | Node::Declarative { name: current, .. } => {
                *current = name
            }
            _ => {
                return Err(EditError::SlotShapeMismatch {
                    expected: "command-like node",
                });
            }
        }
        self.commit_payload(raw, proposed)
    }

    /// Set an environment name while preserving its arguments and body.
    pub fn set_env_name(&mut self, id: NodeId, name: impl Into<String>) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        let name = name.into();
        let mut proposed = self.ast.node(raw).clone();
        let Node::Environment {
            name: current,
            known,
            ..
        } = &mut proposed
        else {
            return Err(EditError::SlotShapeMismatch {
                expected: "environment",
            });
        };
        *known = self
            .knowledge_base
            .lookup_env(&name, self.context_mode(raw))
            .is_some();
        *current = name;
        self.commit_payload(raw, proposed)
    }

    /// Set the payload of a text node.
    pub fn set_text(&mut self, id: NodeId, s: impl Into<String>) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        if !matches!(self.ast.node(raw), Node::Text(_)) {
            return Err(EditError::SlotShapeMismatch {
                expected: "text node",
            });
        }
        self.commit_payload(raw, Node::Text(s.into()))
    }

    /// Set the character of a char node.
    pub fn set_char(&mut self, id: NodeId, c: char) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        if !matches!(self.ast.node(raw), Node::Char(_)) {
            return Err(EditError::SlotShapeMismatch {
                expected: "char node",
            });
        }
        self.commit_payload(raw, Node::Char(c))
    }

    /// Replace an argument, preserving existing paired delimiters for ordinary values.
    pub fn set_arg(
        &mut self,
        id: NodeId,
        index: usize,
        value: impl Into<Arg>,
    ) -> Result<(), EditError> {
        self.check_writable()?;
        let raw = self.check_node_owner(id)?;
        let node = self.ast.node(raw);
        if !matches!(
            node,
            Node::Command { .. }
                | Node::Infix { .. }
                | Node::Declarative { .. }
                | Node::Environment { .. }
        ) {
            return Err(EditError::SlotShapeMismatch {
                expected: "command-like node",
            });
        }
        let previous = node
            .arg_slots()
            .get(index)
            .ok_or(EditError::IndexOutOfBounds)?;
        let spec = conformance::signature(node, self.context_mode(raw), &self.knowledge_base)
            .and_then(|signature| signature.args.get(index))
            .ok_or(EditError::IndexOutOfBounds)?;
        let old_kind = previous.as_ref().map(|arg| arg.kind.clone());
        let old_content = previous
            .as_ref()
            .and_then(|arg| arg.value.content())
            .map(|(id, _)| id);
        let value = value.into();
        self.transact(|doc, staged| {
            let slot = doc
                .argument(staged, value, spec, old_kind.as_ref())
                .map_err(|error| {
                    error.under(&format!("{}.arg.{index}", doc.conformance_path(raw)))
                })?;
            if let Some((content, _)) = slot.as_ref().and_then(|arg| arg.value.content()) {
                doc.check_no_cycle(content, raw)?;
            }
            let mut proposed = doc.ast.node(raw).clone();
            match &mut proposed {
                Node::Command { args, .. }
                | Node::Infix { args, .. }
                | Node::Declarative { args, .. }
                | Node::Environment { args, .. } => args[index] = slot,
                _ => unreachable!("argument owner was checked"),
            }
            doc.check_edited_node(raw, &proposed)?;
            for (child, _) in Ast::node_edges(&proposed) {
                doc.detached_modes.remove(child);
            }
            doc.ast.replace_node(raw, proposed);
            if let Some(old) = old_content {
                doc.ast.remove_detached(old);
            }
            doc.debug_assert_conformance();
            Ok(())
        })
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
        let (parent, index) = self.group_child_position(target)?;
        self.check_no_cycle(wrapper, parent)?;
        let mut proposed_wrapper = self.ast.node(wrapper).clone();
        Self::container_children_mut(&mut proposed_wrapper).push(target);
        self.check_edited_node(wrapper, &proposed_wrapper)?;
        let mut proposed_parent = self.ast.node(parent).clone();
        Self::container_children_mut(&mut proposed_parent)[index] = wrapper;
        self.check_edited_node(parent, &proposed_parent)?;
        let target = self.ast.detach(target);
        self.ast.insert_child(parent, index, wrapper);
        self.ast.append_child(wrapper, target);
        self.detached_modes.remove(wrapper);
        self.debug_assert_conformance();
        Ok(NodeId::new(self.id, wrapper))
    }

    /// Remove a group-child group and splice its children into the parent.
    pub fn unwrap(&mut self, group: NodeId) -> Result<Vec<NodeId>, EditError> {
        self.check_writable()?;
        let group = self.check_node_owner(group)?;
        if group == self.ast.root() {
            return Err(EditError::CannotEditRoot);
        }
        if !matches!(self.ast.node(group), Node::Group { .. }) {
            return Err(EditError::SlotShapeMismatch { expected: "group" });
        }
        let (parent, index) = self.group_child_position(group)?;
        let mut proposed = self.ast.node(parent).clone();
        Self::container_children_mut(&mut proposed)
            .splice(index..=index, self.ast.children(group).iter().copied());
        self.check_edited_node(parent, &proposed)?;
        let count = self.ast.children(group).len();
        let children = self.ast.detach_children_range(group, 0..count);
        let detached_group = self.ast.detach(group);
        self.ast.remove_detached(detached_group);
        for (offset, child) in children.iter().copied().enumerate() {
            self.ast.insert_child(parent, index + offset, child);
        }
        self.debug_assert_conformance();
        Ok(children
            .into_iter()
            .map(|raw| NodeId::new(self.id, raw))
            .collect())
    }

    fn insert_child_at(
        &mut self,
        parent: NodeId,
        index: usize,
        child: NodeId,
    ) -> Result<(), EditError> {
        let parent = self.check_node_owner(parent)?;
        let child = self.check_node_owner(child)?;
        self.check_container(parent)?;
        self.check_detached(child)?;
        self.check_no_cycle(child, parent)?;
        if index > self.ast.children(parent).len() {
            return Err(EditError::IndexOutOfBounds);
        }
        let mut proposed = self.ast.node(parent).clone();
        Self::container_children_mut(&mut proposed).insert(index, child);
        self.check_edited_node(parent, &proposed)?;
        self.ast.insert_child(parent, index, child);
        self.detached_modes.remove(child);
        self.debug_assert_conformance();
        Ok(())
    }

    fn container_children_mut(node: &mut Node) -> &mut Vec<RawNodeId> {
        match node {
            Node::Root { children, .. } | Node::Group { children, .. } => children,
            _ => unreachable!("container shape was checked before editing"),
        }
    }

    /// Check `proposed` as the new payload of `raw`, and re-check the parent
    /// when the change is visible to the parent's local rules.
    ///
    /// Moved-in children are compared by their recorded or container context,
    /// so the cost is one local check per changed node plus a sibling scan.
    fn check_edited_node(&self, raw: RawNodeId, proposed: &Node) -> Result<(), EditError> {
        self.check_local(proposed, self.context_mode(raw), None)
            .map_err(|error| error.under(&self.conformance_path(raw)))?;
        if let Some(parent) = self.ast.parent_id(raw)
            && conformance::parent_view(self.ast.node(raw)) != conformance::parent_view(proposed)
        {
            self.check_local(
                self.ast.node(parent),
                self.context_mode(parent),
                Some((raw, proposed)),
            )
            .map_err(|error| error.under(&self.conformance_path(parent)))?;
        }
        Ok(())
    }

    fn commit_payload(&mut self, raw: RawNodeId, proposed: Node) -> Result<(), EditError> {
        self.check_edited_node(raw, &proposed)?;
        self.ast.replace_node(raw, proposed);
        self.debug_assert_conformance();
        Ok(())
    }
}
