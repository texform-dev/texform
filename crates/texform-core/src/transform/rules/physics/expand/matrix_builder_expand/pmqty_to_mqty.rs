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

use texform_specs::builtin::physics;

use super::helpers::matrix_quantity_command;
use crate::transform::helpers::required_math_content;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static PMQTY_TO_MQTY: PmqtyToMqtyRule {
        key: Physics / "pmqty-to-mqty",
        class: Expand,
        summary: "Expand pmqty to the core mqty builder with paren fence syntax.",
        phase: Normalize,
        safety: Lossless,
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
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();

            cx.expect_arg_len(rule.meta().key, &args, 1, &subject)?;
            let body = required_math_content(rule.meta().key, cx, &args[0], &subject, "body")?;

            cx.ast
                .replace_node(node_id, matrix_quantity_command(body, '(', ')'));
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
        rule: PMQTY_TO_MQTY,
        class: Expand,
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
