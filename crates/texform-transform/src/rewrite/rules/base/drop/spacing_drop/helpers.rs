use crate::ast::{Ast, ContentMode, GroupKind, Node, NodeId, Slot};

pub(super) fn drop_layout_hint(ast: &mut Ast, node_id: NodeId) {
    if matches!(ast.slot(node_id), Some(Slot::GroupChild(_))) {
        ast.remove_node(node_id);
        return;
    }

    ast.replace_node_drop_detached_children(
        node_id,
        Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        },
    );
}
