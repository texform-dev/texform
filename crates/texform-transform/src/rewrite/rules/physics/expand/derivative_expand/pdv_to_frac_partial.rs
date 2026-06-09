//! Expand pdv forms to explicit partial-derivative fractions.
//!
//! ```yaml
//! proposal: pdv-to-frac-partial
//! triggers:
//!   - cmd:pdv
//! consumes:
//!   eliminates: cmd:pdv
//!   touches: null
//! produces:
//!   - cmd:frac
//!   - cmd:flatfrac
//! rewrite_patterns:
//!   - {label: pdv-two-argument, from: '\pdv{#1}{#2}', to: '\frac{\partial #1}{\partial #2}'}
//!   - {label: pdv-operator, from: '\pdv{#1}', to: '\frac{\partial}{\partial #1}'}
//!   - {label: pdv-order, from: '\pdv[#1]{#2}{#3}', to: '\frac{\partial^{#1} #2}{\partial #3^{#1}}'}
//!   - {label: pdv-mixed-partial, from: '\pdv{#1}{#2}{#3}', to: '\frac{\partial^{2} #1}{\partial #2 \partial #3}'}
//!   - {label: pdv-star, from: '\pdv*{#1}{#2}', to: '\flatfrac{\partial #1}{\partial #2}'}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use super::helpers::{
    derivative_denominator, derivative_fraction, derivative_numerator, mixed_derivative_denominator,
    order_two, partial_symbol,
};
use crate::ast::NodeId;
use crate::rewrite::RuleError;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static PDV_TO_FRAC_PARTIAL: PdvToFracPartialRule {
        key: Physics / "pdv-to-frac-partial",
        level: Expand,
        summary: "Expand pdv forms to explicit partial-derivative fractions.",
        fidelity: Full,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::PDV],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::PDV],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC, &physics::cmd::FLATFRAC],
        },
        apply(rule, cx, node_id) {
            expand_pdv(rule, cx, node_id)
        }
    }
}

fn expand_pdv(
    __rule: &PdvToFracPartialRule,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
) -> Result<RuleEffect, RuleError> {
    let Some(command) = cx.match_command(node_id, &physics::cmd::PDV) else {
        return Ok(RuleEffect::Skipped);
    };
    let subject = command.subject();
    let args = command.args.to_vec();

    cx.for_rule(PdvToFracPartialRule::KEY).expect_arg_len(&args, 5, &subject)?;
    let rule_key = PdvToFracPartialRule::KEY;
    let starred = cx.for_rule(rule_key).star_arg_value(&args[0], &subject)?;
    let order = cx.for_rule(rule_key).optional_math_content(&args[1], &subject, "optional order")?;
    let first = cx.for_rule(rule_key).mandatory_math_content(&args[2], &subject, "first argument")?;
    let denominator =
        cx.for_rule(rule_key).optional_group_math_content(&args[3], &subject, "denominator")?;
    let mixed_denominator =
        cx.for_rule(rule_key).optional_group_math_content(&args[4], &subject, "mixed denominator")?;

    let numerator_order = if mixed_denominator.is_some() {
        Some(order_two(cx))
    } else {
        order
    };
    let numerator = if denominator.is_some() {
        derivative_numerator(cx, partial_symbol, numerator_order, Some(first))
    } else {
        derivative_numerator(cx, partial_symbol, order, None)
    };
    let denominator = match (denominator, mixed_denominator) {
        (Some(first_variable), Some(second_variable)) => {
            mixed_derivative_denominator(cx, partial_symbol, first_variable, second_variable)
        }
        (Some(variable), None) => derivative_denominator(cx, partial_symbol, variable, order),
        (None, None) => derivative_denominator(cx, partial_symbol, first, order),
        (None, Some(_)) => {
            return Err(cx.for_rule(rule_key).invalid_shape(format!(
                "{subject} cannot carry a mixed denominator without a first denominator"
            )));
        }
    };
    let replacement = derivative_fraction(starred, numerator, denominator);

    cx.ast.replace_node_drop_detached_children(node_id, replacement);
    Ok(RuleEffect::Applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PDV_TO_FRAC_PARTIAL,
        level: Expand,
        examples: [
        {
            label: pdv_embedded,
            packages: ["base", "physics"],
            input: r"\lambda=\pdv{f}{x}",
            expected: r"\lambda=\frac{\partial f}{\partial x}",
        },
        {
            label: pdv_operator_form,
            packages: ["base", "physics"],
            input: r"\pdv{x} f",
            expected: r"\frac{\partial}{\partial x} f",
        },
        {
            label: pdv_second_order,
            packages: ["base", "physics"],
            input: r"\pdv[2]{f}{x}",
            expected: r"\frac{\partial^{2} f}{\partial x^{2}}",
        },
        {
            label: pdv_mixed_partial,
            packages: ["base", "physics"],
            input: r"\pdv{f}{x}{y}",
            expected: r"\frac{\partial^{2} f}{\partial x \partial y}",
        },
        {
            label: pdv_star_flatfrac,
            packages: ["base", "physics"],
            input: r"\pdv*{f}{x}",
            expected: r"\flatfrac{\partial f}{\partial x}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: PDV_TO_FRAC_PARTIAL,
        level: Expand,
        examples: [
        {
            label: pdv_ordered_operator_form,
            packages: ["base", "physics"],
            input: r"\pdv[2]{x} f",
            expected: r"\frac{\partial^{2}}{\partial x^{2}} f",
        },
        {
            label: pdv_mixed_partial_ignores_optional_order,
            packages: ["base", "physics"],
            input: r"\pdv[3]{f}{x}{y}",
            expected: r"\frac{\partial^{2} f}{\partial x \partial y}",
        },
        {
            label: pdv_starred_ordered_two_argument,
            packages: ["base", "physics"],
            input: r"\pdv*[2]{f}{x}",
            expected: r"\flatfrac{\partial^{2} f}{\partial x^{2}}",
        },
        {
            label: pdv_starred_mixed_partial,
            packages: ["base", "physics"],
            input: r"\pdv*{f}{x}{y}",
            expected: r"\flatfrac{\partial^{2} f}{\partial x \partial y}",
        },
        {
            label: pdv_starred_ordered_mixed_partial,
            packages: ["base", "physics"],
            input: r"\pdv*[n]{f}{x}{y}",
            expected: r"\flatfrac{\partial^{2} f}{\partial x \partial y}",
        },
        {
            label: pdv_empty_optional_order,
            packages: ["base", "physics"],
            input: r"\pdv[]{f}{x}",
            expected: r"\frac{\partial^{} f}{\partial x^{}}",
        },
        ]
    }
}
