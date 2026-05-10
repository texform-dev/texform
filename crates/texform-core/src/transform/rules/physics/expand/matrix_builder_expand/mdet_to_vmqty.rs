//! Expand mdet to the vmqty determinant-style builder.
//!
//! ```yaml
//! proposal: mdet-to-vmqty
//! triggers:
//!   - cmd:mdet
//! consumes:
//!   eliminates: cmd:mdet
//!   touches: null
//! produces: cmd:vmqty
//! rewrite_patterns:
//!   - {from: '\mdet{#1}', to: '\vmqty{#1}'}
//! ```

use texform_specs::builtin::physics;

use crate::transform::helpers::{prefix_command, required_math_content};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static MDET_TO_VMQTY: MdetToVmqtyRule {
        key: Physics / "mdet-to-vmqty",
        class: Expand,
        summary: "Expand mdet to the vmqty determinant-style builder.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::MDET],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::MDET],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::VMQTY],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::MDET) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();

            cx.expect_arg_len(rule.meta().key, &args, 1, &subject)?;
            let _body = required_math_content(rule.meta().key, cx, &args[0], &subject, "body")?;

            cx.ast
                .replace_node(node_id, prefix_command(&physics::cmd::VMQTY, args));
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
        rule: MDET_TO_VMQTY,
        class: Expand,
        examples: [
        {
            label: mdet_characteristic_polynomial,
            packages: ["base", "physics"],
            input: r"\chi_A(\lambda)=\mdet{A-\lambda I}",
            expected: r"\chi_A(\lambda)=\vmqty{A-\lambda I}",
        },
        ]
    }
    // END: Generated examples
}
