//! Rewrite infix choose to an explicit binom command.
//!
//! ```yaml
//! proposal: choose-to-binom
//! triggers:
//!   - cmd:choose
//! consumes:
//!   eliminates: cmd:choose
//!   touches: null
//! produces: cmd:binom
//! rewrite_patterns:
//!   - {from: '#1 \choose #2', to: '\binom{#1}{#2}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::replace_infix_with_command;
use crate::ast::ContentMode;
use crate::rewrite::helpers::infix_prefix_args;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static CHOOSE_TO_BINOM: ChooseToBinomRule {
        key: Base / "choose-to-binom",
        level: Standard,
        summary: "Rewrite infix choose to an explicit binom command.",
        fidelity: Lossless,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::CHOOSE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::CHOOSE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::BINOM],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::CHOOSE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(infix.args, r"\choose")?;
            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::BINOM,
                infix_prefix_args(infix.left, infix.right, ContentMode::Math),
            );
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
        rule: CHOOSE_TO_BINOM,
        level: Standard,
        examples: [
        {
            label: offset_binomial,
            packages: ["base", "ams"],
            input: r"n+r-1 \choose 2k+1",
            expected: r"\binom{n+r-1}{2k+1}",
        },
        {
            label: binomial_coefficient_in_product,
            packages: ["base", "ams"],
            input: r"P(X=k)={n \choose k}p^k(1-p)^{n-k}",
            expected: r"P(X=k)=\binom{n}{k}p^k(1-p)^{n-k}",
        },
        ]
    }
    // END: Generated examples

}
