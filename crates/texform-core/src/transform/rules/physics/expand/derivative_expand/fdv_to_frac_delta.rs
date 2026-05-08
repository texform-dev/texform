//! Expand fdv forms to explicit delta-based derivative fractions.
//!
//! ```yaml
//! proposal: fdv-to-frac-delta
//! consumes:
//!   eliminates: cmd:fdv
//!   touches: null
//! produces:
//!   - cmd:frac
//!   - cmd:flatfrac
//! rewrite_patterns:
//!   - {label: fdv-two-argument, from: '\fdv{#1}{#2}', to: '\frac{\delta #1}{\delta #2}'}
//!   - {label: fdv-operator, from: '\fdv{#1}', to: '\frac{\delta}{\delta #1}'}
//!   - {label: fdv-order, from: '\fdv[#1]{#2}{#3}', to: '\frac{\delta^{#1} #2}{\delta #3^{#1}}'}
//!   - {label: fdv-star, from: '\fdv*{#1}{#2}', to: '\flatfrac{\delta #1}{\delta #2}'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{
    delta_symbol, derivative_denominator, derivative_fraction, derivative_numerator,
};
use crate::ast::NodeId;
use crate::transform::engine::TransformError;
use crate::transform::helpers::{
    optional_group_math_content, optional_math_content, required_math_content, star_arg_value,
};
use crate::transform::helpers::replace_node_discarding_detached_children;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::rule_context::RuleContext;
use crate::transform::{TransformRule, cmd_targets, define_rule};

define_rule! {
    /// Expand fdv forms to explicit delta-based derivative fractions.
    pub static FDV_TO_FRAC_DELTA: FdvToFracDeltaRule {
        key: Physics / "fdv-to-frac-delta",
        tier: Expand,
        summary: "Expand fdv forms to explicit delta-based derivative fractions.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::FDV],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC, &physics::cmd::FLATFRAC],
        },
        apply(rule, cx, node_id) {
            expand_fdv(rule, cx, node_id)
        }
    }
}

fn expand_fdv(
    rule: &FdvToFracDeltaRule,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
) -> Result<RuleEffect, TransformError> {
    let Some(command) = cx.match_command(node_id, &physics::cmd::FDV) else {
        return Ok(RuleEffect::Skipped);
    };
    let subject = format!(r"\{}", command.name);
    let args = command.args.to_vec();

    cx.expect_arg_len(rule.meta().key, &args, 4, &subject)?;
    let rule_key = rule.meta().key;
    let starred = star_arg_value(rule_key, cx, &args[0], &subject)?;
    let order = optional_math_content(rule_key, cx, &args[1], &subject, "optional order")?;
    let first = required_math_content(rule_key, cx, &args[2], &subject, "first argument")?;
    let denominator =
        optional_group_math_content(rule_key, cx, &args[3], &subject, "denominator")?;

    let numerator = if denominator.is_some() {
        derivative_numerator(cx, delta_symbol, order, Some(first))
    } else {
        derivative_numerator(cx, delta_symbol, order, None)
    };
    let variable = denominator.unwrap_or(first);
    let denominator = derivative_denominator(cx, delta_symbol, variable, order);
    let replacement = derivative_fraction(starred, numerator, denominator);

    replace_node_discarding_detached_children(cx, node_id, replacement);
    Ok(RuleEffect::Applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: FDV_TO_FRAC_DELTA,
        tier: Expand,
        examples: [
        {
            label: fdv_euler_lagrange,
            packages: ["base", "physics"],
            input: r"\fdv{S[\phi]}{\phi(x)}=0",
            expected: r"\frac{\delta S[\phi]}{\delta \phi(x)}=0",
        },
        {
            label: fdv_operator_form,
            packages: ["base", "physics"],
            input: r"\fdv{\phi(x)} S",
            expected: r"\frac{\delta}{\delta \phi(x)} S",
        },
        {
            label: fdv_second_order,
            packages: ["base", "physics"],
            input: r"\fdv[2]{S}{\phi}",
            expected: r"\frac{\delta^{2} S}{\delta \phi^{2}}",
        },
        {
            label: fdv_star_flatfrac,
            packages: ["base", "physics"],
            input: r"\fdv*{S}{\phi}",
            expected: r"\flatfrac{\delta S}{\delta \phi}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: FDV_TO_FRAC_DELTA,
        tier: Expand,
        examples: [
        {
            label: fdv_ordered_operator_form,
            packages: ["base", "physics"],
            input: r"\fdv[2]{\phi} S",
            expected: r"\frac{\delta^{2}}{\delta \phi^{2}} S",
        },
        {
            label: fdv_starred_operator_form,
            packages: ["base", "physics"],
            input: r"\fdv*{\phi} S",
            expected: r"\flatfrac{\delta}{\delta \phi} S",
        },
        {
            label: fdv_starred_ordered_two_argument,
            packages: ["base", "physics"],
            input: r"\fdv*[2]{S}{\phi}",
            expected: r"\flatfrac{\delta^{2} S}{\delta \phi^{2}}",
        },
        {
            label: fdv_empty_optional_order,
            packages: ["base", "physics"],
            input: r"\fdv[]{S}{\phi}",
            expected: r"\frac{\delta^{} S}{\delta \phi^{}}",
        },
        ]
    }
}
