use texform_specs::builtin::base;
use texform_specs::builtin::boldsymbol;
use texform_specs::specs::BuiltinCommandRecord;

use crate::ast::{ContentMode, Node, NodeId};
use crate::transform::engine::TransformError;
use crate::transform::helpers::{
    mandatory_content, prefix_command, replace_node_discarding_detached_children,
    required_math_content, star_arg_value,
};
use crate::transform::rule::RuleKey;
use crate::transform::rule_context::RuleContext;

pub(super) fn vector_args(
    rule_key: RuleKey,
    cx: &RuleContext<'_>,
    args: &[crate::ast::ArgumentSlot],
    subject: &str,
) -> Result<(bool, NodeId), TransformError> {
    cx.expect_arg_len(rule_key, args, 2, subject)?;
    let starred = star_arg_value(rule_key, cx, &args[0], subject)?;
    let body = required_math_content(rule_key, cx, &args[1], subject, "argument")?;
    Ok((starred, body))
}

pub(super) fn replace_with_vector_style(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    starred: bool,
    body: NodeId,
) {
    let replacement = vector_style_command(cx, starred, body);
    replace_node_discarding_detached_children(cx, node_id, replacement);
}

pub(super) fn replace_with_wrapped_vector_style(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    wrapper: &'static BuiltinCommandRecord,
    starred: bool,
    body: NodeId,
) {
    let styled_command = vector_style_command(cx, starred, body);
    let styled = cx.ast.new_node(styled_command);
    let replacement = prefix_command(
        wrapper,
        vec![mandatory_content(styled, ContentMode::Math)],
    );
    replace_node_discarding_detached_children(cx, node_id, replacement);
}

fn vector_style_command(cx: &mut RuleContext<'_>, starred: bool, body: NodeId) -> Node {
    let record = if starred {
        &boldsymbol::cmd::BOLDSYMBOL
    } else {
        &base::cmd::MATHBF
    };
    let body = cx.ast.clone_subtree(body);
    prefix_command(record, vec![mandatory_content(body, ContentMode::Math)])
}
