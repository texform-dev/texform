use texform_knowledge::specs::BuiltinCommandRecord;

use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind, Node, NodeId,
};
use crate::rewrite::RuleError;
use crate::rewrite::helpers::{
    delimiter_slot, dimension_slot, integer_slot, mandatory_content_slot, prefix_command_node,
};
use crate::rewrite::rule::RuleKey;
use crate::rewrite::rule_context::RuleContext;

pub(super) fn delimiter_arg(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<Delimiter, RuleError> {
    cx.for_rule(rule).mandatory_delimiter(slot, subject, label)
}

pub(super) fn dimension_arg(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<String, RuleError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Mandatory => match &arg.value {
            ArgumentValue::Dimension(value) => Ok(value.clone()),
            _ => Err(cx.for_rule(rule).invalid_shape(format!("{subject} {label} should be a dimension"))),
        },
        _ => Err(cx.for_rule(rule).invalid_shape(format!("{subject} {label} should be a mandatory dimension argument"))),
    }
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
        mandatory_content_slot(numerator, ContentMode::Math),
        mandatory_content_slot(denominator, ContentMode::Math),
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
    replace_stacked_infix(cx, node_id, prefix_command_node(command, args));
}
