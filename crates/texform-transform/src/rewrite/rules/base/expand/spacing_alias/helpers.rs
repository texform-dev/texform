use crate::ast::{ArgumentKind, ArgumentSlot, ArgumentValue, Ast, Node, NodeId};
use crate::rewrite::RuleError;
use crate::rewrite::rule::RuleKey;
use crate::rewrite::rule_context::RuleContext;

pub(super) fn required_dimension(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
) -> Result<String, RuleError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Mandatory => match &arg.value {
            ArgumentValue::Dimension(value) => Ok(value.clone()),
            _ => Err(cx
                .for_rule(rule)
                .invalid_shape(format!("{subject} argument should carry a dimension"))),
        },
        _ => Err(cx
            .for_rule(rule)
            .invalid_shape(format!("{subject} should carry a mandatory dimension argument"))),
    }
}

/// Consume the separator MathJax absorbs after an unbraced text-mode dimension.
pub(super) fn consume_following_text_separator(ast: &mut Ast, node_id: NodeId) {
    let Some(next_id) = ast.next_sibling(node_id) else {
        return;
    };
    let Some(Node::Text(text)) = ast.node_opt_mut(next_id) else {
        return;
    };
    if text.starts_with(' ') {
        text.remove(0);
    }
}
