use crate::ast::{ContentMode, Delimiter, GroupKind, Node, NodeId};
use crate::rewrite::helpers::FenceToken;
use crate::rewrite::rule_context::RuleContext;

pub(super) struct FencePair {
    pub(super) auto_left: Delimiter,
    pub(super) auto_right: Delimiter,
    pub(super) fixed_left: FenceToken,
    pub(super) fixed_right: FenceToken,
}

pub(super) fn replace_with_delimiter_shorthand(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    starred: bool,
    body: NodeId,
    fences: FencePair,
) {
    if starred {
        replace_with_fixed_fence(cx, node_id, body, fences.fixed_left, fences.fixed_right);
    } else {
        replace_with_auto_fence(cx, node_id, body, fences.auto_left, fences.auto_right);
    }
}

fn replace_with_auto_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    body: NodeId,
    left: Delimiter,
    right: Delimiter,
) {
    let mut children = Vec::new();
    cx.ast.append_cloned_math_content(&mut children, body);

    cx.ast.replace_node_drop_detached_children(node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited { left, right },
            mode: ContentMode::Math,
        },
    );
}

fn replace_with_fixed_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    body: NodeId,
    left: FenceToken,
    right: FenceToken,
) {
    let mut rest = Vec::new();
    cx.ast.append_cloned_math_content(&mut rest, body);
    rest.push(cx.ast.new_node(right.node()));

    let left = cx.ast.new_node(left.node());
    cx.ast
        .replace_with_math_sequence_preserving_scripts(node_id, Vec::new(), left, rest);
}
