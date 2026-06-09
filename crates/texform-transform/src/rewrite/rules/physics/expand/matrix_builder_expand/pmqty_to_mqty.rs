//! Expand pmqty to the core mqty builder with paren fence syntax.
//!
//! ```yaml
//! proposal: pmqty-to-mqty
//! triggers:
//!   - cmd:pmqty
//! consumes:
//!   eliminates: cmd:pmqty
//!   touches: null
//! produces: cmd:mqty
//! rewrite_patterns:
//!   - {from: '\pmqty{#1}', to: \mqty(#1)}
//! ```

use texform_knowledge::builtin::physics;

use super::helpers::matrix_quantity_command;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static PMQTY_TO_MQTY: PmqtyToMqtyRule {
        key: Physics / "pmqty-to-mqty",
        level: Expand,
        summary: "Expand pmqty to the core mqty builder with paren fence syntax.",
        fidelity: Full,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::PMQTY],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::PMQTY],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::MQTY],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::PMQTY) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();

            cx.for_rule(Self::KEY).expect_arg_len(&args, 1, &subject)?;
            let body = cx.for_rule(Self::KEY).mandatory_math_content(&args[0], &subject, "body")?;

            cx.ast
                .replace_node(node_id, matrix_quantity_command(body, '(', ')'));
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
        rule: PMQTY_TO_MQTY,
        level: Expand,
        examples: [
        {
            label: pmqty_matrix_assignment,
            packages: ["base", "physics"],
            input: r"A=\pmqty{a&b\\c&d}",
            expected: r"A=\mqty(a&b\\c&d)",
        },
        ]
    }
    // END: Generated examples
}
