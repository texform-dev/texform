//! Expand bmqty to the core mqty builder with bracket fence syntax.
//!
//! ```yaml
//! proposal: bmqty-to-mqty
//! triggers:
//!   - cmd:bmqty
//! consumes:
//!   eliminates: cmd:bmqty
//!   touches: null
//! produces: cmd:mqty
//! rewrite_patterns:
//!   - {from: '\bmqty{#1}', to: '\mqty[#1]'}
//! ```

use texform_specs::builtin::physics;

use super::helpers::matrix_quantity_command;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static BMQTY_TO_MQTY: BmqtyToMqtyRule {
        key: Physics / "bmqty-to-mqty",
        class: Expand,
        summary: "Expand bmqty to the core mqty builder with bracket fence syntax.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::BMQTY],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::BMQTY],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::MQTY],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::BMQTY) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();

            cx.for_rule(Self::KEY).expect_arg_len(&args, 1, &subject)?;
            let body = cx.for_rule(Self::KEY).mandatory_math_content(&args[0], &subject, "body")?;

            cx.ast
                .replace_node(node_id, matrix_quantity_command(body, '[', ']'));
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
        rule: BMQTY_TO_MQTY,
        class: Expand,
        examples: [
        {
            label: bmqty_gram_matrix,
            packages: ["base", "physics"],
            input: r"G=\bmqty{g_{11}&g_{12}\\g_{21}&g_{22}}",
            expected: r"G=\mqty[g_{11}&g_{12}\\g_{21}&g_{22}]",
        },
        ]
    }
    // END: Generated examples
}
