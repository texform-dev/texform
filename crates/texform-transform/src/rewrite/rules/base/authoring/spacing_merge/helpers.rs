use crate::ast::{ContentMode, Node, NodeId, Slot};
use crate::rewrite::rule_context::RuleContext;

/// Return whether a node belongs to a math-mode sibling container.
pub(super) fn is_math_sibling(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    let Some(parent) = cx.ast.parent(node_id) else {
        return false;
    };
    if !matches!(parent.slot, Slot::GroupChild(_)) {
        return false;
    }

    matches!(
        cx.ast.node(parent.parent),
        Node::Root {
            mode: ContentMode::Math,
            ..
        } | Node::Group {
            mode: ContentMode::Math,
            ..
        }
    )
}
