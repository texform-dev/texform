use texform_knowledge::builtin::mathtools;

use crate::ast::{ArgumentSlot, ContentMode, Delimiter, GroupKind, Node, NodeId};
use crate::rewrite::rule_context::RuleContext;

/// Replaces a fenced `matrix*`-family environment with `\left ... \right`
/// around a `matrix*` environment that keeps the source alignment slot.
pub(super) fn replace_with_fenced_matrix_star_env(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    args: Vec<ArgumentSlot>,
    body: NodeId,
    left: Delimiter,
    right: Delimiter,
) {
    let matrix_body = cx.ast.clone_subtree(body);
    let matrix = cx.ast.new_node(Node::Environment {
        name: mathtools::env::MATRIX_STAR.name.to_string(),
        args,
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
