use super::*;

/// A node's position within its immediate parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeSlot {
    Child(usize),
    Arg(usize),
    ScriptBase,
    Subscript,
    Superscript,
    InfixLeft,
    InfixRight,
    EnvBody,
}

impl From<Slot> for NodeSlot {
    fn from(slot: Slot) -> Self {
        match slot {
            Slot::GroupChild(index) => Self::Child(index),
            Slot::Argument(index) => Self::Arg(index),
            Slot::ScriptBase => Self::ScriptBase,
            Slot::ScriptSub => Self::Subscript,
            Slot::ScriptSup => Self::Superscript,
            Slot::InfixLeft => Self::InfixLeft,
            Slot::InfixRight => Self::InfixRight,
            Slot::EnvBody => Self::EnvBody,
        }
    }
}

/// Writes the compact path segment used by conformance errors and
/// [`Document::node_spans`], such as `child.2`, `arg.0.content`, or `sub`.
impl std::fmt::Display for NodeSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Child(index) => write!(f, "child.{index}"),
            Self::Arg(index) => write!(f, "arg.{index}.content"),
            Self::ScriptBase => f.write_str("base"),
            Self::Subscript => f.write_str("sub"),
            Self::Superscript => f.write_str("sup"),
            Self::InfixLeft => f.write_str("left"),
            Self::InfixRight => f.write_str("right"),
            Self::EnvBody => f.write_str("body"),
        }
    }
}

impl Document {
    /// Context mode of `id`, derived from its ancestors' slots.
    pub(super) fn context_mode(&self, id: RawNodeId) -> ContentMode {
        if let Some(mode) = self.local_context(id) {
            return mode;
        }
        let Some(link) = self.ast.parent(id) else {
            assert!(
                id == self.ast.root(),
                "every detached subtree has a context mode"
            );
            return self.root_mode();
        };
        conformance::child_mode(
            self.ast.node(link.parent),
            link.slot,
            self.context_mode(link.parent),
            &self.knowledge_base,
        )
    }

    /// Context mode of `id` when known without walking ancestors: recorded for
    /// detached roots, and the container's mode for group children.
    pub(super) fn local_context(&self, id: RawNodeId) -> Option<ContentMode> {
        match self.ast.parent(id) {
            Some(ParentLink {
                parent,
                slot: Slot::GroupChild(_),
            }) => self.ast.node(parent).content_mode(),
            Some(_) => None,
            None => self.detached_modes.get(id).copied(),
        }
    }

    pub(super) fn conformance_path(&self, id: RawNodeId) -> String {
        let mut top = id;
        while let Some(parent) = self.ast.parent_id(top) {
            top = parent;
        }
        let label = if top == self.ast.root() {
            "root"
        } else {
            "detached"
        };
        path_below(&self.ast, top, id, label)
    }

    /// Check `node` in `mode` against its current direct children.
    ///
    /// `swap` stands a proposed node in for an existing child. Children with an
    /// O(1) context must be mounted in the context this node assigns.
    pub(super) fn check_local(
        &self,
        node: &Node,
        mode: ContentMode,
        swap: Option<(RawNodeId, &Node)>,
    ) -> Result<(), ConformanceError> {
        conformance::check_node(node, mode, &self.knowledge_base, |id| match swap {
            Some((old, new)) if old == id => (new, None),
            _ => (self.ast.node(id), self.local_context(id)),
        })
    }

    /// Audit the root-reachable document without changing it.
    #[doc(hidden)]
    pub fn __validate_conformance(&self) -> Result<(), ConformanceError> {
        conformance::check_tree(
            &self.ast,
            self.ast.root(),
            self.root_mode(),
            &self.knowledge_base,
            "root",
        )
    }

    pub(super) fn debug_assert_conformance(&self) {
        #[cfg(debug_assertions)]
        {
            if let Err(error) = self.__validate_conformance() {
                panic!("document failed conformance: {error}");
            }
            for (id, mode) in &self.detached_modes {
                assert!(
                    self.ast.is_detached_root(id),
                    "only detached roots record a context mode"
                );
                if let Err(error) =
                    conformance::check_tree(&self.ast, id, *mode, &self.knowledge_base, "detached")
                {
                    panic!("detached subtree failed conformance: {error}");
                }
            }
        }
    }
}

/// Path of `id` inside the subtree rooted at `top`, which is named `label`.
pub(super) fn path_below(ast: &Ast, top: RawNodeId, mut id: RawNodeId, label: &str) -> String {
    let mut slots = Vec::new();
    while id != top {
        let link = ast.parent(id).expect("`top` is an ancestor of `id`");
        slots.push(NodeSlot::from(link.slot));
        id = link.parent;
    }
    let mut path = label.to_owned();
    for slot in slots.into_iter().rev() {
        path.push('.');
        path.push_str(&slot.to_string());
    }
    path
}
