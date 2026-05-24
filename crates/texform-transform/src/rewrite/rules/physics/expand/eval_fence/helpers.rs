use texform_knowledge::builtin::base;

use crate::ast::{ContentMode, Delimiter, GroupKind, Node, NodeId};
use crate::rewrite::helpers::{bare_command_node, mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule_context::RuleContext;

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
        cx.ast.append_cloned_math_content(&mut children, body);
    }
    children.push(vphantom_int(cx));

    cx.ast.replace_node_drop_detached_children(node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited { left, right },
            mode: ContentMode::Math,
        },
    );
}

fn smash_body(cx: &mut RuleContext<'_>, body: NodeId) -> NodeId {
    let body = cx.ast.clone_subtree(body);
    cx.ast.new_node(prefix_command_node(
        &base::cmd::SMASH,
        vec![None, mandatory_content_slot(body, ContentMode::Math)],
    ))
}

fn vphantom_int(cx: &mut RuleContext<'_>) -> NodeId {
    let int = cx.ast.new_node(bare_command_node("int"));
    cx.ast.new_node(prefix_command_node(
        &base::cmd::VPHANTOM,
        vec![mandatory_content_slot(int, ContentMode::Math)],
    ))
}
