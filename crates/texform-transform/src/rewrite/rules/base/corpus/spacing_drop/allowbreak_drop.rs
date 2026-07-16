//! Drop allowbreak as a pure linebreak hint during cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: allowbreak-drop
//! triggers:
//!   - cmd:allowbreak
//! consumes:
//!   eliminates: cmd:allowbreak
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \allowbreak, to: ''}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_layout_hint;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static ALLOWBREAK_DROP: AllowbreakDropRule {
        key: Base / "allowbreak-drop",
        level: Corpus,
        summary: "Drop allowbreak as a pure linebreak hint during cleanup-oriented normalization.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::ALLOWBREAK],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ALLOWBREAK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::ALLOWBREAK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\allowbreak")?;

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
        rule: ALLOWBREAK_DROP,
        level: Corpus,
        examples: [
        {
            label: allowbreak_between_sum_terms,
            packages: ["base"],
            input: r"f(x_1)+\allowbreak f(x_2)+f(x_3)",
            expected: r"f(x_1)+f(x_2)+f(x_3)",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: ALLOWBREAK_DROP,
        level: Corpus,
        examples: [
        {
            label: allowbreak_as_script_base,
            packages: ["base"],
            input: r"\allowbreak^2",
            expected: r"{}^2",
        },
        ]
    }
}
