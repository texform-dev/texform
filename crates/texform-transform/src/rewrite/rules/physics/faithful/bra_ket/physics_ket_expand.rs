//! Expand ket to an explicit bar and angle-bracket fence form.
//!
//! ```yaml
//! proposal: physics-ket-expand
//! triggers:
//!   - cmd:ket
//! consumes:
//!   eliminates: cmd:ket
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: ket-auto-sized, from: '\ket{#1}', to: '\left\vert #1 \right\rangle'}
//!   - {label: ket-fixed-size, from: '\ket*{#1}', to: '\vert #1 \rangle'}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use super::helpers::{BraketSize, required_math_arg, replace_with_ket};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static PHYSICS_KET_EXPAND: PhysicsKetExpandRule {
        key: Physics / "physics-ket-expand",
        level: Faithful,
        summary: "Expand ket to an explicit bar and angle-bracket fence form.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::KET],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::KET],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::KET) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();
            cx.for_rule(Self::KEY).expect_arg_len(&args, 2, &subject)?;

            let starred = cx.for_rule(Self::KEY).star_arg_value(&args[0], &subject)?;
            let body = required_math_arg(Self::KEY, cx, &args[1], &subject, "body")?;
            let size = if starred {
                BraketSize::Fixed
            } else {
                BraketSize::Auto
            };
            replace_with_ket(cx, node_id, size, body);
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PHYSICS_KET_EXPAND,
        level: Faithful,
        examples: [
        {
            label: ket_eigenvalue,
            packages: ["base", "physics"],
            input: r"H\ket{\psi}=E\ket{\psi}",
            expected: r"H\left\vert \psi \right\rangle=E\left\vert \psi \right\rangle",
        },
        {
            label: ket_star_fixed_size,
            packages: ["base", "physics"],
            input: r"H\ket*{\psi}=E\ket*{\psi}",
            expected: r"H\vert \psi \rangle=E\vert \psi \rangle",
        },
        ]
    }
    // END: Generated examples
}
