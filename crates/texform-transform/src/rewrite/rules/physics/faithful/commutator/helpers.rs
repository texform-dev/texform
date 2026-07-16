use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind, Node, NodeId,
};
use crate::rewrite::RuleError;
use crate::rewrite::helpers::FenceToken;
use crate::rewrite::rule::RuleKey;
use crate::rewrite::rule_context::RuleContext;

pub(super) struct BinaryFencePair {
    pub(super) auto_left: Delimiter,
    pub(super) auto_right: Delimiter,
    pub(super) fixed_left: FenceToken,
    pub(super) fixed_right: FenceToken,
}

pub(super) fn required_braced_math_arg(
    rule: RuleKey,
    cx: &mut RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<NodeId, RuleError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Group => match arg.value {
            ArgumentValue::MathContent(node_id) => Ok(node_id),
            _ => Err(cx.for_rule(rule).invalid_shape(format!("{subject} {label} should be math content"))),
        },
        _ => Err(cx.for_rule(rule).invalid_shape(format!("{subject} {label} should be a required braced math group"))),
    }
}

pub(super) fn required_math_arg(
    rule: RuleKey,
    cx: &mut RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<NodeId, RuleError> {
    cx.for_rule(rule)
        .mandatory_or_group_math_content(slot, subject, label)
}

pub(super) fn replace_with_binary_bracket_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    starred: bool,
    left: NodeId,
    right: NodeId,
    fences: BinaryFencePair,
) {
    if starred {
        replace_with_fixed_fence(
            cx,
            node_id,
            left,
            right,
            fences.fixed_left,
            fences.fixed_right,
        );
    } else {
        replace_with_auto_fence(cx, node_id, left, right, fences.auto_left, fences.auto_right);
    }
}

fn replace_with_auto_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    left: NodeId,
    right: NodeId,
    open: Delimiter,
    close: Delimiter,
) {
    let mut children = Vec::new();
    append_binary_bracket_body(cx, &mut children, left, right);

    cx.ast.replace_node_drop_detached_children(node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited {
                left: open,
                right: close,
            },
            mode: ContentMode::Math,
        },
    );
}

fn replace_with_fixed_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    left: NodeId,
    right: NodeId,
    open: FenceToken,
    close: FenceToken,
) {
    let mut rest = Vec::new();
    append_binary_bracket_body(cx, &mut rest, left, right);
    rest.push(cx.ast.new_node(close.node()));

    let open = cx.ast.new_node(open.node());
    cx.ast
        .replace_with_math_sequence_preserving_scripts(node_id, Vec::new(), open, rest);
}

fn append_binary_bracket_body(
    cx: &mut RuleContext<'_>,
    out: &mut Vec<NodeId>,
    left: NodeId,
    right: NodeId,
) {
    cx.ast.append_cloned_math_content(out, left);
    out.push(cx.ast.new_node(Node::Char(',')));
    cx.ast.append_cloned_math_content(out, right);
}
