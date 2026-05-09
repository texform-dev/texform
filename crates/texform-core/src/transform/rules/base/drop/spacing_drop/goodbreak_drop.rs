//! Drop goodbreak as a pure linebreak hint during cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: goodbreak-drop
//! triggers:
//!   - cmd:goodbreak
//! consumes:
//!   eliminates: cmd:goodbreak
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \goodbreak, to: ''}
//! ```

use texform_specs::builtin::base;

use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Drop goodbreak as a pure linebreak hint during cleanup-oriented normalization.
    pub static GOODBREAK_DROP: GoodbreakDropRule {
        key: Base / "goodbreak-drop",
        class: Drop,
        summary: "Drop goodbreak as a pure linebreak hint during cleanup-oriented normalization.",
        phase: Normalize,
        safety: Semantic,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::GOODBREAK],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::GOODBREAK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::GOODBREAK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, command.args, "\\goodbreak")?;

            cx.ast.remove_node(node_id);
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
        rule: GOODBREAK_DROP,
        class: Drop,
        examples: [
        {
            label: goodbreak_inside_polynomial,
            packages: ["base"],
            input: r"x_1+x_2\goodbreak+x_3+x_4",
            expected: r"x_1+x_2+x_3+x_4",
        },
        {
            label: goodbreak_before_condition,
            packages: ["base"],
            input: r"S_n=a_1+\cdots+a_n\goodbreak,\ n\ge 1",
            expected: r"S_n=a_1+\cdots+a_n,\ n\ge 1",
        },
        ]
    }
    // END: Generated examples
}
