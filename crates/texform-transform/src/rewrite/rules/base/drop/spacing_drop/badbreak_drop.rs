//! Drop badbreak as a pure linebreak hint during cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: badbreak-drop
//! triggers:
//!   - cmd:badbreak
//! consumes:
//!   eliminates: cmd:badbreak
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \badbreak, to: ''}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_layout_hint;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BADBREAK_DROP: BadbreakDropRule {
        key: Base / "badbreak-drop",
        level: Drop,
        summary: "Drop badbreak as a pure linebreak hint during cleanup-oriented normalization.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::BADBREAK],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BADBREAK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::BADBREAK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\badbreak")?;

            drop_layout_hint(cx.ast, node_id);
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
        rule: BADBREAK_DROP,
        level: Drop,
        examples: [
        {
            label: badbreak_inside_series,
            packages: ["base"],
            input: r"a_0+a_1\badbreak+a_2+a_3",
            expected: r"a_0+a_1+a_2+a_3",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: BADBREAK_DROP,
        level: Drop,
        examples: [
        {
            label: badbreak_as_script_base,
            packages: ["base"],
            input: r"\badbreak^2",
            expected: r"{}^2",
        },
        ]
    }
}
