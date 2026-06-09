//! Rewrite infix brack to an explicit genfrac with bracket delimiters.
//!
//! ```yaml
//! proposal: brack-to-genfrac
//! triggers:
//!   - cmd:brack
//! consumes:
//!   eliminates: cmd:brack
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \brack #2', to: '\genfrac{[}{]}{0pt}{}{#1}{#2}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::{genfrac_args, replace_infix_with_command};
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BRACK_TO_GENFRAC: BrackToGenfracRule {
        key: Base / "brack-to-genfrac",
        level: Standard,
        summary: "Rewrite infix brack to an explicit genfrac with bracket delimiters.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::BRACK],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BRACK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::GENFRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::BRACK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(infix.args, r"\brack")?;

            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::GENFRAC,
                genfrac_args(
                    Delimiter::Char('['),
                    Delimiter::Char(']'),
                    "0pt",
                    "",
                    infix.left,
                    infix.right,
                ),
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
        rule: BRACK_TO_GENFRAC,
        level: Standard,
        examples: [
        {
            label: bracket_delimited_stack,
            packages: ["base", "ams"],
            input: r"\int_0^1 f(x)\,dx \brack \sum_{k=1}^{n} b_k",
            expected: r"\genfrac{[}{]}{0pt}{}{\int_0^1 f(x)\,dx}{\sum_{k=1}^{n} b_k}",
        },
        ]
    }
    // END: Generated examples

}
