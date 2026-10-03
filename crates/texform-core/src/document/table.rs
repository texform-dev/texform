//! Column-oriented snapshots of the attached document tree.

use serde::Serialize;
use texform_argspec::ArgSpec;

use super::{
    ArgKindRef, ArgumentValue, ContentMode, DelimiterRef, Document, GroupKind, Node, NodeKind,
    NodeSlot, RawNodeId, Slot, conformance, construct, known_flag,
};

/// A columnar tree representation for bulk structural analysis with Arrow or DataFrames.
///
/// Nodes are in depth-first preorder. Arguments are ordered by their owner's
/// node row and then slot index, including absent and scalar-valued slots.
/// Detached subtrees are excluded. Every column in a table has equal length.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ColumnarTree {
    pub nodes: NodeTable,
    pub args: ArgumentTable,
}

/// One row per attached node, including the document root.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct NodeTable {
    /// Node kind, using the names of [`NodeKind`] variants.
    pub kind: Vec<&'static str>,
    /// Parent row; `-1` for the root.
    pub parent: Vec<i64>,
    /// Attachment slot in snake_case; `None` for the root.
    pub slot: Vec<Option<&'static str>>,
    /// Child or argument index; `-1` for other slots and the root.
    pub slot_index: Vec<i64>,
    /// Distance from the root.
    pub depth: Vec<usize>,
    /// Context mode (`math` or `text`), not a group's child-content mode.
    pub mode: Vec<&'static str>,
    /// Command, infix, declarative, or environment name.
    pub name: Vec<Option<String>>,
    /// Character, text, decimal prime count, or error snippet.
    pub value: Vec<Option<String>>,
    /// Knowledge flag for command-like nodes and environments.
    pub known: Vec<Option<bool>>,
    /// `explicit`, `implicit`, `delimited`, or `inline_math` for groups.
    pub group_kind: Vec<Option<&'static str>>,
    /// Delimited-group boundary: character, backslash-prefixed control, or `.`.
    pub left: Vec<Option<String>>,
    /// Delimited-group boundary, in the same format as `left`.
    pub right: Vec<Option<String>>,
}

/// One row per argument slot, including absent optional slots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ArgumentTable {
    /// Owning node's row in [`NodeTable`].
    pub owner: Vec<usize>,
    /// Argument slot index.
    pub index: Vec<usize>,
    /// Actual form, or the knowledge-base form for an absent slot.
    /// Unknown forms in incomplete documents are `None`.
    pub form: Vec<Option<&'static str>>,
    pub present: Vec<bool>,
    /// Payload kind in snake_case; absent slots are `None`.
    pub value_kind: Vec<Option<&'static str>>,
    /// Scalar payload; content and absent slots are `None`.
    pub value: Vec<Option<String>>,
    /// Content root's node row; `-1` for scalar and absent slots.
    pub content: Vec<i64>,
    /// Actual Delimited or Paired opening boundary; absent slots are `None`.
    pub open: Vec<Option<String>>,
    /// Actual Delimited or Paired closing boundary; absent slots are `None`.
    pub close: Vec<Option<String>>,
}

struct PendingNode {
    id: RawNodeId,
    parent: i64,
    slot: Option<Slot>,
    depth: usize,
    mode: ContentMode,
    argument_row: Option<usize>,
}

impl Document {
    /// Export a columnar tree representation for bulk structural analysis with Arrow or DataFrames.
    ///
    /// This linear-time, read-only operation also supports documents containing error nodes.
    /// Table indices identify rows in this snapshot, not persistent node IDs.
    pub fn to_columnar(&self) -> ColumnarTree {
        let mut tables = ColumnarTree::default();
        let mut stack = vec![PendingNode {
            id: self.ast.root(),
            parent: -1,
            slot: None,
            depth: 0,
            mode: self.root_mode(),
            argument_row: None,
        }];
        while let Some(pending) = stack.pop() {
            let node = self.ast.node(pending.id);
            let row = tables.nodes.kind.len();
            if let Some(argument_row) = pending.argument_row {
                tables.args.content[argument_row] = row as i64;
            }
            tables.nodes.push(node, &pending);
            let arguments = self.ast.arg_slots(pending.id);
            let first_argument = tables.args.owner.len();
            // Only absent slots need a signature lookup; actual arguments carry
            // both their form and their selected delimiter pair.
            let signature = arguments
                .iter()
                .any(Option::is_none)
                .then(|| conformance::signature(node, pending.mode, &self.knowledge_base))
                .flatten();
            for (index, argument) in arguments.iter().enumerate() {
                tables.args.push(
                    row,
                    index,
                    argument.as_ref(),
                    signature.and_then(|signature| signature.args.get(index)),
                );
            }
            // Ast::edges owns traversal order, including infix left/args/right.
            // Passing parent rows down avoids paths and repeated ancestor walks.
            for (id, slot) in self.ast.edges(pending.id).into_iter().rev() {
                stack.push(PendingNode {
                    id,
                    parent: row as i64,
                    slot: Some(slot),
                    depth: pending.depth + 1,
                    mode: conformance::child_mode(node, slot, pending.mode, &self.knowledge_base),
                    argument_row: match slot {
                        Slot::Argument(index) => Some(first_argument + index),
                        _ => None,
                    },
                });
            }
        }
        tables
    }
}

impl NodeTable {
    fn push(&mut self, node: &Node, pending: &PendingNode) {
        self.kind.push(match node.kind() {
            NodeKind::Root => "Root",
            NodeKind::Group => "Group",
            NodeKind::Command => "Command",
            NodeKind::Infix => "Infix",
            NodeKind::Declarative => "Declarative",
            NodeKind::Environment => "Environment",
            NodeKind::Scripted => "Scripted",
            NodeKind::Prime => "Prime",
            NodeKind::Text => "Text",
            NodeKind::Char => "Char",
            NodeKind::ActiveSpace => "ActiveSpace",
            NodeKind::AlignmentTab => "AlignmentTab",
            NodeKind::Error => "Error",
        });
        self.parent.push(pending.parent);
        let slot = pending.slot.map(NodeSlot::from);
        self.slot.push(slot.map(NodeSlot::as_str));
        self.slot_index.push(
            slot.and_then(NodeSlot::index)
                .map_or(-1, |index| index as i64),
        );
        self.depth.push(pending.depth);
        self.mode.push(pending.mode.as_str());
        self.name.push(match node {
            Node::Command { name, .. }
            | Node::Infix { name, .. }
            | Node::Declarative { name, .. }
            | Node::Environment { name, .. } => Some(name.clone()),
            _ => None,
        });
        self.value.push(match node {
            Node::Char(value) => Some(value.to_string()),
            Node::Text(value) | Node::Error { snippet: value, .. } => Some(value.clone()),
            Node::Prime { count } => Some(count.to_string()),
            _ => None,
        });
        self.known.push(known_flag(node));
        let (kind, left, right) = match node {
            Node::Group { kind, .. } => match kind {
                GroupKind::Explicit => (Some("explicit"), None, None),
                GroupKind::Implicit => (Some("implicit"), None, None),
                GroupKind::InlineMath => (Some("inline_math"), None, None),
                GroupKind::Delimited { left, right } => (
                    Some("delimited"),
                    Some(DelimiterRef::from(left).to_string()),
                    Some(DelimiterRef::from(right).to_string()),
                ),
            },
            _ => (None, None, None),
        };
        self.group_kind.push(kind);
        self.left.push(left);
        self.right.push(right);
    }
}

impl ArgumentTable {
    fn push(
        &mut self,
        owner: usize,
        index: usize,
        argument: Option<&super::Argument>,
        spec: Option<&ArgSpec>,
    ) {
        self.owner.push(owner);
        self.index.push(index);
        self.present.push(argument.is_some());
        self.content.push(-1);
        let Some(argument) = argument else {
            self.form
                .push(spec.map(|spec| ArgKindRef::from(&construct::spec_kind(spec)).as_str()));
            self.value_kind.push(None);
            self.value.push(None);
            self.open.push(None);
            self.close.push(None);
            return;
        };
        let form = ArgKindRef::from(&argument.kind);
        let (open, close) = match form {
            ArgKindRef::Until { close } => (None, Some(close.to_string())),
            ArgKindRef::Delimited { open, close } | ArgKindRef::Paired { open, close } => {
                (Some(open.to_string()), Some(close.to_string()))
            }
            _ => (None, None),
        };
        self.form.push(Some(form.as_str()));
        self.open.push(open);
        self.close.push(close);
        let (kind, value) = match &argument.value {
            ArgumentValue::MathContent(_) => ("math", None),
            ArgumentValue::TextContent(_) => ("text", None),
            ArgumentValue::OperatorNameContent(_) => ("operator_name", None),
            ArgumentValue::Delimiter(value) => {
                ("delimiter", Some(DelimiterRef::from(value).to_string()))
            }
            ArgumentValue::CSName(value) => ("cs_name", Some(value.clone())),
            ArgumentValue::Dimension(value) => ("dimension", Some(value.clone())),
            ArgumentValue::Integer(value) => ("integer", Some(value.clone())),
            ArgumentValue::KeyVal(value) => ("key_val", Some(value.clone())),
            ArgumentValue::Column(value) => ("column", Some(value.clone())),
            ArgumentValue::Boolean(value) => ("boolean", Some(value.to_string())),
        };
        self.value_kind.push(Some(kind));
        self.value.push(value);
    }
}
