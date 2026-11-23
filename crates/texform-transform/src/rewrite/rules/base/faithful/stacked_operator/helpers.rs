use texform_knowledge::builtin::base;
use texform_knowledge::specs::BuiltinCommandRecord;

use crate::ast::{Ast, ContentMode, Node, NodeId};
use crate::rewrite::helpers::{mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule_context::RuleContext;

/// Returns `true` when `node_id` is an empty group (e.g. `\stackrel[]{a}{b}`).
///
/// Matches `stackrel.sty`'s `\ifx...\@empty` check so empty optional brackets
/// omit the subscript instead of emitting a degenerate `\limits_{}^{...}`.
pub(super) fn is_empty_math_group(ast: &Ast, node_id: NodeId) -> bool {
    matches!(
        ast.node(node_id),
        Node::Group { children, .. } if children.is_empty()
    )
}

/// Builds `\mathrel{\mathop{operator}\limits^{above}}` (or `\mathbin`),
/// attaching `below` as a subscript on `\limits` when `stackrel.sty`'s optional
/// subscript argument is present and non-empty.
pub(super) fn stacked_operator_command(
    cx: &mut RuleContext<'_>,
    class_record: &'static BuiltinCommandRecord,
    operator: NodeId,
    above: NodeId,
    below: Option<NodeId>,
) -> Node {
    let mathop = cx.ast.new_node(prefix_command_node(
        &base::cmd::MATHOP,
        vec![mandatory_content_slot(operator, ContentMode::Math)],
    ));
    let limits = cx
        .ast
        .new_node(prefix_command_node(&base::cmd::LIMITS, Vec::new()));
    let limits_with_scripts = cx.ast.new_node(Node::Scripted {
        base: limits,
        subscript: below,
        superscript: Some(above),
    });
    let body = cx.ast.implicit_math_group(vec![mathop, limits_with_scripts]);

    prefix_command_node(
        class_record,
        vec![mandatory_content_slot(body, ContentMode::Math)],
    )
}
