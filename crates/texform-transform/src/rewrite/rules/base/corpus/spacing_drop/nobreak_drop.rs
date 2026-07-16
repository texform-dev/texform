//! Drop nobreak as a pure linebreak hint during cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: nobreak-drop
//! triggers:
//!   - cmd:nobreak
//! consumes:
//!   eliminates: cmd:nobreak
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \nobreak, to: ''}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_layout_hint;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NOBREAK_DROP: NobreakDropRule {
        key: Base / "nobreak-drop",
        level: Corpus,
        summary: "Drop nobreak as a pure linebreak hint during cleanup-oriented normalization.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NOBREAK],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::NOBREAK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::NOBREAK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\nobreak")?;

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
        rule: NOBREAK_DROP,
        level: Corpus,
        examples: [
        {
            label: nobreak_inside_sum,
            packages: ["base"],
            input: r"m+n\nobreak+p+q",
            expected: r"m+n+p+q",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NOBREAK_DROP,
        level: Corpus,
        examples: [
        {
            label: nobreak_as_script_base,
            packages: ["base"],
            input: r"\nobreak^2",
            expected: r"{}^2",
        },
        ]
    }
}
