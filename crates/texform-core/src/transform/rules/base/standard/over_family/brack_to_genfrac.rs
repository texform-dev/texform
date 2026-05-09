//! Rewrite infix brack to an explicit genfrac with bracket delimiters.
//!
//! ```yaml
//! proposal: brack-to-genfrac
//! consumes:
//!   eliminates: cmd:brack
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \brack #2', to: '\genfrac{[}{]}{0pt}{}{#1}{#2}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{genfrac_args, replace_infix_with_command};
use crate::ast::Delimiter;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite infix brack to an explicit genfrac with bracket delimiters.
    pub static BRACK_TO_GENFRAC: BrackToGenfracRule {
        key: Base / "brack-to-genfrac",
        class: Standard,
        summary: "Rewrite infix brack to an explicit genfrac with bracket delimiters.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
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
            cx.expect_no_args(rule.meta().key, infix.args, r"\brack")?;

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
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BRACK_TO_GENFRAC,
        class: Standard,
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
