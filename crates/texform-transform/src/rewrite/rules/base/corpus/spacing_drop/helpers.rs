use crate::ast::{Ast, NodeId};
use crate::rewrite::helpers::remove_node_preserving_slot;

pub(super) fn drop_layout_hint(ast: &mut Ast, node_id: NodeId) {
    remove_node_preserving_slot(ast, node_id);
}
