//! Rewrite atopwithdelims to an explicit delimited no-rule genfrac form.
//!
//! ```yaml
//! proposal: atopwithdelims-to-genfrac
//! consumes:
//!   eliminates: cmd:atopwithdelims
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \atopwithdelims #2 #3 #4', to: '\genfrac{#2}{#3}{0pt}{}{#1}{#4}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{delimiter_arg, genfrac_args, replace_infix_with_command};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite atopwithdelims to an explicit delimited no-rule genfrac form.
    pub static ATOPWITHDELIMS_TO_GENFRAC: AtopwithdelimsToGenfracRule {
        key: Base / "atopwithdelims-to-genfrac",
        tier: Base,
        summary: "Rewrite atopwithdelims to an explicit delimited no-rule genfrac form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ATOPWITHDELIMS],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::GENFRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::ATOPWITHDELIMS) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_arg_len(rule.meta().key, infix.args, 2, r"\atopwithdelims")?;
            let left_delimiter =
                delimiter_arg(rule.meta().key, cx, &infix.args[0], r"\atopwithdelims", "left delimiter")?;
            let right_delimiter =
                delimiter_arg(rule.meta().key, cx, &infix.args[1], r"\atopwithdelims", "right delimiter")?;

            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::GENFRAC,
                genfrac_args(
                    left_delimiter,
                    right_delimiter,
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
        rule: ATOPWITHDELIMS_TO_GENFRAC,
        tier: Base,
        examples: [
        {
            label: delimited_no_rule_stack,
            packages: ["base", "ams"],
            input: r"\sum_{i=1}^{m} a_i \atopwithdelims [ ] \prod_{j=1}^{n} b_j",
            expected: r"\genfrac{[}{]}{0pt}{}{\sum_{i=1}^{m} a_i}{\prod_{j=1}^{n} b_j}",
        },
        {
            label: parenthesized_no_rule_stack,
            packages: ["base", "ams"],
            input: r"A+B \atopwithdelims ( ) C+D",
            expected: r"\genfrac{(}{)}{0pt}{}{A+B}{C+D}",
        },
        ]
    }
    // END: Generated examples

}
