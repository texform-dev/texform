//! Expand poisson brackets to explicit brace fences.
//!
//! ```yaml
//! proposal: poisson-bracket-expand
//! consumes:
//!   eliminates: cmd:pb
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: poisson-bracket-auto-sized, from: '\pb{#1}{#2}', to: '\left\{#1,#2\right\}'}
//!   - {label: poisson-bracket-fixed-size, from: '\pb*{#1}{#2}', to: '\{#1,#2\}'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{
    BinaryFencePair, FixedFenceToken, replace_with_binary_bracket_fence,
    required_braced_math_arg, required_math_arg,
};
use crate::ast::Delimiter;
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand poisson brackets to explicit brace fences.
    pub static POISSON_BRACKET_EXPAND: PoissonBracketExpandRule {
        key: Physics / "poisson-bracket-expand",
        class: Expand,
        summary: "Expand poisson brackets to explicit brace fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::PB],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::PB) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();

            cx.expect_arg_len(rule.meta().key, &args, 3, &subject)?;
            let starred = star_arg_value(rule.meta().key, cx, &args[0], &subject)?;
            let left = required_braced_math_arg(rule.meta().key, cx, &args[1], &subject, "left operand")?;
            let right = required_math_arg(rule.meta().key, cx, &args[2], &subject, "right operand")?;

            replace_with_binary_bracket_fence(
                cx,
                node_id,
                starred,
                left,
                right,
                BinaryFencePair {
                    auto_left: Delimiter::Control("{".to_string()),
                    auto_right: Delimiter::Control("}".to_string()),
                    fixed_left: FixedFenceToken::Control("{"),
                    fixed_right: FixedFenceToken::Control("}"),
                },
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: POISSON_BRACKET_EXPAND,
        class: Expand,
        examples: [
        {
            label: poisson_bracket_canonical,
            packages: ["base", "physics"],
            input: r"\pb{q_i}{p_j}=\delta_{ij}",
            expected: r"\left\{q_i,p_j\right\}=\delta_{ij}",
        },
        {
            label: poisson_bracket_star_fixed_size,
            packages: ["base", "physics"],
            input: r"\pb*{f}{g}=0",
            expected: r"\{f,g\}=0",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: POISSON_BRACKET_EXPAND,
        class: Expand,
        examples: [
        {
            label: poisson_bracket_bare_second_operand,
            packages: ["base", "physics"],
            input: r"\pb{f}g",
            expected: r"\left\{f,g\right\}",
        },
        {
            label: poisson_bracket_star_power_context,
            packages: ["base", "physics"],
            input: r"\pb*{f}{g}^2",
            expected: r"\{f,g\}^2",
        },
        ]
    }
}
