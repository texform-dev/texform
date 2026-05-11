//! Rewrite overwithdelims to an explicit genfrac with delimiters.
//!
//! ```yaml
//! proposal: overwithdelims-to-genfrac
//! triggers:
//!   - cmd:overwithdelims
//! consumes:
//!   eliminates: cmd:overwithdelims
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \overwithdelims #2 #3 #4', to: '\genfrac{#2}{#3}{}{}{#1}{#4}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{delimiter_arg, genfrac_args, replace_infix_with_command};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static OVERWITHDELIMS_TO_GENFRAC: OverwithdelimsToGenfracRule {
        key: Base / "overwithdelims-to-genfrac",
        class: Standard,
        summary: "Rewrite overwithdelims to an explicit genfrac with delimiters.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::OVERWITHDELIMS],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::OVERWITHDELIMS],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::GENFRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::OVERWITHDELIMS) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(infix.args, 2, r"\overwithdelims")?;
            let left_delimiter =
                delimiter_arg(Self::KEY, cx, &infix.args[0], r"\overwithdelims", "left delimiter")?;
            let right_delimiter =
                delimiter_arg(Self::KEY, cx, &infix.args[1], r"\overwithdelims", "right delimiter")?;

            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::GENFRAC,
                genfrac_args(
                    left_delimiter,
                    right_delimiter,
                    "",
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
        rule: OVERWITHDELIMS_TO_GENFRAC,
        class: Standard,
        examples: [
        {
            label: parenthesized_genfrac,
            packages: ["base", "ams"],
            input: r"x_1+\cdots+x_m \overwithdelims ( ) \sum_{j=1}^{n} y_j",
            expected: r"\genfrac{(}{)}{}{}{x_1+\cdots+x_m}{\sum_{j=1}^{n} y_j}",
        },
        {
            label: bracket_delimited_genfrac,
            packages: ["base", "ams"],
            input: r"u+v \overwithdelims [ ] w+z",
            expected: r"\genfrac{[}{]}{}{}{u+v}{w+z}",
        },
        ]
    }
    // END: Generated examples

}
