use texform_specs::builtin::base;

use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind, Node, NodeId,
};
use crate::rewrite::RuleError;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::RuleKey;
use crate::rewrite::rule_context::RuleContext;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BraketSize {
    Auto,
    Fixed,
    Middle,
}

pub(super) fn optional_group_arg(
    rule: RuleKey,
    cx: &mut RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<Option<NodeId>, RuleError> {
    cx.for_rule(rule)
        .optional_group_math_content(slot, subject, label)
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

pub(super) fn replace_with_bra(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    size: BraketSize,
    body: NodeId,
) {
    match size {
        BraketSize::Auto | BraketSize::Middle => replace_with_delimited_group(
            cx,
            node_id,
            Delimiter::Control("langle".to_string()),
            vec![body],
            Delimiter::Control("vert".to_string()),
        ),
        BraketSize::Fixed => {
            let parts = fixed_parts(cx, body, control("vert"));
            replace_with_fixed_sequence(cx, node_id, vec![control("langle")], parts);
        }
    }
}

pub(super) fn replace_with_ket(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    size: BraketSize,
    body: NodeId,
) {
    match size {
        BraketSize::Auto | BraketSize::Middle => replace_with_delimited_group(
            cx,
            node_id,
            Delimiter::Control("vert".to_string()),
            vec![body],
            Delimiter::Control("rangle".to_string()),
        ),
        BraketSize::Fixed => {
            let parts = fixed_parts(cx, body, control("rangle"));
            replace_with_fixed_sequence(cx, node_id, vec![control("vert")], parts);
        }
    }
}

pub(super) fn replace_with_braket(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    size: BraketSize,
    left: NodeId,
    right: NodeId,
) {
    replace_with_angle_bar_parts(cx, node_id, size, &[left, right]);
}

pub(super) fn replace_with_expectation_body(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    size: BraketSize,
    body: NodeId,
) {
    match size {
        BraketSize::Auto | BraketSize::Middle => replace_with_delimited_group(
            cx,
            node_id,
            Delimiter::Control("langle".to_string()),
            vec![body],
            Delimiter::Control("rangle".to_string()),
        ),
        BraketSize::Fixed => {
            let parts = fixed_parts(cx, body, control("rangle"));
            replace_with_fixed_sequence(cx, node_id, vec![control("langle")], parts);
        }
    }
}

pub(super) fn replace_with_expectation_state(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    size: BraketSize,
    operator: NodeId,
    state: NodeId,
) {
    match size {
        BraketSize::Auto => replace_with_split_auto_angle_bar_parts(
            cx,
            node_id,
            &[state, operator, state],
        ),
        BraketSize::Fixed | BraketSize::Middle => {
            replace_with_angle_bar_parts(cx, node_id, size, &[state, operator, state]);
        }
    }
}

pub(super) fn replace_with_matrix_element(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    size: BraketSize,
    left_state: NodeId,
    operator: NodeId,
    right_state: NodeId,
) {
    match size {
        BraketSize::Auto => replace_with_split_auto_angle_bar_parts(
            cx,
            node_id,
            &[left_state, operator, right_state],
        ),
        BraketSize::Fixed | BraketSize::Middle => replace_with_angle_bar_parts(
            cx,
            node_id,
            size,
            &[left_state, operator, right_state],
        ),
    }
}

fn replace_with_angle_bar_parts(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    size: BraketSize,
    parts: &[NodeId],
) {
    match size {
        BraketSize::Auto | BraketSize::Middle => replace_with_middle_group(cx, node_id, parts),
        BraketSize::Fixed => {
            let mut after = Vec::new();
            for (index, part) in parts.iter().enumerate() {
                if index > 0 {
                    after.push(cx.ast.new_node(control("vert")));
                }
                cx.ast.append_cloned_math_content(&mut after, *part);
            }
            after.push(cx.ast.new_node(control("rangle")));
            replace_with_fixed_sequence(cx, node_id, vec![control("langle")], after);
        }
    }
}

fn replace_with_middle_group(cx: &mut RuleContext<'_>, node_id: NodeId, parts: &[NodeId]) {
    let mut children = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            children.push(cx.ast.new_node(middle_vert()));
        }
        cx.ast.append_cloned_math_content(&mut children, *part);
    }
    cx.ast.replace_node_drop_detached_children(node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited {
                left: Delimiter::Control("langle".to_string()),
                right: Delimiter::Control("rangle".to_string()),
            },
            mode: ContentMode::Math,
        },
    );
}

fn replace_with_split_auto_angle_bar_parts(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    parts: &[NodeId],
) {
    let first = delimited_node(
        cx,
        Delimiter::Control("langle".to_string()),
        parts[0],
        Delimiter::Control("vert".to_string()),
    );
    let mut after = Vec::new();
    cx.ast.append_cloned_math_content(&mut after, parts[1]);
    let last = delimited_node(
        cx,
        Delimiter::Control("vert".to_string()),
        parts[2],
        Delimiter::Control("rangle".to_string()),
    );
    after.push(cx.ast.new_node(last));

    let first = cx.ast.new_node(first);
    cx.ast
        .replace_with_math_sequence(node_id, Vec::new(), first, after);
}

fn replace_with_delimited_group(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    left: Delimiter,
    parts: Vec<NodeId>,
    right: Delimiter,
) {
    let mut children = Vec::new();
    for part in parts {
        cx.ast.append_cloned_math_content(&mut children, part);
    }
    cx.ast.replace_node_drop_detached_children(node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited { left, right },
            mode: ContentMode::Math,
        },
    );
}

fn delimited_node(cx: &mut RuleContext<'_>, left: Delimiter, body: NodeId, right: Delimiter) -> Node {
    let mut children = Vec::new();
    cx.ast.append_cloned_math_content(&mut children, body);
    Node::Group {
        children,
        kind: GroupKind::Delimited { left, right },
        mode: ContentMode::Math,
    }
}

fn replace_with_fixed_sequence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    before: Vec<Node>,
    parts: Vec<NodeId>,
) {
    let mut before_nodes = before
        .into_iter()
        .map(|node| cx.ast.new_node(node))
        .collect::<Vec<_>>();
    let Some(first) = before_nodes.pop() else {
        return;
    };

    let mut after = Vec::new();
    for part in parts {
        after.push(part);
    }

    cx.ast
        .replace_with_math_sequence_preserving_scripts(node_id, before_nodes, first, after);
}

fn fixed_parts(cx: &mut RuleContext<'_>, body: NodeId, close: Node) -> Vec<NodeId> {
    let mut parts = Vec::new();
    cx.ast.append_cloned_math_content(&mut parts, body);
    parts.push(cx.ast.new_node(close));
    parts
}

fn middle_vert() -> Node {
    Node::Command {
        name: base::cmd::MIDDLE.name.to_string(),
        args: vec![Some(crate::ast::Argument {
            kind: ArgumentKind::Mandatory,
            value: ArgumentValue::Delimiter(Delimiter::Control("vert".to_string())),
        })],
        known: true,
    }
}

fn control(name: &'static str) -> Node {
    bare_command_node(name)
}
