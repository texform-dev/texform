use texform_specs::specs::BuiltinCommandRecord;

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind, Node,
    NodeId,
};
use crate::transform::engine::TransformError;
use crate::transform::helpers::{mandatory_content, prefix_command};
use crate::transform::rule::RuleKey;
use crate::transform::rule_context::RuleContext;

pub(super) fn delimiter_arg(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<Delimiter, TransformError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Mandatory => match &arg.value {
            ArgumentValue::Delimiter(delimiter) => Ok(delimiter.clone()),
            _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be a delimiter"))),
        },
        _ => Err(cx.invalid_shape(
            rule,
            format!("{subject} {label} should be a mandatory delimiter argument"),
        )),
    }
}

pub(super) fn dimension_arg(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<String, TransformError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Mandatory => match &arg.value {
            ArgumentValue::Dimension(value) => Ok(value.clone()),
            _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be a dimension"))),
        },
        _ => Err(cx.invalid_shape(
            rule,
            format!("{subject} {label} should be a mandatory dimension argument"),
        )),
    }
}

pub(super) fn delimiter_slot(delimiter: Delimiter) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Delimiter(delimiter),
    })
}

pub(super) fn dimension_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Dimension(value.into()),
    })
}

pub(super) fn integer_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Integer(value.into()),
    })
}

pub(super) fn infix_prefix_args(
    left: NodeId,
    right: NodeId,
    mode: ContentMode,
) -> Vec<ArgumentSlot> {
    vec![mandatory_content(left, mode), mandatory_content(right, mode)]
}

pub(super) fn genfrac_args(
    left_delimiter: Delimiter,
    right_delimiter: Delimiter,
    thickness: impl Into<String>,
    style: impl Into<String>,
    numerator: NodeId,
    denominator: NodeId,
) -> Vec<ArgumentSlot> {
    vec![
        delimiter_slot(left_delimiter),
        delimiter_slot(right_delimiter),
        dimension_slot(thickness),
        integer_slot(style),
        mandatory_content(numerator, ContentMode::Math),
        mandatory_content(denominator, ContentMode::Math),
    ]
}

pub(super) fn replace_stacked_infix(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    replacement: Node,
) {
    let unwrap_parent_id = cx.ast.parent_id(node_id).and_then(|parent_id| {
        match cx.ast.node(parent_id) {
            Node::Group {
                children,
                kind: GroupKind::Explicit,
                ..
            } if children.as_slice() == [node_id] => Some(parent_id),
            _ => None,
        }
    });

    if let Some(parent_id) = unwrap_parent_id {
        cx.ast.replace_node(node_id, Node::Text(String::new()));
        cx.ast.replace_node(parent_id, replacement);
        cx.ast.remove_detached(node_id);
    } else {
        cx.ast.replace_node(node_id, replacement);
    }
}

pub(super) fn replace_infix_with_command(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    command: &'static BuiltinCommandRecord,
    args: Vec<ArgumentSlot>,
) {
    replace_stacked_infix(cx, node_id, prefix_command(command, args));
}

pub(super) fn subtree_contains_command(
    cx: &RuleContext<'_>,
    node_id: NodeId,
    command_name: &str,
) -> bool {
    cx.ast.find(node_id, |node| {
        matches!(node, Node::Command { name, .. } if name == command_name)
    }).is_some()
}
