use texform_specs::builtin::base;

use crate::ast::{ContentMode, Delimiter, GroupKind, Node, NodeId};
use crate::transform::helpers::{
    append_cloned_math_content, mandatory_content, prefix_command,
    replace_node_discarding_detached_children,
};
use crate::transform::rule_context::RuleContext;

pub(super) fn replace_with_eval_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    starred: bool,
    body: NodeId,
    left: Delimiter,
    right: Delimiter,
) {
    let mut children = Vec::new();
    if starred {
        children.push(smash_body(cx, body));
    } else {
        append_cloned_math_content(cx, &mut children, body);
    }
    children.push(vphantom_int(cx));

    replace_node_discarding_detached_children(
        cx,
        node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited { left, right },
            mode: ContentMode::Math,
        },
    );
}

fn smash_body(cx: &mut RuleContext<'_>, body: NodeId) -> NodeId {
    let body = cx.ast.clone_subtree(body);
    cx.ast.new_node(prefix_command(
        &base::cmd::SMASH,
        vec![None, mandatory_content(body, ContentMode::Math)],
    ))
}

fn vphantom_int(cx: &mut RuleContext<'_>) -> NodeId {
    let int = cx.ast.new_node(Node::Command {
        name: "int".to_string(),
        args: Vec::new(),
        known: true,
    });
    cx.ast.new_node(prefix_command(
        &base::cmd::VPHANTOM,
        vec![mandatory_content(int, ContentMode::Math)],
    ))
}
