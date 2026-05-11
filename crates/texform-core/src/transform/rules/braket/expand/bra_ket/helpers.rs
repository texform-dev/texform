use texform_specs::builtin::base;

use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind, Node, NodeId,
};
use crate::transform::engine::TransformError;
use crate::transform::rule::RuleKey;
use crate::transform::rule_context::RuleContext;

pub(super) fn required_math_arg(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<NodeId, TransformError> {
    match slot {
        Some(arg)
            if matches!(arg.kind, ArgumentKind::Mandatory | ArgumentKind::Group)
                && matches!(arg.value, ArgumentValue::MathContent(_)) =>
        {
            match arg.value {
                ArgumentValue::MathContent(node_id) => Ok(node_id),
                _ => unreachable!("math content was checked above"),
            }
        }
        _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be math content"))),
    }
}

pub(super) fn replace_with_fixed_bra(cx: &mut RuleContext<'_>, node_id: NodeId, body: NodeId) {
    let mut after = Vec::new();
    cx.ast.append_cloned_math_content(&mut after, body);
    after.push(cx.ast.new_node(control("vert")));
    let replacement = cx.ast.new_node(control("langle"));
    cx.ast
        .replace_with_math_sequence(node_id, Vec::new(), replacement, after);
}

pub(super) fn replace_with_fixed_ket(cx: &mut RuleContext<'_>, node_id: NodeId, body: NodeId) {
    let mut after = Vec::new();
    cx.ast.append_cloned_math_content(&mut after, body);
    after.push(cx.ast.new_node(control("rangle")));
    let replacement = cx.ast.new_node(control("vert"));
    cx.ast
        .replace_with_math_sequence(node_id, Vec::new(), replacement, after);
}

pub(super) fn replace_with_braket(cx: &mut RuleContext<'_>, node_id: NodeId, body: NodeId) {
    if let Some((left, right)) = split_math_content_on_vert(cx, body) {
        replace_with_middle_braket(cx, node_id, left, right);
    } else {
        replace_with_angle_group(cx, node_id, body);
    }
}

fn replace_with_angle_group(cx: &mut RuleContext<'_>, node_id: NodeId, body: NodeId) {
    let mut children = Vec::new();
    cx.ast.append_cloned_math_content(&mut children, body);
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

fn replace_with_middle_braket(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    left: NodeId,
    right: NodeId,
) {
    let mut children = Vec::new();
    cx.ast.append_cloned_math_content(&mut children, left);
    children.push(cx.ast.new_node(middle_vert()));
    cx.ast.append_cloned_math_content(&mut children, right);
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

fn split_math_content_on_vert(cx: &mut RuleContext<'_>, body: NodeId) -> Option<(NodeId, NodeId)> {
    let Node::Group {
        children,
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    } = cx.ast.node(body)
    else {
        return None;
    };

    let children = children.clone();
    let split_at = children.iter().position(|&child| match cx.ast.node(child) {
        Node::Char('|') => true,
        Node::Command { name, .. } => name == "|" || name == "vert",
        _ => false,
    })?;

    let left_children: Vec<NodeId> = children[..split_at]
        .iter()
        .map(|&child| cx.ast.clone_subtree(child))
        .collect();
    let right_children: Vec<NodeId> = children[split_at + 1..]
        .iter()
        .map(|&child| cx.ast.clone_subtree(child))
        .collect();

    Some((
        cx.ast.new_node(Node::Group {
            children: left_children,
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        }),
        cx.ast.new_node(Node::Group {
            children: right_children,
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        }),
    ))
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
    Node::Command {
        name: name.to_string(),
        args: Vec::new(),
        known: true,
    }
}
