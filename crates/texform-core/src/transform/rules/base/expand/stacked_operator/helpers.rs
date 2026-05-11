use texform_specs::builtin::base;
use texform_specs::specs::BuiltinCommandRecord;

use crate::ast::{ContentMode, Node, NodeId};
use crate::transform::helpers::{mandatory_content, prefix_command_node};
use crate::transform::rule_context::RuleContext;

pub(super) fn stacked_operator_command(
    cx: &mut RuleContext<'_>,
    class_record: &'static BuiltinCommandRecord,
    operator: NodeId,
    above: NodeId,
) -> Node {
    let mathop = cx.ast.new_node(prefix_command_node(
        &base::cmd::MATHOP,
        vec![mandatory_content(operator, ContentMode::Math)],
    ));
    let limits = cx
        .ast
        .new_node(prefix_command_node(&base::cmd::LIMITS, Vec::new()));
    let limits_with_above = cx.ast.superscript(limits, above);
    let body = cx.ast.implicit_math_group(vec![mathop, limits_with_above]);

    prefix_command_node(
        class_record,
        vec![mandatory_content(body, ContentMode::Math)],
    )
}
