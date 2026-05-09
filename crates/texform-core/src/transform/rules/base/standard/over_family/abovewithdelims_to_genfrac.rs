//! Rewrite abovewithdelims to an explicit genfrac with delimiters and thickness.
//!
//! ```yaml
//! proposal: abovewithdelims-to-genfrac
//! triggers:
//!   - cmd:abovewithdelims
//! consumes:
//!   eliminates: cmd:abovewithdelims
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \abovewithdelims #2 #3 #4 #5', to: '\genfrac{#2}{#3}{#4}{}{#1}{#5}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{delimiter_arg, dimension_arg, genfrac_args, replace_infix_with_command};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite abovewithdelims to an explicit genfrac with delimiters and thickness.
    pub static ABOVEWITHDELIMS_TO_GENFRAC: AbovewithdelimsToGenfracRule {
        key: Base / "abovewithdelims-to-genfrac",
        class: Standard,
        summary: "Rewrite abovewithdelims to an explicit genfrac with delimiters and thickness.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::ABOVEWITHDELIMS],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ABOVEWITHDELIMS],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::GENFRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::ABOVEWITHDELIMS) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_arg_len(rule.meta().key, infix.args, 3, r"\abovewithdelims")?;
            let left_delimiter =
                delimiter_arg(rule.meta().key, cx, &infix.args[0], r"\abovewithdelims", "left delimiter")?;
            let right_delimiter =
                delimiter_arg(rule.meta().key, cx, &infix.args[1], r"\abovewithdelims", "right delimiter")?;
            let thickness =
                dimension_arg(rule.meta().key, cx, &infix.args[2], r"\abovewithdelims", "thickness")?;

            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::GENFRAC,
                genfrac_args(
                    left_delimiter,
                    right_delimiter,
                    thickness,
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
        rule: ABOVEWITHDELIMS_TO_GENFRAC,
        class: Standard,
        examples: [
        {
            label: delimited_thick_genfrac,
            packages: ["base", "ams"],
            input: r"\sum_{i=0}^{m} a_i t^i \abovewithdelims [ ] 0.8pt \int_0^1 f(x)\,dx",
            expected: r"\genfrac{[}{]}{0.8pt}{}{\sum_{i=0}^{m} a_i t^i}{\int_0^1 f(x)\,dx}",
        },
        {
            label: parenthesized_thick_genfrac,
            packages: ["base", "ams"],
            input: r"A+B \abovewithdelims ( ) 2pt C+D",
            expected: r"\genfrac{(}{)}{2pt}{}{A+B}{C+D}",
        },
        ]
    }
    // END: Generated examples

}
