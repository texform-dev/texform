//! Expand commutators to explicit square-bracket fences.
//!
//! ```yaml
//! proposal: commutator-expand
//! triggers:
//!   - cmd:comm
//! consumes:
//!   eliminates: cmd:comm
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: commutator-auto-sized, from: '\comm{#1}{#2}', to: '\left[#1,#2\right]'}
//!   - {label: commutator-fixed-size, from: '\comm*{#1}{#2}', to: '[#1,#2]'}
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
    pub static COMMUTATOR_EXPAND: CommutatorExpandRule {
        key: Physics / "commutator-expand",
        class: Expand,
        summary: "Expand commutators to explicit square-bracket fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::COMM],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::COMM],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::COMM) else {
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
                    auto_left: Delimiter::Char('['),
                    auto_right: Delimiter::Char(']'),
                    fixed_left: FixedFenceToken::Char('['),
                    fixed_right: FixedFenceToken::Char(']'),
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
        rule: COMMUTATOR_EXPAND,
        class: Expand,
        examples: [
        {
            label: commutator_angular_momentum,
            packages: ["base", "physics"],
            input: r"\comm{L_x}{L_y}=i\hbar L_z",
            expected: r"\left[L_x,L_y\right]=i\hbar L_z",
        },
        {
            label: commutator_star_fixed_size,
            packages: ["base", "physics"],
            input: r"\comm*{A}{B}=0",
            expected: r"[A,B]=0",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: COMMUTATOR_EXPAND,
        class: Expand,
        examples: [
        {
            label: commutator_bare_second_operand,
            packages: ["base", "physics"],
            input: r"\comm{A}B",
            expected: r"\left[A,B\right]",
        },
        {
            label: commutator_star_power_context,
            packages: ["base", "physics"],
            input: r"\comm*{A}{B}^2",
            expected: r"[A,B]^2",
        },
        ]
    }
}
