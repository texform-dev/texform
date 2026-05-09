//! Rewrite infix atop to an explicit no-rule genfrac form.
//!
//! ```yaml
//! proposal: atop-to-genfrac
//! consumes:
//!   eliminates: cmd:atop
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \atop #2', to: '\genfrac{}{}{0pt}{}{#1}{#2}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{genfrac_args, replace_infix_with_command};
use crate::ast::Delimiter;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite infix atop to an explicit no-rule genfrac form.
    pub static ATOP_TO_GENFRAC: AtopToGenfracRule {
        key: Base / "atop-to-genfrac",
        class: Standard,
        summary: "Rewrite infix atop to an explicit no-rule genfrac form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ATOP],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::GENFRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::ATOP) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, infix.args, r"\atop")?;

            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::GENFRAC,
                genfrac_args(
                    Delimiter::None,
                    Delimiter::None,
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
        rule: ATOP_TO_GENFRAC,
        class: Standard,
        examples: [
        {
            label: no_rule_stacked_fraction,
            packages: ["base", "ams"],
            input: r"x_1+\cdots+x_m \atop y_1y_2\cdots y_n",
            expected: r"\genfrac{}{}{0pt}{}{x_1+\cdots+x_m}{y_1y_2\cdots y_n}",
        },
        {
            label: script_atop_condition,
            packages: ["base", "ams"],
            input: r"\sum_{i \atop i\ne j} a_i",
            expected: r"\sum_{\genfrac{}{}{0pt}{}{i}{i\ne j}} a_i",
        },
        ]
    }
    // END: Generated examples

}
