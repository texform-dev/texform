//! Expand braket to an explicit angle-bracket form with a middle bar.
//!
//! ```yaml
//! proposal: physics-braket-expand
//! triggers:
//!   - cmd:braket
//! consumes:
//!   eliminates: cmd:braket
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:middle
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: braket-two-arg-auto-sized, from: '\braket{#1}{#2}', to: '\left\langle #1 \middle\vert #2 \right\rangle'}
//!   - {label: braket-two-arg-fixed-size, from: '\braket*{#1}{#2}', to: '\langle #1 \vert #2 \rangle'}
//!   - {label: braket-single-arg-auto-sized, from: '\braket{#1}', to: '\left\langle #1 \middle\vert #1 \right\rangle'}
//!   - {label: braket-single-arg-fixed-size, from: '\braket*{#1}', to: '\langle #1 \vert #1 \rangle'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{
    BraketSize, optional_group_arg, required_math_arg, replace_with_braket,
};
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand braket to an explicit angle-bracket form with a middle bar.
    pub static PHYSICS_BRAKET_EXPAND: PhysicsBraketExpandRule {
        key: Physics / "physics-braket-expand",
        class: Expand,
        summary: "Expand braket to an explicit angle-bracket form with a middle bar.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::BRAKET],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::BRAKET],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::MIDDLE, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::BRAKET) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();
            cx.expect_arg_len(rule.meta().key, &args, 3, &subject)?;

            let starred = star_arg_value(rule.meta().key, cx, &args[0], &subject)?;
            let left = required_math_arg(rule.meta().key, cx, &args[1], &subject, "left side")?;
            let right =
                optional_group_arg(rule.meta().key, cx, &args[2], &subject, "right side")?
                    .unwrap_or(left);
            let size = if starred {
                BraketSize::Fixed
            } else {
                BraketSize::Middle
            };
            replace_with_braket(cx, node_id, size, left, right);
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
        rule: PHYSICS_BRAKET_EXPAND,
        class: Expand,
        examples: [
        {
            label: braket_orthonormality,
            packages: ["base", "physics"],
            input: r"\braket{\psi_n}{\psi_m}=\delta_{nm}",
            expected: r"\left\langle \psi_n \middle\vert \psi_m \right\rangle=\delta_{nm}",
        },
        {
            label: braket_single_argument_self_overlap,
            packages: ["base", "physics"],
            input: r"N=\braket{\psi}",
            expected: r"N=\left\langle \psi \middle\vert \psi \right\rangle",
        },
        {
            label: braket_star_fixed_size,
            packages: ["base", "physics"],
            input: r"\braket*{u}{v}=0",
            expected: r"\langle u \vert v \rangle=0",
        },
        ]
    }
    // END: Generated examples
}
