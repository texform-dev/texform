//! Rewrite above to a thickness-preserving genfrac form.
//!
//! ```yaml
//! proposal: above-to-genfrac
//! consumes:
//!   eliminates: cmd:above
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \above #2 #3', to: '\genfrac{}{}{#2}{}{#1}{#3}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{dimension_arg, genfrac_args, replace_infix_with_command};
use crate::ast::Delimiter;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite above to a thickness-preserving genfrac form.
    pub static ABOVE_TO_GENFRAC: AboveToGenfracRule {
        key: Base / "above-to-genfrac",
        tier: Base,
        summary: "Rewrite above to a thickness-preserving genfrac form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ABOVE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::GENFRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::ABOVE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_arg_len(rule.meta().key, infix.args, 1, r"\above")?;
            let thickness = dimension_arg(rule.meta().key, cx, &infix.args[0], r"\above", "thickness")?;

            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::GENFRAC,
                genfrac_args(
                    Delimiter::None,
                    Delimiter::None,
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
        rule: ABOVE_TO_GENFRAC,
        tier: Base,
        examples: [
        {
            label: rule_thickness_genfrac,
            packages: ["base", "ams"],
            input: r"\sum_{i=1}^{m} a_i x^i \above 1.2pt \prod_{j=1}^{n}(1+y_j)",
            expected: r"\genfrac{}{}{1.2pt}{}{\sum_{i=1}^{m} a_i x^i}{\prod_{j=1}^{n}(1+y_j)}",
        },
        {
            label: zero_thickness_genfrac,
            packages: ["base", "ams"],
            input: r"A+B \above 0pt C+D",
            expected: r"\genfrac{}{}{0pt}{}{A+B}{C+D}",
        },
        ]
    }
    // END: Generated examples

}
