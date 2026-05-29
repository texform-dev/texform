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
//!
//! # Structural Invariants Only
//!
//! [`Ast`] validates tree ownership, parent links, detached roots, and slot
//! shape. It does not validate command signatures or argument semantics such as
//! whether an [`ArgumentKind`] matches a particular [`ArgumentValue`].

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
    /// Child at an index inside [`Node::Root::children`] / [`Node::Group::children`]
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
    /// Body child of [`Node::Environment`]
    EnvBody,
}

/// Cheap node discriminant for queries that do not need full pattern matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Root,
    Group,
    Command,
    Infix,
    Declarative,
    Environment,
    Scripted,
    Text,
    Char,
    ActiveSpace,
    /// Parser-produced error placeholder; see [`Node::Error`].
    Error,
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
/// Only content-carrying argument variants contribute a tree edge. All scalar variants
/// are stored inline on the owning node and are skipped by tree traversal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgumentValue {
    /// Child subtree used as math-mode argument content
    MathContent(NodeId),
    /// Child subtree used as text-mode argument content
    TextContent(NodeId),
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
    /// Main document root containing top-level children and parse mode.
    Root {
        /// Ordered direct children of the root
        children: Vec<NodeId>,
        /// Content mode used to parse the formula
        mode: ContentMode,
    },
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
        /// Whether this name is present in the knowledge base.
        known: bool,
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
    /// Declarative command with explicit argument slots.
    Declarative {
        /// Command name without leading backslash
        name: String,
        /// Additional argument slots owned by the declarative node
        args: Vec<ArgumentSlot>,
    },
    /// Environment node whose body must always be a group.
    Environment {
        /// Environment name without `begin` / `end`
        name: String,
        /// Parsed argument slots attached to the environment
        args: Vec<ArgumentSlot>,
        /// Whether this name is present in the knowledge base.
        known: bool,
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
    /// Text-mode text chunk
    Text(String),
    /// Single character node
    Char(char),
    /// Active `~` space node
    ActiveSpace,
    /// Parser-produced error placeholder, mirroring
    /// [`SyntaxNode::Error`].
    ///
    /// `Error` is a first-class structural leaf: a tree containing `Error`
    /// nodes is still structurally valid. Semantic completeness (the absence of
    /// `Error` nodes) is a separate property, not a structural invariant.
    /// `Error` carries the original recovery message and the source snippet so
    /// serialization can round-trip it losslessly.
    Error {
        /// Human-readable recovery message captured by the parser.
        message: String,
        /// Verbatim source slice the parser failed to interpret.
        snippet: String,
    },
}

impl Node {
    /// Return the lightweight discriminant for this node.
    pub const fn kind(&self) -> NodeKind {
        match self {
            Node::Root { .. } => NodeKind::Root,
            Node::Group { .. } => NodeKind::Group,
            Node::Command { .. } => NodeKind::Command,
            Node::Infix { .. } => NodeKind::Infix,
            Node::Declarative { .. } => NodeKind::Declarative,
            Node::Environment { .. } => NodeKind::Environment,
            Node::Scripted { .. } => NodeKind::Scripted,
            Node::Text(_) => NodeKind::Text,
            Node::Char(_) => NodeKind::Char,
            Node::ActiveSpace => NodeKind::ActiveSpace,
            Node::Error { .. } => NodeKind::Error,
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
    /// Create an empty AST with a math-mode root node.
    pub fn new() -> Self {
        Self::with_root_mode(ContentMode::Math)
    }

    /// Create an empty AST whose root uses the given content mode.
    pub fn with_root_mode(mode: ContentMode) -> Self {
        let mut nodes = HopSlotMap::with_key();
        let root = nodes.insert(Node::Root {
            children: Vec::new(),
            mode,
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
    /// - content argument variants are converted directly to the referenced node
    /// - single-node content is not wrapped in an implicit group
    /// - `Delimiter::Control(&'static str)` becomes [`Delimiter::Control`] with
    ///   owned `String` data
    /// - [`SyntaxNode::Error`] is preserved losslessly as [`Node::Error`]; the
    ///   resulting tree is structurally valid but semantically incomplete
    ///
    /// # Panics
    ///
    /// Panics if the converted tree violates AST invariants, such as an
    /// environment body that is not a group.
    pub fn from_syntax_root(node: &SyntaxNode) -> Self {
        let SyntaxNode::Root { mode, children } = node else {
            panic!("Ast::from_syntax_root expects SyntaxNode::Root");
        };

        let mut nodes = HopSlotMap::with_key();
        let mut parent = SecondaryMap::new();
        let converted_children: Vec<NodeId> = children
            .iter()
            .map(|child| Self::convert_syntax_node(child, &mut nodes, &mut parent))
            .collect();
        let root = nodes.insert(Node::Root {
            children: converted_children,
            mode: *mode,
        });

        for (child, slot) in Self::node_edges(nodes.get(root).expect("Converted node must exist")) {
            parent.insert(child, ParentLink { parent: root, slot });
        }

        let ast = Ast {
            nodes,
            parent,
            detached_roots: HashSet::new(),
            root,
        };
        ast.assert_invariants();
        ast
    }

    /// Convert this AST back into a [`SyntaxNode`] tree.
    pub fn to_syntax_root(&self) -> SyntaxNode {
        let Node::Root { children, mode } = self.node(self.root) else {
            unreachable!("root must be a Root node");
        };
        SyntaxNode::Root {
            mode: *mode,
            children: children
                .iter()
                .map(|child| self.to_syntax_node(*child))
                .collect(),
        }
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

    /// Non-panicking node borrow; returns `None` if `id` is invalid.
    pub fn node_opt(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Non-panicking mutable node borrow; returns `None` if `id` is invalid.
    ///
    /// Direct mutation through this helper must not change a node's edges.
    /// Structural changes must keep using the edge-aware AST methods.
    pub fn node_opt_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Whether `id` is currently tracked as a detached root.
    pub fn is_detached_root(&self, id: NodeId) -> bool {
        self.detached_roots.contains(&id)
    }

    /// Whether `target` lies in the subtree rooted at `root`, inclusive.
    pub fn subtree_contains_node(&self, root: NodeId, target: NodeId) -> bool {
        self.subtree_contains(root, target)
    }

    /// Return the parent link for `id`, if the node is attached.
    ///
    /// Root, detached roots, and invalid or removed IDs return `None`. Callers
    /// that need to distinguish a valid detached root from an invalid ID should
    /// check [`Ast::contains`] first.
    pub fn parent(&self, id: NodeId) -> Option<ParentLink> {
        self.parent.get(id).copied()
    }

    /// Return the parent node ID for `id`, if attached.
    ///
    /// See [`Ast::parent`] for the `None` cases.
    pub fn parent_id(&self, id: NodeId) -> Option<NodeId> {
        self.parent(id).map(|link| link.parent)
    }

    /// Return the slot occupied by `id` in its parent, if attached.
    ///
    /// See [`Ast::parent`] for the `None` cases.
    pub fn slot(&self, id: NodeId) -> Option<Slot> {
        self.parent(id).map(|link| link.slot)
    }

    /// Return the direct children of a root/group node.
    ///
    /// Other node kinds return an empty slice.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        match self.node(id) {
            Node::Root { children, .. } | Node::Group { children, .. } => children,
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
    /// Content argument entries are exposed as argument edges.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn edges(&self, id: NodeId) -> Vec<(NodeId, Slot)> {
        Self::node_edges(self.node(id))
    }

    /// Deep-copy the subtree rooted at `id` and return the copied detached root.
    ///
    /// The cloned tree preserves node shape and scalar argument values, but every
    /// copied node receives a fresh [`NodeId`]. The returned root is detached, so
    /// callers can attach it with [`Ast::new_node`], [`Ast::replace_node`], or
    /// group insertion helpers.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid or points at the main AST root.
    pub fn clone_subtree(&mut self, id: NodeId) -> NodeId {
        if id == self.root {
            panic!("Cannot clone root node as a detached subtree");
        }
        if !self.contains(id) {
            panic!("Invalid NodeId");
        }

        self.clone_subtree_impl(id)
    }

    /// Return the next sibling of `id` when it is attached as a group child.
    ///
    /// Nodes in non-`GroupChild` slots and invalid or removed IDs return
    /// `None`.
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent_link = self.parent(id)?;
        let Slot::GroupChild(index) = parent_link.slot else {
            return None;
        };

        self.children(parent_link.parent).get(index + 1).copied()
    }

    /// Return the previous sibling of `id` when it is attached as a group child.
    ///
    /// Nodes in non-`GroupChild` slots and invalid or removed IDs return
    /// `None`.
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

    /// Check whether `id` is a character node matching `expected`.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn is_char(&self, id: NodeId, expected: char) -> bool {
        matches!(self.node(id), Node::Char(ch) if *ch == expected)
    }

    /// Check whether the subtree rooted at `start` contains a command named `name`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is invalid.
    pub fn subtree_contains_command(&self, start: NodeId, name: &str) -> bool {
        self.find(
            start,
            |node| matches!(node, Node::Command { name: command_name, .. } if command_name == name),
        )
        .is_some()
    }

    /// Insert a new detached node into the arena and return its [`NodeId`].
    ///
    /// If `node` references child IDs, those children must already exist as
    /// detached roots. They are adopted by the new node immediately.
    ///
    /// # Panics
    ///
    /// These panics indicate a caller-side structural invariant violation. The
    /// AST is not guaranteed to be reusable after such a panic.
    ///
    /// Panics if:
    ///
    /// - the same child is referenced more than once
    /// - a referenced child does not exist
    /// - a referenced child is not a detached root
    /// - adopting a child would introduce a cycle
    /// - an environment body is not a group
    pub fn new_node(&mut self, node: Node) -> NodeId {
        if matches!(node, Node::Root { .. }) {
            panic!("Cannot create detached root node");
        }

        Self::assert_unique_direct_children(&node);
        let direct_children = Self::node_edges(&node);
        let id = self.nodes.insert(node);
        self.detached_roots.insert(id);

        for &(child, slot) in &direct_children {
            self.assert_child_is_detached_root(child);
            self.assert_no_cycle(child, id);
            self.assert_slot_shape(slot, child);
            self.adopt_child(id, child, slot);
        }

        id
    }

    /// Append `child` to the end of a root/group child list.
    ///
    /// `child` must currently be a detached root.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - `parent` is not a root/group node
    /// - `child` is invalid
    /// - `child` is already attached
    /// - `child` is not tracked as a detached root
    /// - attaching `child` would introduce a cycle
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.assert_child_is_detached_root(child);
        self.assert_no_cycle(child, parent);

        let index = match self.node_mut(parent) {
            Node::Root { children, .. } | Node::Group { children, .. } => {
                let index = children.len();
                children.push(child);
                index
            }
            _ => panic!("Parent is not a root/group node"),
        };

        self.adopt_child(parent, child, Slot::GroupChild(index));
    }

    /// Insert `child` at `index` within a root/group child list.
    ///
    /// `child` must currently be a detached root.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///
    /// - `parent` is not a root/group node
    /// - `index` is out of bounds for `Vec::insert`
    /// - `child` is invalid
    /// - `child` is already attached
    /// - `child` is not tracked as a detached root
    /// - attaching `child` would introduce a cycle
    pub fn insert_child(&mut self, parent: NodeId, index: usize, child: NodeId) {
        self.assert_child_is_detached_root(child);
        self.assert_no_cycle(child, parent);

        match self.node_mut(parent) {
            Node::Root { children, .. } | Node::Group { children, .. } => {
                children.insert(index, child);
            }
            _ => panic!("Parent is not a root/group node"),
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
            Node::Root { children, .. } | Node::Group { children, .. } => {
                assert_eq!(
                    children.get(index).copied(),
                    Some(id),
                    "Group child index must match detached node"
                );
                children.remove(index);
            }
            _ => panic!("Parent is not a root/group node"),
        }

        self.release_child_as_detached_root(id);
        self.reindex_group_children(parent_link.parent, index);
        id
    }

    /// Replace all direct children of a root/group node.
    ///
    /// New children may be detached roots or existing direct children of the
    /// same parent. Removed children are preserved as detached roots.
    pub fn replace_children(&mut self, parent: NodeId, children: Vec<NodeId>) -> Vec<NodeId> {
        let old_children = match self.node(parent) {
            Node::Root { children, .. } | Node::Group { children, .. } => children.clone(),
            _ => panic!("Parent is not a root/group node"),
        };
        let old_child_set: HashSet<NodeId> = old_children.iter().copied().collect();

        let mut seen = HashSet::new();
        for child in &children {
            assert!(
                seen.insert(*child),
                "Node cannot reference the same child twice"
            );
            assert!(self.contains(*child), "Invalid child NodeId");
            assert!(*child != self.root, "Cannot attach the AST root as a child");
            self.assert_no_cycle(*child, parent);

            if old_child_set.contains(child) {
                assert_eq!(
                    self.parent(*child),
                    Some(ParentLink {
                        parent,
                        slot: Slot::GroupChild(
                            old_children
                                .iter()
                                .position(|old_child| old_child == child)
                                .expect("old child should have an index")
                        )
                    }),
                    "Can only reuse direct children of the replaced parent"
                );
            } else {
                self.assert_child_is_detached_root(*child);
            }
        }

        let new_child_set: HashSet<NodeId> = children.iter().copied().collect();
        let removed: Vec<NodeId> = old_children
            .iter()
            .copied()
            .filter(|child| !new_child_set.contains(child))
            .collect();

        for child in &removed {
            self.release_child_as_detached_root(*child);
        }
        for child in &children {
            self.detached_roots.remove(child);
        }

        match self.node_mut(parent) {
            Node::Root {
                children: old_children,
                ..
            }
            | Node::Group {
                children: old_children,
                ..
            } => *old_children = children,
            _ => panic!("Parent is not a root/group node"),
        }
        self.reindex_group_children(parent, 0);
        removed
    }

    /// Detach a contiguous range of direct children from a root/group node.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is not a root/group node or `range` is out of bounds
    /// for `Vec::drain`.
    pub fn detach_children_range(
        &mut self,
        parent: NodeId,
        range: std::ops::Range<usize>,
    ) -> Vec<NodeId> {
        let removed = match self.node_mut(parent) {
            Node::Root { children, .. } | Node::Group { children, .. } => {
                children.drain(range.clone()).collect::<Vec<_>>()
            }
            _ => panic!("Parent is not a root/group node"),
        };

        for child in &removed {
            self.release_child_as_detached_root(*child);
        }
        self.reindex_group_children(parent, range.start);
        removed
    }

    /// Replace a content child or another single-child slot.
    ///
    /// The replacement must be a detached root. The old child becomes detached.
    pub fn replace_content_child(&mut self, old: NodeId, replacement: NodeId) {
        if old == self.root {
            panic!("Cannot replace root node");
        }
        let parent_link = self
            .parent(old)
            .unwrap_or_else(|| panic!("Cannot replace node without a parent"));
        self.assert_child_is_detached_root(replacement);
        self.assert_no_cycle(replacement, parent_link.parent);
        self.assert_slot_shape(parent_link.slot, replacement);

        match self.node_mut(parent_link.parent) {
            Node::Command { args, .. }
            | Node::Infix { args, .. }
            | Node::Declarative { args, .. }
                if matches!(parent_link.slot, Slot::Argument(_)) =>
            {
                replace_argument_child(args, parent_link.slot, old, replacement);
            }
            Node::Infix { left, right, .. } => match parent_link.slot {
                Slot::InfixLeft => {
                    assert_eq!(*left, old, "Infix left operand must match old node");
                    *left = replacement;
                }
                Slot::InfixRight => {
                    assert_eq!(*right, old, "Infix right operand must match old node");
                    *right = replacement;
                }
                _ => panic!("Expected infix operand slot"),
            },
            Node::Environment { args, body, .. } => match parent_link.slot {
                Slot::Argument(_) => {
                    replace_argument_child(args, parent_link.slot, old, replacement)
                }
                Slot::EnvBody => {
                    assert_eq!(*body, old, "Environment body must match old node");
                    *body = replacement;
                }
                _ => panic!("Expected environment body or argument slot"),
            },
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => match parent_link.slot {
                Slot::ScriptBase => {
                    assert_eq!(*base, old, "Script base must match old node");
                    *base = replacement;
                }
                Slot::ScriptSub => {
                    assert_eq!(
                        subscript,
                        &Some(old),
                        "Script subscript must match old node"
                    );
                    *subscript = Some(replacement);
                }
                Slot::ScriptSup => {
                    assert_eq!(
                        superscript,
                        &Some(old),
                        "Script superscript must match old node"
                    );
                    *superscript = Some(replacement);
                }
                _ => panic!("Expected script slot"),
            },
            _ => panic!("Parent does not have replaceable content children"),
        }

        self.release_child_as_detached_root(old);
        self.adopt_child(parent_link.parent, replacement, parent_link.slot);
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
    /// These panics indicate a caller-side structural invariant violation. The
    /// AST is not guaranteed to be reusable after such a panic.
    ///
    /// Panics if:
    ///
    /// - `id` is the main root or invalid
    /// - the new node references the same child more than once
    /// - the new node is a root variant
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
        if matches!(new_node, Node::Root { .. }) {
            panic!("Cannot replace node with root variant");
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
                self.release_child_as_detached_root(child);
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

    /// Appends cloned math content into `out`, flattening implicit math groups.
    ///
    /// Parser-created content arguments often wrap multiple items in an
    /// implicit math group. Flattening that wrapper lets transforms compose
    /// output such as `\partial f` without introducing extra braces around `f`.
    pub fn append_cloned_math_content(&mut self, out: &mut Vec<NodeId>, source: NodeId) {
        match self.node(source) {
            Node::Group {
                children,
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            } => {
                let children = children.clone();
                out.extend(children.into_iter().map(|child| self.clone_subtree(child)));
            }
            _ => out.push(self.clone_subtree(source)),
        }
    }

    /// Creates an implicit math group containing `children`.
    pub fn implicit_math_group(&mut self, children: Vec<NodeId>) -> NodeId {
        self.new_node(Node::Group {
            children,
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        })
    }

    /// Creates a scripted node with only a superscript.
    pub fn superscript(&mut self, base: NodeId, superscript: NodeId) -> NodeId {
        self.new_node(Node::Scripted {
            base,
            subscript: None,
            superscript: Some(superscript),
        })
    }

    /// Replaces `id` and removes any old child subtrees detached by the replacement.
    pub fn replace_node_drop_detached_children(&mut self, id: NodeId, replacement: Node) {
        let old_children: Vec<NodeId> =
            self.edges(id).into_iter().map(|(child, _)| child).collect();
        self.replace_node(id, replacement);
        for child in old_children {
            if self.parent(child).is_none() {
                self.remove_detached(child);
            }
        }
    }

    /// Replaces a node with a math-mode sequence.
    ///
    /// Every node in `before`, `replacement`, and `after` must be a unique
    /// detached root. If `id` is a group child, `before` and `after` are
    /// inserted as real siblings around the replacement payload, and
    /// `replacement` is consumed into `id`. In single-child slots, the sequence
    /// is wrapped in an implicit math group because those slots cannot hold
    /// siblings.
    pub fn replace_with_math_sequence(
        &mut self,
        id: NodeId,
        before: Vec<NodeId>,
        replacement: NodeId,
        after: Vec<NodeId>,
    ) {
        self.assert_detached_root_sequence(
            before
                .iter()
                .copied()
                .chain(std::iter::once(replacement))
                .chain(after.iter().copied()),
        );

        match self.parent(id).map(|link| link.slot) {
            Some(Slot::GroupChild(index)) => {
                let parent = self
                    .parent_id(id)
                    .expect("group child should have a parent");
                let before_len = before.len();
                let replacement_node = self.take_detached_root_node(replacement);

                self.replace_node_drop_detached_children(id, replacement_node);
                for (offset, child) in before.into_iter().enumerate() {
                    self.insert_child(parent, index + offset, child);
                }
                for (offset, child) in after.into_iter().enumerate() {
                    self.insert_child(parent, index + before_len + 1 + offset, child);
                }
            }
            _ => {
                let mut children = before;
                children.push(replacement);
                children.extend(after);
                self.replace_node_drop_detached_children(
                    id,
                    Node::Group {
                        children,
                        kind: GroupKind::Implicit,
                        mode: ContentMode::Math,
                    },
                );
            }
        }
    }

    /// Replaces a node with a math-mode sequence, moving scripts from a
    /// parent [`Node::Scripted`] onto the final emitted node when `id` is the
    /// scripted base.
    ///
    /// Every node in `before`, `first`, and `after` must be a unique detached
    /// root.
    pub fn replace_with_math_sequence_preserving_scripts(
        &mut self,
        id: NodeId,
        before: Vec<NodeId>,
        first: NodeId,
        mut after: Vec<NodeId>,
    ) {
        self.assert_detached_root_sequence(
            before
                .iter()
                .copied()
                .chain(std::iter::once(first))
                .chain(after.iter().copied()),
        );

        if self.slot(id) != Some(Slot::ScriptBase) {
            self.replace_with_math_sequence(id, before, first, after);
            return;
        }

        let Some(parent) = self.parent_id(id) else {
            self.replace_with_math_sequence(id, before, first, after);
            return;
        };
        let Node::Scripted {
            subscript,
            superscript,
            ..
        } = self.node(parent)
        else {
            self.replace_with_math_sequence(id, before, first, after);
            return;
        };
        let subscript = *subscript;
        let superscript = *superscript;
        let subscript = subscript.map(|node_id| self.clone_subtree(node_id));
        let superscript = superscript.map(|node_id| self.clone_subtree(node_id));

        // Fixed fences carry scripts on the closing token, which is the final
        // node of these replacement sequences.
        let last = after.pop().unwrap_or(first);
        let scripted_last = self.new_node(Node::Scripted {
            base: last,
            subscript,
            superscript,
        });
        if after.is_empty() && last == first {
            self.replace_with_math_sequence(parent, before, scripted_last, after);
        } else {
            after.push(scripted_last);
            self.replace_with_math_sequence(parent, before, first, after);
        }
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
            matches!(self.node(self.root), Node::Root { .. }),
            "ast.root() must be Node::Root"
        );
        assert!(
            self.parent(self.root).is_none(),
            "Root node must not have a parent"
        );
        assert!(
            !self.detached_roots.contains(&self.root),
            "Root node cannot be a detached root"
        );

        for (id, node) in self.nodes.iter() {
            if matches!(node, Node::Root { .. }) {
                assert_eq!(id, self.root, "Only ast.root() may be Node::Root");
            }
        }

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

    fn assert_detached_root_sequence(&self, nodes: impl IntoIterator<Item = NodeId>) {
        let mut seen = HashSet::new();
        for node in nodes {
            assert!(
                seen.insert(node),
                "Node cannot appear in a replacement sequence twice"
            );
            self.assert_child_is_detached_root(node);
        }
    }

    fn adopt_child(&mut self, parent: NodeId, child: NodeId, slot: Slot) {
        self.detached_roots.remove(&child);
        self.parent.insert(child, ParentLink { parent, slot });
    }

    fn release_child_as_detached_root(&mut self, child: NodeId) {
        self.parent.remove(child);
        self.detached_roots.insert(child);
    }

    fn take_detached_root_node(&mut self, id: NodeId) -> Node {
        if id == self.root {
            panic!("Cannot consume root node as detached replacement");
        }
        if !self.contains(id) {
            panic!("Invalid NodeId");
        }
        if self.parent(id).is_some() {
            panic!("Can only consume detached roots");
        }
        if !self.detached_roots.remove(&id) {
            panic!("Node is not a detached root");
        }

        let node = self.nodes.remove(id).expect("Detached root must exist");
        for (child, _) in Self::node_edges(&node) {
            let link = self
                .parent
                .remove(child)
                .expect("Detached root child should have a parent link");
            assert_eq!(
                link.parent, id,
                "Detached replacement child must point at the consumed root"
            );
            self.release_child_as_detached_root(child);
        }
        node
    }

    fn reindex_group_children(&mut self, parent: NodeId, start: usize) {
        let children = match self.node(parent) {
            Node::Root { children, .. } | Node::Group { children, .. } => children.clone(),
            _ => panic!("Parent is not a root/group node"),
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

    fn to_syntax_node(&self, id: NodeId) -> SyntaxNode {
        match self.node(id) {
            Node::Root { .. } => unreachable!("nested Root is impossible"),
            Node::Group {
                children,
                kind,
                mode,
            } => SyntaxNode::Group {
                mode: *mode,
                kind: self.to_syntax_group_kind(kind),
                children: children
                    .iter()
                    .map(|child| self.to_syntax_node(*child))
                    .collect(),
            },
            Node::Command { name, args, known } => SyntaxNode::Command {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| self.to_syntax_arg_slot(slot))
                    .collect(),
                known: *known,
            },
            Node::Infix {
                name,
                args,
                left,
                right,
            } => SyntaxNode::Infix {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| self.to_syntax_arg_slot(slot))
                    .collect(),
                left: Box::new(self.to_syntax_node(*left)),
                right: Box::new(self.to_syntax_node(*right)),
            },
            Node::Declarative { name, args } => SyntaxNode::Declarative {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| self.to_syntax_arg_slot(slot))
                    .collect(),
            },
            Node::Environment {
                name,
                args,
                known,
                body,
            } => SyntaxNode::Environment {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| self.to_syntax_arg_slot(slot))
                    .collect(),
                known: *known,
                body: Box::new(self.to_syntax_node(*body)),
            },
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => SyntaxNode::Scripted {
                base: Box::new(self.to_syntax_node(*base)),
                subscript: subscript.map(|node| Box::new(self.to_syntax_node(node))),
                superscript: superscript.map(|node| Box::new(self.to_syntax_node(node))),
            },
            Node::Text(text) => SyntaxNode::Text(text.clone()),
            Node::Char(ch) => SyntaxNode::Char(*ch),
            Node::ActiveSpace => SyntaxNode::ActiveSpace,
            Node::Error { message, snippet } => SyntaxNode::Error {
                message: message.clone(),
                snippet: snippet.clone(),
            },
        }
    }

    fn to_syntax_arg_slot(&self, slot: &ArgumentSlot) -> syntax_node::ArgumentSlot {
        slot.as_ref().map(|argument| syntax_node::Argument {
            kind: self.to_syntax_arg_kind(&argument.kind),
            value: self.to_syntax_arg_value(&argument.value),
        })
    }

    fn to_syntax_arg_kind(&self, kind: &ArgumentKind) -> syntax_node::ArgumentKind {
        match kind {
            ArgumentKind::Mandatory => syntax_node::ArgumentKind::Mandatory,
            ArgumentKind::Optional => syntax_node::ArgumentKind::Optional,
            ArgumentKind::Star => syntax_node::ArgumentKind::Star,
            ArgumentKind::Group => syntax_node::ArgumentKind::Group,
            ArgumentKind::Delimited { open, close } => syntax_node::ArgumentKind::Delimited {
                open: self.to_syntax_delimiter(open),
                close: self.to_syntax_delimiter(close),
            },
            ArgumentKind::Paired { open, close } => syntax_node::ArgumentKind::Paired {
                open: self.to_syntax_delimiter(open),
                close: self.to_syntax_delimiter(close),
            },
        }
    }

    fn to_syntax_arg_value(&self, value: &ArgumentValue) -> syntax_node::ArgumentValue {
        match value {
            ArgumentValue::MathContent(id) => {
                syntax_node::ArgumentValue::MathContent(self.to_syntax_node(*id))
            }
            ArgumentValue::TextContent(id) => {
                syntax_node::ArgumentValue::TextContent(self.to_syntax_node(*id))
            }
            ArgumentValue::Delimiter(delimiter) => {
                syntax_node::ArgumentValue::Delimiter(self.to_syntax_delimiter(delimiter))
            }
            ArgumentValue::CSName(value) => syntax_node::ArgumentValue::CSName(value.clone()),
            ArgumentValue::Dimension(value) => syntax_node::ArgumentValue::Dimension(value.clone()),
            ArgumentValue::Integer(value) => syntax_node::ArgumentValue::Integer(value.clone()),
            ArgumentValue::KeyVal(value) => syntax_node::ArgumentValue::KeyVal(value.clone()),
            ArgumentValue::Column(value) => syntax_node::ArgumentValue::Column(value.clone()),
            ArgumentValue::Boolean(value) => syntax_node::ArgumentValue::Boolean(*value),
        }
    }

    fn to_syntax_group_kind(&self, kind: &GroupKind) -> syntax_node::GroupKind {
        match kind {
            GroupKind::Explicit => syntax_node::GroupKind::Explicit,
            GroupKind::Implicit => syntax_node::GroupKind::Implicit,
            GroupKind::Delimited { left, right } => syntax_node::GroupKind::Delimited {
                left: self.to_syntax_delimiter(left),
                right: self.to_syntax_delimiter(right),
            },
            GroupKind::InlineMath => syntax_node::GroupKind::InlineMath,
        }
    }

    fn to_syntax_delimiter(&self, delimiter: &Delimiter) -> syntax_node::Delimiter {
        match delimiter {
            Delimiter::None => syntax_node::Delimiter::None,
            Delimiter::Char(ch) => syntax_node::Delimiter::Char(*ch),
            Delimiter::Control(name) => {
                syntax_node::Delimiter::Control(Box::leak(name.clone().into_boxed_str()))
            }
        }
    }

    fn clone_subtree_impl(&mut self, id: NodeId) -> NodeId {
        let cloned = match self.node(id).clone() {
            Node::Root { .. } => panic!("Cannot clone root node as a detached subtree"),
            Node::Group {
                children,
                kind,
                mode,
            } => Node::Group {
                children: children
                    .into_iter()
                    .map(|child| self.clone_subtree_impl(child))
                    .collect(),
                kind,
                mode,
            },
            Node::Command { name, args, known } => Node::Command {
                name,
                args: self.clone_argument_slots(args),
                known,
            },
            Node::Infix {
                name,
                args,
                left,
                right,
            } => Node::Infix {
                name,
                args: self.clone_argument_slots(args),
                left: self.clone_subtree_impl(left),
                right: self.clone_subtree_impl(right),
            },
            Node::Declarative { name, args } => Node::Declarative {
                name,
                args: self.clone_argument_slots(args),
            },
            Node::Environment {
                name,
                args,
                known,
                body,
            } => Node::Environment {
                name,
                args: self.clone_argument_slots(args),
                known,
                body: self.clone_subtree_impl(body),
            },
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => Node::Scripted {
                base: self.clone_subtree_impl(base),
                subscript: subscript.map(|child| self.clone_subtree_impl(child)),
                superscript: superscript.map(|child| self.clone_subtree_impl(child)),
            },
            Node::Text(text) => Node::Text(text),
            Node::Char(ch) => Node::Char(ch),
            Node::ActiveSpace => Node::ActiveSpace,
            Node::Error { message, snippet } => Node::Error { message, snippet },
        };

        self.new_node(cloned)
    }

    fn clone_argument_slots(&mut self, args: Vec<ArgumentSlot>) -> Vec<ArgumentSlot> {
        args.into_iter()
            .map(|slot| {
                slot.map(|arg| Argument {
                    kind: arg.kind,
                    value: self.clone_argument_value(arg.value),
                })
            })
            .collect()
    }

    fn clone_argument_value(&mut self, value: ArgumentValue) -> ArgumentValue {
        match value {
            ArgumentValue::MathContent(child) => {
                ArgumentValue::MathContent(self.clone_subtree_impl(child))
            }
            ArgumentValue::TextContent(child) => {
                ArgumentValue::TextContent(self.clone_subtree_impl(child))
            }
            ArgumentValue::Delimiter(delimiter) => ArgumentValue::Delimiter(delimiter),
            ArgumentValue::CSName(name) => ArgumentValue::CSName(name),
            ArgumentValue::Dimension(value) => ArgumentValue::Dimension(value),
            ArgumentValue::Integer(value) => ArgumentValue::Integer(value),
            ArgumentValue::KeyVal(value) => ArgumentValue::KeyVal(value),
            ArgumentValue::Column(value) => ArgumentValue::Column(value),
            ArgumentValue::Boolean(value) => ArgumentValue::Boolean(value),
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
            Node::Root { children, .. } | Node::Group { children, .. } => {
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
            Node::Declarative { args, .. } => {
                Self::push_argument_edges(args, &mut edges);
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
            Node::Text(_) | Node::Char(_) | Node::ActiveSpace | Node::Error { .. } => {}
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
            match &argument.value {
                ArgumentValue::MathContent(child) | ArgumentValue::TextContent(child) => {
                    edges.push((*child, Slot::Argument(index)));
                }
                _ => {}
            }
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
            SyntaxNode::Root { .. } => {
                panic!("Ast::from_syntax_root does not accept nested SyntaxNode::Root")
            }
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
            SyntaxNode::Command { name, args, known } => Node::Command {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| Self::convert_argument_slot(slot, nodes, parent))
                    .collect(),
                known: *known,
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
            SyntaxNode::Declarative { name, args } => Node::Declarative {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| Self::convert_argument_slot(slot, nodes, parent))
                    .collect(),
            },
            SyntaxNode::Environment {
                name,
                args,
                known,
                body,
            } => Node::Environment {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|slot| Self::convert_argument_slot(slot, nodes, parent))
                    .collect(),
                known: *known,
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
            SyntaxNode::Error { message, snippet } => Node::Error {
                message: message.clone(),
                snippet: snippet.clone(),
            },
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
            syntax_node::ArgumentValue::MathContent(node) => {
                ArgumentValue::MathContent(Self::convert_syntax_node(node, nodes, parent))
            }
            syntax_node::ArgumentValue::TextContent(node) => {
                ArgumentValue::TextContent(Self::convert_syntax_node(node, nodes, parent))
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

fn replace_argument_child(args: &mut [ArgumentSlot], slot: Slot, old: NodeId, replacement: NodeId) {
    let Slot::Argument(index) = slot else {
        panic!("Expected argument slot");
    };
    let argument = args
        .get_mut(index)
        .and_then(Option::as_mut)
        .unwrap_or_else(|| panic!("Argument slot is missing"));
    match &mut argument.value {
        ArgumentValue::MathContent(child) | ArgumentValue::TextContent(child) => {
            assert_eq!(*child, old, "Argument child must match old node");
            *child = replacement;
        }
        _ => panic!("Argument slot is not content"),
    }
}

impl Default for Ast {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Argument, ArgumentKind, ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeKind, Slot,
    };

    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        match payload.downcast::<String>() {
            Ok(message) => *message,
            Err(payload) => match payload.downcast::<&'static str>() {
                Ok(message) => message.to_string(),
                Err(_) => "non-string panic payload".to_string(),
            },
        }
    }

    #[test]
    fn error_node_reports_error_kind() {
        let node = Node::Error {
            message: "unexpected token".to_string(),
            snippet: r"\frac".to_string(),
        };

        assert_eq!(node.kind(), NodeKind::Error);
    }

    #[test]
    fn error_node_is_a_leaf() {
        let mut ast = Ast::new();
        let error = ast.new_node(Node::Error {
            message: "bad".to_string(),
            snippet: "x".to_string(),
        });
        ast.append_child(ast.root(), error);

        assert!(ast.edges(error).is_empty());
        ast.assert_invariants();
    }

    #[test]
    fn clone_subtree_clones_error_node() {
        let mut ast = Ast::new();
        let error = ast.new_node(Node::Error {
            message: "bad".to_string(),
            snippet: "x".to_string(),
        });

        let cloned = ast.clone_subtree(error);

        assert_ne!(cloned, error);
        assert_eq!(
            ast.node(cloned),
            &Node::Error {
                message: "bad".to_string(),
                snippet: "x".to_string(),
            }
        );
        ast.assert_invariants();
    }

    #[test]
    fn from_syntax_root_converts_error_nodes() {
        use texform_interface::syntax_node::{ContentMode as SynMode, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: SynMode::Math,
            children: vec![
                SyntaxNode::Char('a'),
                SyntaxNode::Error {
                    message: "unexpected".to_string(),
                    snippet: r"\bad".to_string(),
                },
            ],
        };

        let ast = Ast::from_syntax_root(&syntax);
        let children = ast.children(ast.root());

        assert_eq!(children.len(), 2);
        assert_eq!(ast.node(children[0]), &Node::Char('a'));
        assert_eq!(
            ast.node(children[1]),
            &Node::Error {
                message: "unexpected".to_string(),
                snippet: r"\bad".to_string(),
            }
        );
        ast.assert_invariants();
    }

    #[test]
    #[should_panic(expected = "Only ast.root() may be Node::Root")]
    fn assert_invariants_rejects_additional_root_nodes() {
        let mut ast = Ast::new();
        let extra_root = ast.nodes.insert(Node::Root {
            children: Vec::new(),
            mode: ContentMode::Math,
        });
        ast.detached_roots.insert(extra_root);

        ast.assert_invariants();
    }

    #[test]
    fn clone_subtree_creates_detached_copy_with_rewired_children() {
        let mut ast = Ast::new();
        let numerator_child = ast.new_node(Node::Char('x'));
        let numerator = ast.new_node(Node::Group {
            children: vec![numerator_child],
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        let denominator = ast.new_node(Node::Char('t'));
        let frac = ast.new_node(Node::Command {
            name: "frac".to_string(),
            args: vec![
                Some(Argument {
                    kind: ArgumentKind::Mandatory,
                    value: ArgumentValue::MathContent(numerator),
                }),
                Some(Argument {
                    kind: ArgumentKind::Mandatory,
                    value: ArgumentValue::MathContent(denominator),
                }),
            ],
            known: true,
        });

        let cloned = ast.clone_subtree(frac);

        assert_ne!(cloned, frac);
        assert_eq!(ast.parent(cloned), None);
        assert!(ast.detached_roots.contains(&cloned));

        let Node::Command { args, .. } = ast.node(cloned) else {
            panic!("cloned root should be a command");
        };
        let ArgumentValue::MathContent(cloned_numerator) =
            args[0].as_ref().expect("first argument should exist").value
        else {
            panic!("first argument should be math content");
        };
        let ArgumentValue::MathContent(cloned_denominator) = args[1]
            .as_ref()
            .expect("second argument should exist")
            .value
        else {
            panic!("second argument should be math content");
        };

        assert_ne!(cloned_numerator, numerator);
        assert_ne!(cloned_denominator, denominator);
        assert_eq!(
            ast.parent(cloned_numerator),
            Some(super::ParentLink {
                parent: cloned,
                slot: Slot::Argument(0),
            })
        );
        assert_eq!(
            ast.parent(cloned_denominator),
            Some(super::ParentLink {
                parent: cloned,
                slot: Slot::Argument(1),
            })
        );

        let cloned_numerator_children = ast.children(cloned_numerator);
        assert_eq!(cloned_numerator_children.len(), 1);
        assert_ne!(cloned_numerator_children[0], numerator_child);
        assert_eq!(
            ast.parent(cloned_numerator_children[0]),
            Some(super::ParentLink {
                parent: cloned_numerator,
                slot: Slot::GroupChild(0),
            })
        );

        ast.assert_invariants();
    }

    #[test]
    fn append_cloned_math_content_flattens_implicit_groups() {
        let mut ast = Ast::new();
        let x = ast.new_node(Node::Char('x'));
        let y = ast.new_node(Node::Char('y'));
        let source = ast.new_node(Node::Group {
            children: vec![x, y],
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        let mut out = Vec::new();

        ast.append_cloned_math_content(&mut out, source);

        assert_eq!(out.len(), 2);
        assert_ne!(out[0], x);
        assert_ne!(out[1], y);
        assert_eq!(ast.node(out[0]), &Node::Char('x'));
        assert_eq!(ast.node(out[1]), &Node::Char('y'));
        assert_eq!(ast.parent(out[0]), None);
        assert_eq!(ast.parent(out[1]), None);
        ast.assert_invariants();
    }

    #[test]
    fn constructs_implicit_math_group() {
        let mut ast = Ast::new();
        let x = ast.new_node(Node::Char('x'));
        let y = ast.new_node(Node::Char('y'));

        let group = ast.implicit_math_group(vec![x, y]);

        assert_eq!(
            ast.node(group),
            &Node::Group {
                children: vec![x, y],
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            }
        );
        assert_eq!(ast.parent_id(x), Some(group));
        assert_eq!(ast.parent_id(y), Some(group));
        ast.assert_invariants();
    }

    #[test]
    fn constructs_superscript_node() {
        let mut ast = Ast::new();
        let base = ast.new_node(Node::Char('a'));
        let power = ast.new_node(Node::Char('2'));

        let scripted = ast.superscript(base, power);

        assert_eq!(
            ast.node(scripted),
            &Node::Scripted {
                base,
                subscript: None,
                superscript: Some(power),
            }
        );
        assert_eq!(ast.parent_id(base), Some(scripted));
        assert_eq!(ast.parent_id(power), Some(scripted));
        ast.assert_invariants();
    }

    #[test]
    fn replace_children_detaches_removed_children_and_adopts_new_children() {
        let mut ast = Ast::new();
        let root = ast.root();
        let a = ast.new_node(Node::Char('a'));
        let b = ast.new_node(Node::Char('b'));
        let c = ast.new_node(Node::Char('c'));
        ast.append_child(root, a);
        ast.append_child(root, b);

        let removed = ast.replace_children(root, vec![b, c]);

        assert_eq!(removed, vec![a]);
        assert_eq!(ast.children(root), &[b, c]);
        assert_eq!(ast.parent(a), None);
        assert!(ast.detached_roots.contains(&a));
        assert_eq!(
            ast.parent(b),
            Some(super::ParentLink {
                parent: root,
                slot: Slot::GroupChild(0),
            })
        );
        assert_eq!(
            ast.parent(c),
            Some(super::ParentLink {
                parent: root,
                slot: Slot::GroupChild(1),
            })
        );
        ast.assert_invariants();
    }

    #[test]
    fn detach_children_range_detaches_ordered_segment() {
        let mut ast = Ast::new();
        let root = ast.root();
        let a = ast.new_node(Node::Char('a'));
        let b = ast.new_node(Node::Char('b'));
        let c = ast.new_node(Node::Char('c'));
        let d = ast.new_node(Node::Char('d'));
        for child in [a, b, c, d] {
            ast.append_child(root, child);
        }

        let removed = ast.detach_children_range(root, 1..3);

        assert_eq!(removed, vec![b, c]);
        assert_eq!(ast.children(root), &[a, d]);
        assert_eq!(ast.parent(b), None);
        assert_eq!(ast.parent(c), None);
        assert!(ast.detached_roots.contains(&b));
        assert!(ast.detached_roots.contains(&c));
        assert_eq!(ast.slot(d), Some(Slot::GroupChild(1)));
        ast.assert_invariants();
    }

    #[test]
    fn detach_panics_without_removing_wrong_child_when_parent_link_index_is_stale() {
        let mut ast = Ast::new();
        let root = ast.root();
        let a = ast.new_node(Node::Char('a'));
        let b = ast.new_node(Node::Char('b'));
        ast.append_child(root, a);
        ast.append_child(root, b);
        ast.parent.insert(
            b,
            super::ParentLink {
                parent: root,
                slot: Slot::GroupChild(0),
            },
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ast.detach(b)));

        let message = panic_message(result.expect_err("detach should reject stale child index"));
        assert!(
            message.contains("Group child index must match detached node"),
            "unexpected panic: {message}"
        );
        assert_eq!(ast.children(root), &[a, b]);
    }

    #[test]
    fn replace_content_child_replaces_script_slot() {
        let mut ast = Ast::new();
        let base = ast.new_node(Node::Char('x'));
        let superscript = ast.new_node(Node::Char('y'));
        let scripted = ast.new_node(Node::Scripted {
            base,
            subscript: None,
            superscript: Some(superscript),
        });
        ast.append_child(ast.root(), scripted);
        let replacement = ast.new_node(Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });

        ast.replace_content_child(superscript, replacement);

        assert_eq!(ast.parent(superscript), None);
        assert!(ast.detached_roots.contains(&superscript));
        assert_eq!(
            ast.parent(replacement),
            Some(super::ParentLink {
                parent: scripted,
                slot: Slot::ScriptSup,
            })
        );
        assert!(matches!(
            ast.node(scripted),
            Node::Scripted {
                superscript: Some(child),
                ..
            } if *child == replacement
        ));
        ast.assert_invariants();
    }

    #[test]
    fn replace_content_child_replaces_infix_operand() {
        let mut ast = Ast::new();
        let left = ast.new_node(Node::Char('a'));
        let right = ast.new_node(Node::Char('b'));
        let infix = ast.new_node(Node::Infix {
            name: "over".to_string(),
            args: Vec::new(),
            left,
            right,
        });
        ast.append_child(ast.root(), infix);
        let replacement = ast.new_node(Node::Char('x'));

        ast.replace_content_child(right, replacement);

        assert_eq!(ast.parent(right), None);
        assert!(ast.detached_roots.contains(&right));
        assert_eq!(
            ast.parent(replacement),
            Some(super::ParentLink {
                parent: infix,
                slot: Slot::InfixRight,
            })
        );
        assert!(matches!(
            ast.node(infix),
            Node::Infix {
                right: child,
                ..
            } if *child == replacement
        ));
        ast.assert_invariants();
    }

    #[test]
    fn replace_node_drop_detached_children_removes_old_subtree() {
        let mut ast = Ast::new();
        let old_child = ast.new_node(Node::Char('x'));
        let old_grandchild = ast.new_node(Node::Char('y'));
        let old_child = ast.new_node(Node::Group {
            children: vec![old_child, old_grandchild],
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        let target = ast.new_node(Node::Group {
            children: vec![old_child],
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        ast.append_child(ast.root(), target);
        let new_child = ast.new_node(Node::Char('z'));

        ast.replace_node_drop_detached_children(
            target,
            Node::Group {
                children: vec![new_child],
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            },
        );

        assert!(!ast.contains(old_child));
        assert!(!ast.contains(old_grandchild));
        assert_eq!(ast.parent_id(new_child), Some(target));
        assert_eq!(ast.children(target), &[new_child]);
        ast.assert_invariants();
    }

    #[test]
    fn replace_with_math_sequence_splices_group_children() {
        let mut ast = Ast::new();
        let target = ast.new_node(Node::Char('x'));
        let root = ast.root();
        ast.append_child(root, target);
        let before = ast.new_node(Node::Char('a'));
        let replacement = ast.new_node(Node::Char('b'));
        let after = ast.new_node(Node::Char('c'));

        ast.replace_with_math_sequence(target, vec![before], replacement, vec![after]);

        assert!(!ast.contains(replacement));
        assert_eq!(ast.children(root), &[before, target, after]);
        assert_eq!(ast.node(target), &Node::Char('b'));
        assert_eq!(ast.parent_id(before), Some(root));
        assert_eq!(ast.parent_id(after), Some(root));
        ast.assert_invariants();
    }

    #[test]
    fn replace_with_math_sequence_wraps_single_child_slots() {
        let mut ast = Ast::new();
        let target = ast.new_node(Node::Char('x'));
        let command = ast.new_node(Node::Command {
            name: "sqrt".to_string(),
            args: vec![Some(Argument {
                kind: ArgumentKind::Mandatory,
                value: ArgumentValue::MathContent(target),
            })],
            known: true,
        });
        ast.append_child(ast.root(), command);
        let before = ast.new_node(Node::Char('a'));
        let replacement = ast.new_node(Node::Char('b'));
        let after = ast.new_node(Node::Char('c'));

        ast.replace_with_math_sequence(target, vec![before], replacement, vec![after]);

        assert_eq!(
            ast.node(target),
            &Node::Group {
                children: vec![before, replacement, after],
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            }
        );
        assert_eq!(ast.parent_id(before), Some(target));
        assert_eq!(ast.parent_id(replacement), Some(target));
        assert_eq!(ast.parent_id(after), Some(target));
        ast.assert_invariants();
    }

    #[test]
    fn replace_with_math_sequence_rejects_duplicate_sequence_node_before_replacement() {
        let mut ast = Ast::new();
        let root = ast.root();
        let target = ast.new_node(Node::Char('x'));
        ast.append_child(root, target);
        let replacement = ast.new_node(Node::Char('y'));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ast.replace_with_math_sequence(target, vec![replacement], replacement, Vec::new());
        }));

        let message = panic_message(result.expect_err("duplicate sequence node should panic"));
        assert!(
            message.contains("Node cannot appear in a replacement sequence twice"),
            "unexpected panic: {message}"
        );
        assert_eq!(ast.node(target), &Node::Char('x'));
        assert_eq!(ast.children(root), &[target]);
        assert!(ast.contains(replacement));
        assert_eq!(ast.parent(replacement), None);
        assert!(ast.detached_roots.contains(&replacement));
    }

    #[test]
    fn replace_with_math_sequence_rejects_attached_before_node_before_replacement() {
        let mut ast = Ast::new();
        let root = ast.root();
        let target = ast.new_node(Node::Char('x'));
        let attached = ast.new_node(Node::Char('a'));
        ast.append_child(root, attached);
        ast.append_child(root, target);
        let replacement = ast.new_node(Node::Char('y'));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ast.replace_with_math_sequence(target, vec![attached], replacement, Vec::new());
        }));

        let message = panic_message(result.expect_err("attached sequence node should panic"));
        assert!(
            message.contains("Cannot attach child that already has a parent"),
            "unexpected panic: {message}"
        );
        assert_eq!(ast.node(target), &Node::Char('x'));
        assert_eq!(ast.children(root), &[attached, target]);
        assert!(ast.contains(replacement));
        assert_eq!(ast.parent(replacement), None);
        assert!(ast.detached_roots.contains(&replacement));
    }

    #[test]
    fn replace_with_math_sequence_preserving_scripts_rejects_duplicate_before_staging() {
        let mut ast = Ast::new();
        let base = ast.new_node(Node::Char('x'));
        let superscript = ast.new_node(Node::Char('2'));
        let scripted = ast.new_node(Node::Scripted {
            base,
            subscript: None,
            superscript: Some(superscript),
        });
        ast.append_child(ast.root(), scripted);
        let first = ast.new_node(Node::Char('['));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ast.replace_with_math_sequence_preserving_scripts(base, vec![first], first, Vec::new());
        }));

        let message = panic_message(result.expect_err("duplicate sequence node should panic"));
        assert!(
            message.contains("Node cannot appear in a replacement sequence twice"),
            "unexpected panic: {message}"
        );
        assert_eq!(
            ast.node(scripted),
            &Node::Scripted {
                base,
                subscript: None,
                superscript: Some(superscript),
            }
        );
        assert_eq!(ast.parent(first), None);
        assert!(ast.detached_roots.contains(&first));
    }

    #[test]
    fn replace_with_math_sequence_preserving_scripts_moves_scripts_to_last_node() {
        let mut ast = Ast::new();
        let base = ast.new_node(Node::Char('x'));
        let subscript = ast.new_node(Node::Char('i'));
        let superscript = ast.new_node(Node::Char('2'));
        let scripted = ast.new_node(Node::Scripted {
            base,
            subscript: Some(subscript),
            superscript: Some(superscript),
        });
        ast.append_child(ast.root(), scripted);
        let open = ast.new_node(Node::Char('['));
        let body = ast.new_node(Node::Char('y'));
        let close = ast.new_node(Node::Char(']'));

        ast.replace_with_math_sequence_preserving_scripts(
            base,
            Vec::new(),
            open,
            vec![body, close],
        );

        let root_children = ast.children(ast.root()).to_vec();
        assert_eq!(root_children.len(), 3);
        assert_eq!(root_children[0], scripted);
        assert_eq!(ast.node(scripted), &Node::Char('['));
        assert_eq!(root_children[1], body);
        let Node::Scripted {
            base: scripted_close,
            subscript: moved_subscript,
            superscript: moved_superscript,
        } = ast.node(root_children[2])
        else {
            panic!("expected scripts to move to the close token");
        };
        assert_eq!(ast.node(*scripted_close), &Node::Char(']'));
        assert_eq!(
            moved_subscript.map(|id| ast.node(id)),
            Some(&Node::Char('i'))
        );
        assert_eq!(
            moved_superscript.map(|id| ast.node(id)),
            Some(&Node::Char('2'))
        );
        ast.assert_invariants();
    }
}
