use texform_specs::builtin::ams;

use crate::ast::{ContentMode, Delimiter, GroupKind, Node, NodeId};
use crate::transform::rule_context::RuleContext;

pub(super) fn replace_with_fenced_matrix_env(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    body: NodeId,
    left: Delimiter,
    right: Delimiter,
) {
    let matrix_body = cx.ast.clone_subtree(body);
    let matrix = cx.ast.new_node(Node::Environment {
        name: ams::env::MATRIX.name.to_string(),
        args: Vec::new(),
        known: true,
        body: matrix_body,
    });

    cx.ast.replace_node_drop_detached_children(
        node_id,
        Node::Group {
            children: vec![matrix],
            kind: GroupKind::Delimited { left, right },
            mode: ContentMode::Math,
        },
    );
}
