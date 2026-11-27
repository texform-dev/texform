use texform_knowledge::builtin::ams;

use crate::ast::{ContentMode, Delimiter, GroupKind, Node, NodeId};
use crate::rewrite::rule_context::RuleContext;

/// Replaces a fenced `smallmatrix`-family environment with `\left ... \right`
/// around an AMS `smallmatrix` environment.
pub(super) fn replace_with_fenced_smallmatrix_env(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    body: NodeId,
    left: Delimiter,
    right: Delimiter,
) {
    let matrix_body = cx.ast.clone_subtree(body);
    let matrix = cx.ast.new_node(Node::Environment {
        name: ams::env::SMALLMATRIX.name.to_string(),
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
