//! Transform-ready mutable AST for tree editing.
//!
//! The parser still exposes [`syntax_node::SyntaxNode`] as the public parse
//! result. This module defines a separate arena-backed IR that keeps the same
//! semantics while supporting:
//!
//! - fast node kind checks
//! - bidirectional navigation through parent links
//! - safe structural edits without exposing raw mutable node access
//!
//! The AST owns all nodes in a [`slotmap::HopSlotMap`]. Tree edits must go
//! through [`Ast`] methods so `nodes`, `parent`, and detached subtree tracking
//! stay consistent.
//!
//! # Detached Roots
//!
//! A node with no parent is valid only when it is either:
//!
//! - the main [`Ast::root`], or
//! - a tracked detached subtree root
//!
//! Detached roots let transforms stage or preserve subtrees without silently
//! creating invisible orphans in the arena.

use std::collections::HashSet;

use slotmap::{HopSlotMap, SecondaryMap, new_key_type};
use texform_interface::syntax_node::{self, SyntaxNode};

/// Re-exported content mode shared with parser syntax nodes.
///
/// Keeping a single definition avoids duplicate `Math` / `Text` enums drifting
/// apart across parsing and transform stages.
pub use texform_interface::syntax_node::ContentMode;

new_key_type! {
    /// Stable arena key for a node owned by [`Ast`].
    pub struct NodeId;
}

/// Upward edge metadata for a node that is currently attached to a parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParentLink {
    /// Parent node that owns the child.
    pub parent: NodeId,
    /// Concrete position occupied by the child within the parent.
    pub slot: Slot,
}

/// The direct attachment site a child occupies in its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Child at an index inside [`Node::Group::children`]
    GroupChild(usize),
    /// Content argument stored in an argument list slot
    Argument(usize),
    /// Base child of [`Node::Scripted`]
    ScriptBase,
    /// Subscript child of [`Node::Scripted`]
    ScriptSub,
    /// Superscript child of [`Node::Scripted`]
    ScriptSup,
    /// Left operand of [`Node::Infix`]
    InfixLeft,
    /// Right operand of [`Node::Infix`]
    InfixRight,
    /// Scope child of [`Node::Declarative`]
    DeclarativeScope,
    /// Body child of [`Node::Environment`]
    EnvBody,
}

/// Cheap node discriminant for queries that do not need full pattern matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Group,
    Command,
    Infix,
    Declarative,
    Environment,
    Scripted,
    UnknownCommand,
    Text,
    Char,
    ActiveSpace,
}

/// Delimiter value used by groups and argument kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delimiter {
    /// No delimiter, corresponding to `.` in LaTeX
    None,
    /// Single-character delimiter such as `(`, `)` or `|`
    Char(char),
    // The AST keeps this owned so future transforms are not restricted to
    // interned or static control names.
    /// Control-sequence delimiter such as `\langle` or `\rbrace`
    ///
    /// This is intentionally owned `String` data even though the parser stores
    /// `&'static str`, because transforms may synthesize non-static names.
    Control(String),
}

/// Concrete grouping form preserved from parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// Explicit brace group: `{...}`
    Explicit,
    /// Implicit synthetic group used by normalization or parser folding
    Implicit,
    /// Delimited group such as `\left( ... \right)`
    Delimited { left: Delimiter, right: Delimiter },
    /// Inline math segment inside text mode: `$...$`
    InlineMath,
}

/// Argument slot kind preserved from xparse-aware parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgumentKind {
    /// Standard mandatory argument
    Mandatory,
    /// Standard optional bracket argument
    Optional,
    /// Boolean star slot
    Star,
    /// Required braced-group form
    Group,
    /// Delimited argument with a matched open / close pair
    Delimited { open: Delimiter, close: Delimiter },
    /// Paired-candidate argument that records the matched delimiters
    Paired { open: Delimiter, close: Delimiter },
}

/// Parsed argument payload.
///
/// Only [`ArgumentValue::Content`] contributes a tree edge. All scalar variants
/// are stored inline on the owning node and are skipped by tree traversal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgumentValue {
    /// Child subtree used as argument content
    Content(NodeId),
    /// Parsed delimiter value
    Delimiter(Delimiter),
    /// Control-sequence name without leading backslash
    CSName(String),
    /// Raw dimension string
    Dimension(String),
    /// Raw integer string
    Integer(String),
    /// Raw key-value string
    KeyVal(String),
    /// Parsed column specification
    Column(String),
    /// Boolean value, primarily used for star slots
    Boolean(bool),
}

/// Parsed command or environment argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    /// Slot kind recorded by the parser
    pub kind: ArgumentKind,
    /// Parsed value stored in the slot
    pub value: ArgumentValue,
}

/// Optional argument slot in a command or environment signature.
///
/// `None` means the slot exists in the spec but was not supplied in source.
pub type ArgumentSlot = Option<Argument>;

/// Mutable AST node stored inside [`Ast`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// Group node containing ordered children and mode metadata.
    Group {
        /// Ordered direct children of the group
        children: Vec<NodeId>,
        /// Source-level grouping form
        kind: GroupKind,
        /// Content mode used to parse the group
        mode: ContentMode,
    },
    /// Prefix command with argument slots.
    Command {
        /// Command name without leading backslash
        name: String,
        /// Slots defined by the matched command spec
        args: Vec<ArgumentSlot>,
    },
    /// Infix command with explicit left and right operands.
    Infix {
        /// Command name without leading backslash
        name: String,
        /// Additional argument slots owned by the infix node
        args: Vec<ArgumentSlot>,
        /// Left operand subtree
        left: NodeId,
        /// Right operand subtree
        right: NodeId,
    },
    /// Declarative command whose scope runs to the end of the current group.
    Declarative {
        /// Command name without leading backslash
        name: String,
        /// Additional argument slots owned by the declarative node
        args: Vec<ArgumentSlot>,
        /// Scope subtree affected by the declaration
        scope: NodeId,
    },
    /// Environment node whose body must always be a group.
    Environment {
        /// Environment name without `begin` / `end`
        name: String,
        /// Parsed argument slots attached to the environment
        args: Vec<ArgumentSlot>,
        /// Environment body subtree. Must be a [`Node::Group`]
        body: NodeId,
    },
    /// Scripted expression such as `x_i^2`.
    Scripted {
        /// Base expression
        base: NodeId,
        /// Optional subscript subtree
        subscript: Option<NodeId>,
        /// Optional superscript subtree
        superscript: Option<NodeId>,
    },
    /// Unknown command preserved from non-strict parsing.
    UnknownCommand {
        /// Unknown command name without leading backslash
        name: String,
    },
    /// Text-mode text chunk
    Text(String),
    /// Single character node
    Char(char),
    /// Active `~` space node
    ActiveSpace,
}

impl Node {
    /// Return the lightweight discriminant for this node.
    pub const fn kind(&self) -> NodeKind {
        match self {
            Node::Group { .. } => NodeKind::Group,
            Node::Command { .. } => NodeKind::Command,
            Node::Infix { .. } => NodeKind::Infix,
            Node::Declarative { .. } => NodeKind::Declarative,
            Node::Environment { .. } => NodeKind::Environment,
            Node::Scripted { .. } => NodeKind::Scripted,
            Node::UnknownCommand { .. } => NodeKind::UnknownCommand,
            Node::Text(_) => NodeKind::Text,
            Node::Char(_) => NodeKind::Char,
            Node::ActiveSpace => NodeKind::ActiveSpace,
        }
    }
}

/// Arena-backed mutable AST used by transform-oriented code.
///
/// Public mutation APIs intentionally avoid exposing `&mut Node` so parent links
/// and detached subtree tracking cannot be bypassed accidentally.
#[derive(Debug, Clone)]
pub struct Ast {
    nodes: HopSlotMap<NodeId, Node>,
    parent: SecondaryMap<NodeId, ParentLink>,
    // Detached roots are valid subtrees that currently live in the arena but
    // are not attached to the main root. This lets transforms stage nodes
    // before insertion without turning them into invisible orphans.
    detached_roots: HashSet<NodeId>,
    root: NodeId,
}

impl Ast {
    /// Create an empty AST with an implicit math-mode root group.
    pub fn new() -> Self {
        let mut nodes = HopSlotMap::with_key();
        let root = nodes.insert(Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });

        Ast {
            nodes,
            parent: SecondaryMap::new(),
            detached_roots: HashSet::new(),
            root,
        }
    }

    /// Convert a parsed [`SyntaxNode`] tree into a mutable [`Ast`].
    ///
    /// Conversion preserves shape:
    ///
    /// - `ArgumentValue::Content` is converted directly to the referenced node
    /// - single-node content is not wrapped in an implicit group
    /// - `Delimiter::Control(&'static str)` becomes [`Delimiter::Control`] with
    ///   owned `String` data
    ///
    /// # Panics
    ///
    /// Panics if the converted tree violates AST invariants, such as an
    /// environment body that is not a group.
    pub fn from_syntax_node(node: &SyntaxNode) -> Self {
        let mut nodes = HopSlotMap::with_key();
        let mut parent = SecondaryMap::new();
        let root = Self::convert_syntax_node(node, &mut nodes, &mut parent);

        let ast = Ast {
            nodes,
            parent,
            detached_roots: HashSet::new(),
            root,
        };
        ast.assert_invariants();
        ast
    }

    /// Return the main root node of the AST.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Check whether `id` still exists in the arena.
    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Return the lightweight node kind for `id`.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn kind(&self, id: NodeId) -> NodeKind {
        self.node(id).kind()
    }

    /// Borrow the full node data for `id`.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(id).expect("Invalid NodeId")
    }

    /// Return the parent link for `id`, if the node is attached.
    ///
    /// Root and detached roots return `None`.
    pub fn parent(&self, id: NodeId) -> Option<ParentLink> {
        self.parent.get(id).copied()
    }

    /// Return the parent node ID for `id`, if attached.
    pub fn parent_id(&self, id: NodeId) -> Option<NodeId> {
        self.parent(id).map(|link| link.parent)
    }

    /// Return the slot occupied by `id` in its parent, if attached.
    pub fn slot(&self, id: NodeId) -> Option<Slot> {
        self.parent(id).map(|link| link.slot)
    }

    /// Return the direct children of a group node.
    ///
    /// Non-group nodes return an empty slice.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        match self.node(id) {
            Node::Group { children, .. } => children,
            _ => &[],
        }
    }

    /// Return the argument slots owned by a command-like node.
    ///
    /// Non-command-like nodes return an empty slice.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn arg_slots(&self, id: NodeId) -> &[ArgumentSlot] {
        match self.node(id) {
            Node::Command { args, .. }
            | Node::Infix { args, .. }
            | Node::Declarative { args, .. }
            | Node::Environment { args, .. } => args,
            _ => &[],
        }
    }

    /// Return every direct tree edge of `id` as `(child, slot)` pairs.
    ///
    /// Returned order matches the AST's direct traversal order. Only
    /// [`ArgumentValue::Content`] entries are exposed as argument edges.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn edges(&self, id: NodeId) -> Vec<(NodeId, Slot)> {
        Self::node_edges(self.node(id))
    }

    /// Return the next sibling of `id` when it is attached as a group child.
    ///
    /// Nodes in non-`GroupChild` slots return `None`.
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent_link = self.parent(id)?;
        let Slot::GroupChild(index) = parent_link.slot else {
            return None;
        };

        self.children(parent_link.parent).get(index + 1).copied()
    }

    /// Return the previous sibling of `id` when it is attached as a group child.
    ///
    /// Nodes in non-`GroupChild` slots return `None`.
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent_link = self.parent(id)?;
        let Slot::GroupChild(index) = parent_link.slot else {
            return None;
        };

        index
            .checked_sub(1)
            .and_then(|prev| self.children(parent_link.parent).get(prev).copied())
    }

    /// Depth-first search starting at `start`, returning the first matching node.
    ///
    /// # Panics
    ///
    /// Panics if `start` is invalid.
    pub fn find<F>(&self, start: NodeId, predicate: F) -> Option<NodeId>
    where
        F: Fn(&Node) -> bool,
    {
        self.find_impl(start, &predicate)
    }

    /// Collect all matching nodes reachable from `start` in depth-first order.
    ///
    /// The returned vector is a snapshot of `NodeId`s at collection time.
    /// Later mutations may delete or move those nodes, so callers should use
    /// [`Ast::contains`] and, when necessary, re-check parent / slot state
    /// before mutating.
    ///
    /// # Panics
    ///
    /// Panics if `start` is invalid.
    pub fn find_all<F>(&self, start: NodeId, predicate: F) -> Vec<NodeId>
    where
        F: Fn(&Node) -> bool,
    {
        let mut result = Vec::new();
        self.find_all_impl(start, &predicate, &mut result);
        result
    }

    /// Insert a new detached node into the arena and return its [`NodeId`].
    ///
    /// If `node` references child IDs, those children must already exist as
    /// detached roots. They are adopted by the new node immediately.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - the same child is referenced more than once
    /// - a referenced child does not exist
    /// - a referenced child is not a detached root
    /// - adopting a child would introduce a cycle
    /// - an environment body is not a group
    pub fn new_node(&mut self, node: Node) -> NodeId {
        Self::assert_unique_direct_children(&node);
        let direct_children = Self::node_edges(&node);
        let id = self.nodes.insert(node);
        self.detached_roots.insert(id);

        for &(child, slot) in &direct_children {
            self.assert_child_is_detached_root(child);
            self.assert_no_cycle(child, id);
            self.assert_slot_shape(slot, child);
            self.detached_roots.remove(&child);
            self.parent.insert(child, ParentLink { parent: id, slot });
        }

        id
    }

    /// Append `child` to the end of a group's child list.
    ///
    /// `child` must currently be a detached root.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - `parent` is not a group
    /// - `child` is invalid
    /// - `child` is already attached
    /// - `child` is not tracked as a detached root
    /// - attaching `child` would introduce a cycle
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.assert_child_is_detached_root(child);
        self.assert_no_cycle(child, parent);

        let index = match self.node_mut(parent) {
            Node::Group { children, .. } => {
                let index = children.len();
                children.push(child);
                index
            }
            _ => panic!("Parent is not a Group node"),
        };

        self.detached_roots.remove(&child);
        self.parent.insert(
            child,
            ParentLink {
                parent,
                slot: Slot::GroupChild(index),
            },
        );
    }

    /// Insert `child` at `index` within a group's child list.
    ///
    /// `child` must currently be a detached root.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - `parent` is not a group
    /// - `index` is out of bounds for `Vec::insert`
    /// - `child` is invalid
    /// - `child` is already attached
    /// - `child` is not tracked as a detached root
    /// - attaching `child` would introduce a cycle
    pub fn insert_child(&mut self, parent: NodeId, index: usize, child: NodeId) {
        self.assert_child_is_detached_root(child);
        self.assert_no_cycle(child, parent);

        match self.node_mut(parent) {
            Node::Group { children, .. } => {
                children.insert(index, child);
            }
            _ => panic!("Parent is not a Group node"),
        }

        self.detached_roots.remove(&child);
        self.reindex_group_children(parent, index);
    }

    /// Detach `id` from its parent and return the same node ID.
    ///
    /// This version only supports detaching nodes that occupy [`Slot::GroupChild`].
    /// Detached nodes remain in the arena and become detached roots until they
    /// are reattached or explicitly removed with [`Ast::remove_detached`].
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - `id` is the main root
    /// - `id` is already detached
    /// - `id` is attached through a non-`GroupChild` slot
    pub fn detach(&mut self, id: NodeId) -> NodeId {
        if id == self.root {
            panic!("Cannot detach root node");
        }

        let parent_link = self
            .parent(id)
            .unwrap_or_else(|| panic!("Cannot detach node without a parent"));
        let Slot::GroupChild(index) = parent_link.slot else {
            panic!("Can only detach GroupChild nodes");
        };

        match self.node_mut(parent_link.parent) {
            Node::Group { children, .. } => {
                let removed = children.remove(index);
                assert_eq!(removed, id, "Group child index must match detached node");
            }
            _ => panic!("Parent is not a Group node"),
        }

        self.parent.remove(id);
        self.detached_roots.insert(id);
        self.reindex_group_children(parent_link.parent, index);
        id
    }

    /// Remove an attached node and its entire subtree from the arena.
    ///
    /// This is implemented as [`Ast::detach`] followed by
    /// [`Ast::remove_detached`].
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Ast::detach`] or
    /// [`Ast::remove_detached`].
    pub fn remove_node(&mut self, id: NodeId) {
        let detached = self.detach(id);
        self.remove_detached(detached);
    }

    /// Replace the node data stored at `id` while preserving the same [`NodeId`].
    ///
    /// The new node may:
    ///
    /// - reuse direct children already owned by `id`
    /// - adopt detached roots
    ///
    /// Direct children removed by the replacement are not deleted. They become
    /// detached roots so transforms can keep or inspect them explicitly.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - `id` is the main root or invalid
    /// - the new node references the same child more than once
    /// - a reused child is not a direct child of `id`
    /// - a newly introduced child is not a detached root
    /// - adopting a child would introduce a cycle
    /// - an environment body is not a group
    pub fn replace_node(&mut self, id: NodeId, new_node: Node) -> Node {
        if id == self.root {
            panic!("Cannot replace root node");
        }
        if !self.contains(id) {
            panic!("Invalid NodeId");
        }

        Self::assert_unique_direct_children(&new_node);

        let old_edges = self.edges(id);
        let old_children: HashSet<NodeId> = old_edges.iter().map(|(child, _)| *child).collect();
        let new_edges = Self::node_edges(&new_node);
        let was_detached = self.detached_roots.contains(&id);

        for &(child, slot) in &new_edges {
            self.assert_slot_shape(slot, child);
            if old_children.contains(&child) {
                let link = self
                    .parent(child)
                    .unwrap_or_else(|| panic!("Existing child is missing parent link"));
                assert_eq!(
                    link.parent, id,
                    "Can only reuse direct children of the replaced node"
                );
            } else {
                self.assert_child_is_detached_root(child);
                self.assert_no_cycle(child, id);
            }
        }

        // Children that disappear from the new node are preserved as detached
        // roots instead of being silently orphaned or recursively deleted.
        for &(child, _) in &old_edges {
            if !new_edges.iter().any(|(new_child, _)| *new_child == child) {
                self.parent.remove(child);
                self.detached_roots.insert(child);
            }
        }

        for &(child, _) in &new_edges {
            if !old_children.contains(&child) {
                self.detached_roots.remove(&child);
            }
        }

        let old_node = std::mem::replace(self.node_mut(id), new_node);
        for (child, slot) in new_edges {
            self.parent.insert(child, ParentLink { parent: id, slot });
        }

        if was_detached {
            self.detached_roots.insert(id);
        } else {
            self.detached_roots.remove(&id);
        }

        old_node
    }

    /// Destroy a detached subtree and return the removed root node value.
    ///
    /// The subtree must already be detached from the main tree. All descendants
    /// are removed from the arena as well.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - `id` is the main root or invalid
    /// - `id` is still attached
    /// - `id` is not tracked as a detached root
    pub fn remove_detached(&mut self, id: NodeId) -> Node {
        if id == self.root {
            panic!("Cannot remove root node");
        }
        if !self.contains(id) {
            panic!("Invalid NodeId");
        }
        if self.parent(id).is_some() {
            panic!("Can only remove detached roots");
        }
        if !self.detached_roots.remove(&id) {
            panic!("Node is not a detached root");
        }

        let mut postorder = Vec::new();
        self.collect_postorder(id, &mut postorder);

        for node_id in postorder
            .iter()
            .copied()
            .take(postorder.len().saturating_sub(1))
        {
            self.parent.remove(node_id);
            self.detached_roots.remove(&node_id);
            self.nodes.remove(node_id);
        }

        self.parent.remove(id);
        self.nodes.remove(id).expect("Detached root must exist")
    }

    /// Assert all structural invariants of the AST.
    ///
    /// Checked conditions include:
    ///
    /// - the main root exists and has no parent
    /// - every parent link corresponds to a real direct edge
    /// - every node is reachable from either the root or a detached root
    /// - every parentless non-root node is tracked in detached roots
    /// - environment bodies are always groups
    ///
    /// This method is intended for tests and debug-time validation.
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message when any invariant is violated.
    pub fn assert_invariants(&self) {
        assert!(self.contains(self.root), "Root node must exist");
        assert!(
            self.parent(self.root).is_none(),
            "Root node must not have a parent"
        );
        assert!(
            !self.detached_roots.contains(&self.root),
            "Root node cannot be a detached root"
        );

        for (id, link) in self.parent.iter() {
            assert!(self.contains(id), "Parent map contains non-existent child");
            assert!(
                self.contains(link.parent),
                "Parent map points to non-existent parent"
            );
            let has_edge = self
                .edges(link.parent)
                .into_iter()
                .any(|(child, slot)| child == id && slot == link.slot);
            assert!(has_edge, "Parent link must match a direct edge");
        }

        let mut visited = HashSet::new();
        self.assert_subtree(self.root, None, &mut visited);

        for detached_root in &self.detached_roots {
            assert!(*detached_root != self.root, "Root cannot be detached");
            assert!(self.contains(*detached_root), "Detached root must exist");
            assert!(
                self.parent(*detached_root).is_none(),
                "Detached root must not have a parent"
            );
            self.assert_subtree(*detached_root, None, &mut visited);
        }

        for (id, _) in self.nodes.iter() {
            assert!(visited.contains(&id), "Node is orphaned or unreachable");
            if id != self.root && self.parent(id).is_none() {
                assert!(
                    self.detached_roots.contains(&id),
                    "Rootless nodes must be tracked as detached roots"
                );
            }
        }
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(id).expect("Invalid NodeId")
    }

    fn assert_child_is_detached_root(&self, child: NodeId) {
        assert!(self.contains(child), "Invalid child NodeId");
        assert!(child != self.root, "Cannot attach the AST root as a child");
        assert!(
            self.parent(child).is_none(),
            "Cannot attach child that already has a parent"
        );
        assert!(
            self.detached_roots.contains(&child),
            "Can only attach detached roots"
        );
    }

    fn assert_no_cycle(&self, child: NodeId, new_parent: NodeId) {
        assert!(
            !self.subtree_contains(child, new_parent),
            "Cannot create an ancestor cycle"
        );
    }

    fn assert_slot_shape(&self, slot: Slot, child: NodeId) {
        if matches!(slot, Slot::EnvBody) {
            assert!(
                matches!(self.node(child), Node::Group { .. }),
                "Environment body must be a Group node"
            );
        }
    }

    fn reindex_group_children(&mut self, parent: NodeId, start: usize) {
        let children = match self.node(parent) {
            Node::Group { children, .. } => children.clone(),
            _ => panic!("Parent is not a Group node"),
        };

        // Group child slots store indices, so every insertion / removal requires
        // rewriting the suffix of the parent link table.
        for (index, child) in children.into_iter().enumerate().skip(start) {
            self.parent.insert(
                child,
                ParentLink {
                    parent,
                    slot: Slot::GroupChild(index),
                },
            );
        }
    }

    fn collect_postorder(&self, id: NodeId, out: &mut Vec<NodeId>) {
        for (child, _) in self.edges(id) {
            self.collect_postorder(child, out);
        }
        out.push(id);
    }

    fn assert_subtree(
        &self,
        id: NodeId,
        expected_parent: Option<ParentLink>,
        visited: &mut HashSet<NodeId>,
    ) {
        assert!(visited.insert(id), "Node is reachable from multiple roots");
        assert_eq!(
            self.parent(id),
            expected_parent,
            "Parent link must match traversal path"
        );

        for (child, slot) in self.edges(id) {
            assert!(self.contains(child), "Direct edge points to invalid child");
            self.assert_slot_shape(slot, child);
            self.assert_subtree(child, Some(ParentLink { parent: id, slot }), visited);
        }
    }

    fn subtree_contains(&self, root: NodeId, target: NodeId) -> bool {
        if root == target {
            return true;
        }

        self.edges(root)
            .into_iter()
            .any(|(child, _)| self.subtree_contains(child, target))
    }

    fn find_impl(&self, start: NodeId, predicate: &dyn Fn(&Node) -> bool) -> Option<NodeId> {
        if predicate(self.node(start)) {
            return Some(start);
        }

        for (child, _) in self.edges(start) {
            if let Some(found) = self.find_impl(child, predicate) {
                return Some(found);
            }
        }

        None
    }

    fn find_all_impl(
        &self,
        start: NodeId,
        predicate: &dyn Fn(&Node) -> bool,
        out: &mut Vec<NodeId>,
    ) {
        if predicate(self.node(start)) {
            out.push(start);
        }

        for (child, _) in self.edges(start) {
            self.find_all_impl(child, predicate, out);
        }
    }

    fn assert_unique_direct_children(node: &Node) {
        let mut seen = HashSet::new();
        for (child, _) in Self::node_edges(node) {
            assert!(
                seen.insert(child),
                "Node cannot reference the same child twice"
            );
        }
    }

    fn node_edges(node: &Node) -> Vec<(NodeId, Slot)> {
        let mut edges = Vec::new();

        // This function is the single source of truth for direct traversal
        // order. Public `edges()` and internal mutation helpers both rely on it
        // so read and write paths stay aligned.
        match node {
            Node::Group { children, .. } => {
                for (index, child) in children.iter().copied().enumerate() {
                    edges.push((child, Slot::GroupChild(index)));
                }
            }
            Node::Command { args, .. } => {
                Self::push_argument_edges(args, &mut edges);
            }
            Node::Infix {
                args, left, right, ..
            } => {
                edges.push((*left, Slot::InfixLeft));
                Self::push_argument_edges(args, &mut edges);
                edges.push((*right, Slot::InfixRight));
            }
            Node::Declarative { args, scope, .. } => {
                Self::push_argument_edges(args, &mut edges);
                edges.push((*scope, Slot::DeclarativeScope));
            }
            Node::Environment { args, body, .. } => {
                Self::push_argument_edges(args, &mut edges);
                edges.push((*body, Slot::EnvBody));
            }
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => {
                edges.push((*base, Slot::ScriptBase));
                if let Some(subscript) = subscript {
                    edges.push((*subscript, Slot::ScriptSub));
                }
                if let Some(superscript) = superscript {
                    edges.push((*superscript, Slot::ScriptSup));
                }
            }
            Node::UnknownCommand { .. } | Node::Text(_) | Node::Char(_) | Node::ActiveSpace => {}
        }

        edges
    }

    fn push_argument_edges(args: &[ArgumentSlot], edges: &mut Vec<(NodeId, Slot)>) {
        for (index, slot) in args.iter().enumerate() {
            let Some(argument) = slot else {
                continue;
            };
            // Only content arguments participate in tree traversal. Scalar
            // values still live on the node, but they do not become AST edges.
            let ArgumentValue::Content(child) = &argument.value else {
                continue;
            };
            edges.push((*child, Slot::Argument(index)));
        }
    }

    fn convert_syntax_node(
        node: &SyntaxNode,
        nodes: &mut HopSlotMap<NodeId, Node>,
        parent: &mut SecondaryMap<NodeId, ParentLink>,
    ) -> NodeId {
        // Conversion constructs child nodes first, then inserts the current
        // node, then wires direct parent links immediately. That keeps the
        // transformation local and avoids a second global rebuild pass over the
        // finished tree.
        let converted_node = match node {
            SyntaxNode::Group {
                mode,
                kind,
                children,
            } => {
                let converted_children: Vec<NodeId> = children
                    .iter()
                    .map(|child| Self::convert_syntax_node(child, nodes, parent))
                    .collect();
                Node::Group {
                    children: converted_children,
                    kind: Self::convert_group_kind(kind),
                    mode: *mode,
                }
            }
            SyntaxNode::Command { name, args } => Node::Command {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| Self::convert_argument_slot(slot, nodes, parent))
                    .collect(),
            },
            SyntaxNode::Infix {
                name,
                args,
                left,
                right,
            } => Node::Infix {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| Self::convert_argument_slot(slot, nodes, parent))
                    .collect(),
                left: Self::convert_syntax_node(left, nodes, parent),
                right: Self::convert_syntax_node(right, nodes, parent),
            },
            SyntaxNode::Declarative { name, args, scope } => Node::Declarative {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| Self::convert_argument_slot(slot, nodes, parent))
                    .collect(),
                scope: Self::convert_syntax_node(scope, nodes, parent),
            },
            SyntaxNode::Environment { name, args, body } => Node::Environment {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| Self::convert_argument_slot(slot, nodes, parent))
                    .collect(),
                body: Self::convert_syntax_node(body, nodes, parent),
            },
            SyntaxNode::Scripted {
                base,
                subscript,
                superscript,
            } => Node::Scripted {
                base: Self::convert_syntax_node(base, nodes, parent),
                subscript: subscript
                    .as_ref()
                    .map(|node| Self::convert_syntax_node(node, nodes, parent)),
                superscript: superscript
                    .as_ref()
                    .map(|node| Self::convert_syntax_node(node, nodes, parent)),
            },
            SyntaxNode::UnknownCommand { name } => Node::UnknownCommand { name: name.clone() },
            SyntaxNode::Text(text) => Node::Text(text.clone()),
            SyntaxNode::Char(ch) => Node::Char(*ch),
            SyntaxNode::ActiveSpace => Node::ActiveSpace,
        };

        let id = nodes.insert(converted_node);
        let edges = Self::node_edges(nodes.get(id).expect("Converted node must exist"));
        for (child, slot) in edges {
            if matches!(slot, Slot::EnvBody) {
                assert!(
                    matches!(nodes.get(child), Some(Node::Group { .. })),
                    "Environment body must convert to a Group node"
                );
            }
            parent.insert(child, ParentLink { parent: id, slot });
        }

        id
    }

    fn convert_argument_slot(
        slot: &syntax_node::ArgumentSlot,
        nodes: &mut HopSlotMap<NodeId, Node>,
        parent: &mut SecondaryMap<NodeId, ParentLink>,
    ) -> ArgumentSlot {
        slot.as_ref().map(|arg| Argument {
            kind: Self::convert_argument_kind(arg.kind),
            value: Self::convert_argument_value(&arg.value, nodes, parent),
        })
    }

    fn convert_argument_kind(kind: syntax_node::ArgumentKind) -> ArgumentKind {
        match kind {
            syntax_node::ArgumentKind::Mandatory => ArgumentKind::Mandatory,
            syntax_node::ArgumentKind::Optional => ArgumentKind::Optional,
            syntax_node::ArgumentKind::Star => ArgumentKind::Star,
            syntax_node::ArgumentKind::Group => ArgumentKind::Group,
            syntax_node::ArgumentKind::Delimited { open, close } => ArgumentKind::Delimited {
                open: Self::convert_delimiter(open),
                close: Self::convert_delimiter(close),
            },
            syntax_node::ArgumentKind::Paired { open, close } => ArgumentKind::Paired {
                open: Self::convert_delimiter(open),
                close: Self::convert_delimiter(close),
            },
        }
    }

    fn convert_argument_value(
        value: &syntax_node::ArgumentValue,
        nodes: &mut HopSlotMap<NodeId, Node>,
        parent: &mut SecondaryMap<NodeId, ParentLink>,
    ) -> ArgumentValue {
        match value {
            // Conversion keeps the original shape instead of wrapping single
            // content nodes in an implicit group.
            syntax_node::ArgumentValue::Content(node) => {
                ArgumentValue::Content(Self::convert_syntax_node(node, nodes, parent))
            }
            syntax_node::ArgumentValue::Delimiter(delimiter) => {
                ArgumentValue::Delimiter(Self::convert_delimiter(*delimiter))
            }
            syntax_node::ArgumentValue::CSName(name) => ArgumentValue::CSName(name.clone()),
            syntax_node::ArgumentValue::Dimension(value) => ArgumentValue::Dimension(value.clone()),
            syntax_node::ArgumentValue::Integer(value) => ArgumentValue::Integer(value.clone()),
            syntax_node::ArgumentValue::KeyVal(value) => ArgumentValue::KeyVal(value.clone()),
            syntax_node::ArgumentValue::Column(value) => ArgumentValue::Column(value.clone()),
            syntax_node::ArgumentValue::Boolean(value) => ArgumentValue::Boolean(*value),
        }
    }

    fn convert_group_kind(kind: &syntax_node::GroupKind) -> GroupKind {
        match kind {
            syntax_node::GroupKind::Explicit => GroupKind::Explicit,
            syntax_node::GroupKind::Implicit => GroupKind::Implicit,
            syntax_node::GroupKind::Delimited { left, right } => GroupKind::Delimited {
                left: Self::convert_delimiter(*left),
                right: Self::convert_delimiter(*right),
            },
            syntax_node::GroupKind::InlineMath => GroupKind::InlineMath,
        }
    }

    fn convert_delimiter(delimiter: syntax_node::Delimiter) -> Delimiter {
        match delimiter {
            syntax_node::Delimiter::None => Delimiter::None,
            syntax_node::Delimiter::Char(ch) => Delimiter::Char(ch),
            syntax_node::Delimiter::Control(name) => Delimiter::Control(name.to_string()),
        }
    }
}

impl Default for Ast {
    fn default() -> Self {
        Self::new()
    }
}
