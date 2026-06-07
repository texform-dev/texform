//! Expand bra to an explicit angle-bracket and bar fence form.
//!
//! ```yaml
//! proposal: physics-bra-expand
//! triggers:
//!   - cmd:bra
//! consumes:
//!   eliminates: cmd:bra
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: bra-auto-sized, from: '\bra{#1}', to: '\left\langle #1 \right\vert'}
//!   - {label: bra-fixed-size, from: '\bra*{#1}', to: '\langle #1 \vert'}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use super::helpers::{BraketSize, required_math_arg, replace_with_bra};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static PHYSICS_BRA_EXPAND: PhysicsBraExpandRule {
        key: Physics / "physics-bra-expand",
        level: Expand,
        summary: "Expand bra to an explicit angle-bracket and bar fence form.",
        fidelity: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::BRA],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::BRA],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::BRA) else {
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
            replace_with_bra(cx, node_id, size, body);
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
        rule: PHYSICS_BRA_EXPAND,
        level: Expand,
        examples: [
        {
            label: bra_acting_on_operator,
            packages: ["base", "physics"],
            input: r"\bra{\psi} H",
            expected: r"\left\langle \psi \right\vert H",
        },
        {
            label: bra_star_fixed_size,
            packages: ["base", "physics"],
            input: r"\bra*{\psi} H",
            expected: r"\langle \psi \vert H",
        },
        ]
    }
    // END: Generated examples
}
