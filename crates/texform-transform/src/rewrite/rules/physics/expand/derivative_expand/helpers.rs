use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use crate::ast::{ContentMode, Node, NodeId};
use crate::rewrite::helpers::{bare_command_node, mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule_context::RuleContext;

pub(super) type DifferentialSymbol = fn(&mut RuleContext<'_>) -> NodeId;

pub(super) fn derivative_numerator(
    cx: &mut RuleContext<'_>,
    symbol: DifferentialSymbol,
    order: Option<NodeId>,
    expression: Option<NodeId>,
) -> NodeId {
    let differential = ordered_differential_symbol(cx, symbol, order);

    let mut children = vec![differential];
    if let Some(expression) = expression {
        cx.ast.append_cloned_math_content(&mut children, expression);
    }
    cx.ast.implicit_math_group(children)
}

pub(super) fn derivative_denominator(
    cx: &mut RuleContext<'_>,
    symbol: DifferentialSymbol,
    variable: NodeId,
    order: Option<NodeId>,
) -> NodeId {
    let variable = cx.ast.clone_subtree(variable);
    let variable = match order {
        Some(order) => {
            let order = cx.ast.clone_subtree(order);
            cx.ast.superscript(variable, order)
        }
        None => variable,
    };

    let differential = symbol(cx);
    let mut children = vec![differential];
    cx.ast.append_cloned_math_content(&mut children, variable);
    cx.ast.implicit_math_group(children)
}

pub(super) fn mixed_derivative_denominator(
    cx: &mut RuleContext<'_>,
    symbol: DifferentialSymbol,
    first_variable: NodeId,
    second_variable: NodeId,
) -> NodeId {
    let first_partial = symbol(cx);
    let mut children = vec![first_partial];
    cx.ast.append_cloned_math_content(&mut children, first_variable);
    let second_partial = symbol(cx);
    children.push(second_partial);
    cx.ast.append_cloned_math_content(&mut children, second_variable);
    cx.ast.implicit_math_group(children)
}

pub(super) fn derivative_fraction(starred: bool, numerator: NodeId, denominator: NodeId) -> Node {
    let record = if starred {
        &physics::cmd::FLATFRAC
    } else {
        &base::cmd::FRAC
    };
    prefix_command_node(
        record,
        vec![
            mandatory_content_slot(numerator, ContentMode::Math),
            mandatory_content_slot(denominator, ContentMode::Math),
        ],
    )
}

pub(super) fn differential_d(cx: &mut RuleContext<'_>) -> NodeId {
    let d = cx.ast.new_node(Node::Char('d'));
    cx.ast.new_node(prefix_command_node(
        &base::cmd::MATHRM,
        vec![mandatory_content_slot(d, ContentMode::Math)],
    ))
}

pub(super) fn delta_symbol(cx: &mut RuleContext<'_>) -> NodeId {
    named_symbol(cx, "delta")
}

pub(super) fn partial_symbol(cx: &mut RuleContext<'_>) -> NodeId {
    named_symbol(cx, "partial")
}

pub(super) fn order_two(cx: &mut RuleContext<'_>) -> NodeId {
    cx.ast.new_node(Node::Char('2'))
}

fn ordered_differential_symbol(
    cx: &mut RuleContext<'_>,
    symbol: DifferentialSymbol,
    order: Option<NodeId>,
) -> NodeId {
    match order {
        Some(order) => {
            let symbol = symbol(cx);
            let order = cx.ast.clone_subtree(order);
            cx.ast.superscript(symbol, order)
        }
        None => symbol(cx),
    }
}

fn named_symbol(cx: &mut RuleContext<'_>, name: &str) -> NodeId {
    cx.ast.new_node(bare_command_node(name))
}
