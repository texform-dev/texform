use crate::ast::{Ast, Node, NodeId};

/// Consume one normalized ASCII separator after a text-mode source command.
pub(super) fn consume_following_text_separator(ast: &mut Ast, node_id: NodeId) {
    let Some(next_id) = ast.next_sibling(node_id) else {
        return;
    };
    let Some(Node::Text(text)) = ast.node_opt_mut(next_id) else {
        return;
    };
    if text.starts_with(' ') {
        text.remove(0);
    }
}
