//! Expand dv forms to explicit d-based derivative fractions.
//!
//! ```yaml
//! proposal: dv-to-frac-d
//! triggers:
//!   - cmd:dv
//! consumes:
//!   eliminates: cmd:dv
//!   touches: null
//! produces:
//!   - cmd:frac
//!   - cmd:flatfrac
//!   - cmd:mathrm
//! rewrite_patterns:
//!   - {label: dv-two-argument, from: '\dv{#1}{#2}', to: '\frac{\mathrm{d} #1}{\mathrm{d} #2}'}
//!   - {label: dv-operator, from: '\dv{#1}', to: '\frac{\mathrm{d}}{\mathrm{d} #1}'}
//!   - {label: dv-order, from: '\dv[#1]{#2}{#3}', to: '\frac{\mathrm{d}^{#1} #2}{\mathrm{d} #3^{#1}}'}
//!   - {label: dv-star, from: '\dv*{#1}{#2}', to: '\flatfrac{\mathrm{d} #1}{\mathrm{d} #2}'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{
    derivative_denominator, derivative_fraction, derivative_numerator, differential_d,
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
    pub static DV_TO_FRAC_D: DvToFracDRule {
        key: Physics / "dv-to-frac-d",
        class: Expand,
        summary: "Expand dv forms to explicit d-based derivative fractions.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::DV],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::DV],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC, &physics::cmd::FLATFRAC, &base::cmd::MATHRM],
        },
        apply(rule, cx, node_id) {
            expand_dv(rule, cx, node_id)
        }
    }
}

fn expand_dv(
    rule: &DvToFracDRule,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
) -> Result<RuleEffect, TransformError> {
    let Some(command) = cx.match_command(node_id, &physics::cmd::DV) else {
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
        derivative_numerator(cx, differential_d, order, Some(first))
    } else {
        derivative_numerator(cx, differential_d, order, None)
    };
    let variable = denominator.unwrap_or(first);
    let denominator = derivative_denominator(cx, differential_d, variable, order);
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
        rule: DV_TO_FRAC_D,
        class: Expand,
        examples: [
        {
            label: dv_kinematics,
            packages: ["base", "physics"],
            input: r"m\dv{x}{t}=p",
            expected: r"m\frac{\mathrm{d} x}{\mathrm{d} t}=p",
        },
        {
            label: dv_operator_form,
            packages: ["base", "physics"],
            input: r"\dv{x} f",
            expected: r"\frac{\mathrm{d}}{\mathrm{d} x} f",
        },
        {
            label: dv_second_order,
            packages: ["base", "physics"],
            input: r"\dv[2]{x}{t}",
            expected: r"\frac{\mathrm{d}^{2} x}{\mathrm{d} t^{2}}",
        },
        {
            label: dv_star_flatfrac,
            packages: ["base", "physics"],
            input: r"\dv*{x}{t}",
            expected: r"\flatfrac{\mathrm{d} x}{\mathrm{d} t}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: DV_TO_FRAC_D,
        class: Expand,
        examples: [
        {
            label: dv_ordered_operator_form,
            packages: ["base", "physics"],
            input: r"\dv[2]{x} f",
            expected: r"\frac{\mathrm{d}^{2}}{\mathrm{d} x^{2}} f",
        },
        {
            label: dv_starred_operator_form,
            packages: ["base", "physics"],
            input: r"\dv*{x} f",
            expected: r"\flatfrac{\mathrm{d}}{\mathrm{d} x} f",
        },
        {
            label: dv_starred_ordered_two_argument,
            packages: ["base", "physics"],
            input: r"\dv*[2]{x}{t}",
            expected: r"\flatfrac{\mathrm{d}^{2} x}{\mathrm{d} t^{2}}",
        },
        {
            label: dv_empty_optional_order,
            packages: ["base", "physics"],
            input: r"\dv[]{x}{t}",
            expected: r"\frac{\mathrm{d}^{} x}{\mathrm{d} t^{}}",
        },
        ]
    }
}
