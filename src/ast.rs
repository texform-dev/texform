//! AST module with BeautifulSoup-like navigation API
//!
//! Uses arena-based tree management with slotmap for efficient node access.

use slotmap::{HopSlotMap, SecondaryMap, new_key_type};

// ============ Core Types ============

new_key_type! {
    /// Unique identifier for AST nodes
    pub struct NodeId;
}

/// Describes how a node is referenced by its parent
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParentLink {
    /// Parent node ID
    pub parent: NodeId,
    /// The "slot" this child occupies in the parent
    pub slot: Slot,
}

/// Defines the "slot" a child node occupies relative to its parent
///
/// Type-safe distinction between vector-based slots (with index) and single functional slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    GroupChild(usize),
    Argument(usize),
    ScriptBase,
    ScriptSub,
    ScriptSup,
    InfixLeft,
    InfixRight,
    DeclarativeScope,
    EnvBody,
}

/// Content mode for groups
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentMode {
    Math,
    Text,
}

/// Delimiter type for delimited groups (\left ... \right)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delimiter {
    /// No delimiter (corresponds to '.' in LaTeX: \left. or \right.)
    None,
    /// Single character delimiter: '(', ')', '[', ']', '|', etc.
    Char(char),
    /// Control sequence delimiter: "\langle", "\rangle", "\{", "\}", etc.
    /// Stored as owned String to allow for arbitrary control sequences
    Control(String),
}

/// Group type for serialization
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupKind {
    Explicit,
    /// Used to wrap operands/scope of infix and declarative commands
    Implicit,
    Delimited {
        left: Delimiter,
        right: Delimiter,
    },
    /// Inline math in text mode: $...$
    InlineMath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentKind {
    /// Mandatory argument: {...}
    Mandatory,
    /// Optional argument: [...]
    Optional,
}

/// Command argument - an "edge attribute" pointing to content via NodeId
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Argument {
    pub kind: ArgumentKind,
    pub content: NodeId,
}

// ============ Node Types ============

/// AST node data
///
/// Each variant stores "downward" links via NodeId.
/// "Upward" links are maintained separately in Ast::parent.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    // ============ Leaf Nodes ============
    /// Single character (letter, digit, symbol)
    /// Example: a, 1, +, α
    Char(char),

    /// Text string (for \text{...} content or optimized consecutive chars)
    /// Used in text mode or when consolidating multiple characters
    Text(String),

    // ============ Container Nodes ============
    /// Prefix command: arguments follow the command
    /// Example: \frac{a}{b}, \sqrt[n]{x}, \section*{title}
    /// Most common command type
    ///
    /// Note: Unknown commands (from non-strict mode parsing) are also represented
    /// as Command nodes with is_unknown=true and empty args.
    Command {
        name: String,
        starred: bool,    // Has * suffix (e.g., \section*)
        is_unknown: bool, // True if this was an unknown command in non-strict mode
        args: Vec<Argument>,
    },

    /// Infix command: left and right operands
    /// Example: a \over b, {n \choose k}
    /// Only ONE infix command allowed per group
    InfixCommand {
        name: String,
        starred: bool,
        args: Vec<Argument>, // Command's own arguments (usually empty)
        left: NodeId,        // Left operand (Char, Command, Scripted, or Group)
        right: NodeId,       // Right operand (Char, Command, Scripted, or Group)
    },

    /// Declarative command affecting scope from command to end of group
    DeclarativeCommand {
        name: String,
        starred: bool,
        args: Vec<Argument>, // Command's own arguments (e.g., {red} in \color{red})
        scope: NodeId,       // Scope content (from command to end of group)
    },

    Group {
        children: Vec<NodeId>,
        kind: GroupKind,
        mode: ContentMode,
    },

    Scripted {
        base: NodeId,
        subscript: Option<NodeId>,
        superscript: Option<NodeId>,
    },

    Environment {
        name: String,
        starred: bool,
        args: Vec<Argument>,
        body: NodeId, // Must be a Group node
    },
}

// ============ Ast Structure ============

/// Complete AST arena - the sole owner of all node data
///
/// All tree operations must go through Ast methods to ensure
/// the `nodes` map and `parent` map stay in sync.
pub struct Ast {
    /// Node data storage (single source of truth for "downward" links)
    nodes: HopSlotMap<NodeId, Node>,
    /// Parent links (derived data for O(1) "upward" traversal)
    parent: SecondaryMap<NodeId, ParentLink>,
    /// Root node (always a Group with mode: Math)
    root: NodeId,
}

impl Ast {
    // ============ Construction ============

    pub fn new() -> Self {
        let mut nodes = HopSlotMap::with_key();
        let parent = SecondaryMap::new();

        let root = nodes.insert(Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });

        Ast {
            nodes,
            parent,
            root,
        }
    }

    // ============ Tree Access ============

    /// Get node data (immutable)
    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(id).expect("Invalid NodeId")
    }

    /// Get node data (mutable)
    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(id).expect("Invalid NodeId")
    }

    /// Check whether the arena contains the provided NodeId
    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Get root node ID
    pub fn root(&self) -> NodeId {
        self.root
    }

    // ============ Upward Traversal (O(1)) ============

    /// Get parent link (returns None only for root)
    pub fn parent(&self, id: NodeId) -> Option<ParentLink> {
        self.parent.get(id).copied()
    }

    /// Get parent node ID
    pub fn parent_id(&self, id: NodeId) -> Option<NodeId> {
        self.parent.get(id).map(|link| link.parent)
    }

    /// Get the slot this node occupies in its parent
    pub fn slot(&self, id: NodeId) -> Option<Slot> {
        self.parent.get(id).map(|link| link.slot)
    }

    // ============ Downward Traversal ============

    /// Get children of a Group node (returns empty slice for non-Group nodes)
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        match self.node(id) {
            Node::Group { children, .. } => children,
            _ => &[],
        }
    }

    /// Get arguments of a Command/InfixCommand/DeclarativeCommand/Environment
    pub fn args(&self, id: NodeId) -> &[Argument] {
        match self.node(id) {
            Node::Command { args, .. }
            | Node::InfixCommand { args, .. }
            | Node::DeclarativeCommand { args, .. }
            | Node::Environment { args, .. } => args,
            _ => &[],
        }
    }

    /// Get Scripted node's base (panics if not Scripted)
    pub fn script_base(&self, id: NodeId) -> NodeId {
        match self.node(id) {
            Node::Scripted { base, .. } => *base,
            _ => panic!("Node is not Scripted"),
        }
    }

    /// Get Scripted node's subscript (panics if not Scripted)
    pub fn script_sub(&self, id: NodeId) -> Option<NodeId> {
        match self.node(id) {
            Node::Scripted { subscript, .. } => *subscript,
            _ => panic!("Node is not Scripted"),
        }
    }

    /// Get Scripted node's superscript (panics if not Scripted)
    pub fn script_sup(&self, id: NodeId) -> Option<NodeId> {
        match self.node(id) {
            Node::Scripted { superscript, .. } => *superscript,
            _ => panic!("Node is not Scripted"),
        }
    }

    // ============ Sibling Navigation (only for GroupChild) ============

    /// Get next sibling (returns None if not a GroupChild or is last child)
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent_link = self.parent(id)?;

        if let Slot::GroupChild(idx) = parent_link.slot {
            let siblings = self.children(parent_link.parent);
            if idx + 1 < siblings.len() {
                return Some(siblings[idx + 1]);
            }
        }

        None
    }

    /// Get previous sibling (returns None if not a GroupChild or is first child)
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent_link = self.parent(id)?;

        if let Slot::GroupChild(idx) = parent_link.slot {
            if idx > 0 {
                let siblings = self.children(parent_link.parent);
                return Some(siblings[idx - 1]);
            }
        }

        None
    }

    // ============ Tree Modification ============

    /// Create a new node and return its ID
    /// Note: The node is not attached to the tree yet
    pub fn new_node(&mut self, node: Node) -> NodeId {
        self.nodes.insert(node)
    }

    /// Replace a node with a new one (updates parent links automatically)
    /// Returns the old node data
    ///
    /// # Panics
    /// - If trying to replace the root node
    /// - If any child NodeId in new_node already has a parent
    pub fn replace_node(&mut self, id: NodeId, new_node: Node) -> Node {
        if id == self.root {
            panic!("Cannot replace root node");
        }

        let child_ids = Self::extract_child_ids_from_node(&new_node);
        for child_id in child_ids {
            assert!(
                self.parent.get(child_id).is_none(),
                "Cannot replace with node containing child {:?} that already has a parent",
                child_id
            );
        }

        self.remove_parent_links(id);
        let old_node = std::mem::replace(self.node_mut(id), new_node);
        self.update_parent_links(id);

        old_node
    }

    /// Remove a node and all its descendants from the tree and arena
    /// Returns the removed node data
    /// Panics if trying to remove the root node
    pub fn remove_node(&mut self, id: NodeId) -> Node {
        if id == self.root {
            panic!("Cannot remove root node");
        }

        if let Some(parent_link) = self.parent(id) {
            match parent_link.slot {
                Slot::GroupChild(idx) => {
                    // Update children vector and get the updated list
                    let children =
                        if let Node::Group { children, .. } = self.node_mut(parent_link.parent) {
                            children.remove(idx);
                            children.clone()
                        } else {
                            Vec::new()
                        };

                    self.rebuild_group_child_links(parent_link.parent, &children, idx);
                }
                _ => {
                    // For non-GroupChild slots, we cannot remove the node
                    // as it would leave the parent in an invalid state
                    panic!("Cannot remove node with non-GroupChild slot");
                }
            }
        }

        // Delete all descendants first, then the node itself
        self.delete_subtree(id)
    }

    /// Append a child to a Group node (panics if not a Group)
    ///
    /// # Panics
    /// - If parent is not a Group node
    /// - If child already has a parent (use detach_child first)
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        assert!(
            self.parent.get(child).is_none(),
            "Cannot append child that already has a parent. Use detach_child first."
        );

        let idx = match self.node_mut(parent) {
            Node::Group { children, .. } => {
                let idx = children.len();
                children.push(child);
                idx
            }
            _ => panic!("Parent is not a Group node"),
        };

        self.parent.insert(
            child,
            ParentLink {
                parent,
                slot: Slot::GroupChild(idx),
            },
        );

        self.update_parent_links(child);
    }

    /// Insert a child at position i in a Group node (panics if not a Group)
    ///
    /// # Panics
    /// - If parent is not a Group node
    /// - If child already has a parent (use detach_child first)
    pub fn insert_child(&mut self, parent: NodeId, index: usize, child: NodeId) {
        assert!(
            self.parent.get(child).is_none(),
            "Cannot insert child that already has a parent. Use detach_child first."
        );

        let children = match self.node_mut(parent) {
            Node::Group { children, .. } => {
                children.insert(index, child);
                children.clone()
            }
            _ => panic!("Parent is not a Group node"),
        };

        self.rebuild_group_child_links(parent, &children, index);

        self.update_parent_links(child);
    }

    /// Detach a child from a Group node at position i (does NOT delete the node)
    /// Returns the detached child's NodeId
    ///
    /// Note: The detached node and its subtree remain in the arena.
    /// Use `delete_subtree(id)` to reclaim memory if no longer needed.
    pub fn detach_child(&mut self, parent: NodeId, index: usize) -> NodeId {
        let (child, children) = match self.node_mut(parent) {
            Node::Group { children, .. } => {
                let child = children.remove(index);
                (child, children.clone())
            }
            _ => panic!("Parent is not a Group node"),
        };

        self.rebuild_group_child_links(parent, &children, index);

        self.parent.remove(child);
        child
    }

    /// Delete a node and all its descendants from the arena
    /// Returns the deleted node data
    ///
    /// Uses post-order traversal: first collects all nodes in the subtree,
    /// then deletes them in reverse order (children before parents).
    pub fn delete_subtree(&mut self, id: NodeId) -> Node {
        // Collect all nodes in post-order (children before parents)
        let mut to_delete = Vec::new();
        self.collect_subtree_postorder(id, &mut to_delete);

        // Delete all descendants (but not the root yet)
        for &node_id in &to_delete[..to_delete.len() - 1] {
            self.parent.remove(node_id);
            self.nodes.remove(node_id);
        }

        // Delete and return the root node
        self.parent.remove(id);
        self.nodes.remove(id).expect("Invalid NodeId")
    }

    /// Collect all nodes in a subtree using post-order traversal
    fn collect_subtree_postorder(&self, id: NodeId, result: &mut Vec<NodeId>) {
        for child in self.iter_children(id).collect::<Vec<_>>() {
            self.collect_subtree_postorder(child, result);
        }
        result.push(id);
    }

    // ============ Query ============

    pub fn find<F>(&self, start: NodeId, predicate: F) -> Option<NodeId>
    where
        F: Fn(&Node) -> bool,
    {
        self.find_internal(start, &predicate)
    }

    /// Find all nodes matching predicate (depth-first search from given node)
    pub fn find_all<F>(&self, start: NodeId, predicate: F) -> Vec<NodeId>
    where
        F: Fn(&Node) -> bool,
    {
        let mut result = Vec::new();
        self.find_all_recursive(start, &predicate, &mut result);
        result
    }

    // ============ Internal Helper Methods ============

    /// Internal helper for find to avoid type recursion
    fn find_internal(&self, start: NodeId, predicate: &dyn Fn(&Node) -> bool) -> Option<NodeId> {
        let node = self.node(start);

        if predicate(node) {
            return Some(start);
        }

        self.iter_children(start)
            .find_map(|child| self.find_internal(child, predicate))
    }

    /// Update parent links for all children of a node
    fn update_parent_links(&mut self, id: NodeId) {
        // Extract all child IDs first to avoid borrow conflicts
        let child_data = self.edges(id);

        for (child, slot) in child_data {
            if matches!(slot, Slot::EnvBody) {
                assert!(
                    matches!(self.node(child), Node::Group { .. }),
                    "Environment body must be a Group node"
                );
            }
            self.parent.insert(child, ParentLink { parent: id, slot });
            self.update_parent_links(child);
        }
    }

    /// Extract all child NodeIds from a node (without borrowing self)
    fn extract_child_ids_from_node(node: &Node) -> Vec<NodeId> {
        let mut result = Vec::new();

        match node {
            Node::Group { children, .. } => {
                result.extend(children.iter().copied());
            }
            Node::Command { args, .. } => {
                result.extend(args.iter().map(|arg| arg.content));
            }
            Node::Environment { args, body, .. } => {
                result.extend(args.iter().map(|arg| arg.content));
                result.push(*body);
            }
            Node::InfixCommand {
                left, right, args, ..
            } => {
                result.push(*left);
                result.push(*right);
                result.extend(args.iter().map(|arg| arg.content));
            }
            Node::DeclarativeCommand { scope, args, .. } => {
                result.push(*scope);
                result.extend(args.iter().map(|arg| arg.content));
            }
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => {
                result.push(*base);
                if let Some(sub) = subscript {
                    result.push(*sub);
                }
                if let Some(sup) = superscript {
                    result.push(*sup);
                }
            }
            Node::Char(_) | Node::Text(_) => {}
        }

        result
    }

    /// Get all edges (parent-child relationships) for a node.
    /// Returns a vector of (child_id, slot) pairs in left-to-right traversal order.
    /// This is the single source of truth for child traversal order.
    pub fn edges(&self, id: NodeId) -> Vec<(NodeId, Slot)> {
        let mut result = Vec::new();
        let node = self.node(id);

        match node {
            Node::Group { children, .. } => {
                for (idx, &child) in children.iter().enumerate() {
                    result.push((child, Slot::GroupChild(idx)));
                }
            }
            Node::Command { args, .. } => {
                for (idx, arg) in args.iter().enumerate() {
                    result.push((arg.content, Slot::Argument(idx)));
                }
            }
            Node::Environment { args, body, .. } => {
                for (idx, arg) in args.iter().enumerate() {
                    result.push((arg.content, Slot::Argument(idx)));
                }
                result.push((*body, Slot::EnvBody));
            }
            Node::InfixCommand {
                left, right, args, ..
            } => {
                result.push((*left, Slot::InfixLeft));
                result.push((*right, Slot::InfixRight));
                for (idx, arg) in args.iter().enumerate() {
                    result.push((arg.content, Slot::Argument(idx)));
                }
            }
            Node::DeclarativeCommand { scope, args, .. } => {
                result.push((*scope, Slot::DeclarativeScope));
                for (idx, arg) in args.iter().enumerate() {
                    result.push((arg.content, Slot::Argument(idx)));
                }
            }
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => {
                result.push((*base, Slot::ScriptBase));
                if let Some(sub) = subscript {
                    result.push((*sub, Slot::ScriptSub));
                }
                if let Some(sup) = superscript {
                    result.push((*sup, Slot::ScriptSup));
                }
            }
            Node::Char(_) | Node::Text(_) => {}
        }

        result
    }

    /// Remove parent links for all direct children of a node (non-recursive)
    fn remove_parent_links(&mut self, id: NodeId) {
        let children = self.iter_children(id).collect::<Vec<_>>();
        for child in children {
            self.parent.remove(child);
        }
    }

    /// Rebuild parent links for Group children starting at a specific index
    fn rebuild_group_child_links(
        &mut self,
        parent: NodeId,
        children: &[NodeId],
        start_index: usize,
    ) {
        for (idx, &child_id) in children.iter().enumerate().skip(start_index) {
            self.parent.insert(
                child_id,
                ParentLink {
                    parent,
                    slot: Slot::GroupChild(idx),
                },
            );
        }
    }

    /// Iterate over all direct children of a node.
    /// Based on edges() to ensure consistent traversal order.
    fn iter_children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.edges(id).into_iter().map(|(child, _slot)| child)
    }

    /// Recursive helper for find_all
    fn find_all_recursive(
        &self,
        start: NodeId,
        predicate: &dyn Fn(&Node) -> bool,
        result: &mut Vec<NodeId>,
    ) {
        let node = self.node(start);

        if predicate(node) {
            result.push(start);
        }

        for child in self.iter_children(start).collect::<Vec<_>>() {
            self.find_all_recursive(child, predicate, result);
        }
    }
}

impl Default for Ast {
    fn default() -> Self {
        Self::new()
    }
}

// Tests in tests/ast.rs
