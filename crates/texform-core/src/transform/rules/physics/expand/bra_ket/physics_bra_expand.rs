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

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{BraketSize, required_math_arg, replace_with_bra};
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand bra to an explicit angle-bracket and bar fence form.
    pub static PHYSICS_BRA_EXPAND: PhysicsBraExpandRule {
        key: Physics / "physics-bra-expand",
        class: Expand,
        summary: "Expand bra to an explicit angle-bracket and bar fence form.",
        phase: Normalize,
        safety: Lossless,
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
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();
            cx.expect_arg_len(rule.meta().key, &args, 2, &subject)?;

            let starred = star_arg_value(rule.meta().key, cx, &args[0], &subject)?;
            let body = required_math_arg(rule.meta().key, cx, &args[1], &subject, "body")?;
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
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PHYSICS_BRA_EXPAND,
        class: Expand,
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
