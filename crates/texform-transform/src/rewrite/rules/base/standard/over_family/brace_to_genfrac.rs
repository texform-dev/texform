//! Rewrite infix brace to an explicit genfrac with brace delimiters.
//!
//! ```yaml
//! proposal: brace-to-genfrac
//! triggers:
//!   - cmd:brace
//! consumes:
//!   eliminates: cmd:brace
//!   touches: null
//! produces: cmd:genfrac
//! rewrite_patterns:
//!   - {from: '#1 \brace #2', to: '\genfrac{\{}{\}}{0pt}{}{#1}{#2}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::{genfrac_args, replace_infix_with_command};
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BRACE_TO_GENFRAC: BraceToGenfracRule {
        key: Base / "brace-to-genfrac",
        level: Standard,
        summary: "Rewrite infix brace to an explicit genfrac with brace delimiters.",
        fidelity: Lossless,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::BRACE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BRACE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::GENFRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::BRACE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(infix.args, r"\brace")?;

            replace_infix_with_command(
                cx,
                node_id,
                &ams::cmd::GENFRAC,
                genfrac_args(
                    Delimiter::Control("{".to_string()),
                    Delimiter::Control("}".to_string()),
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
        rule: BRACE_TO_GENFRAC,
        level: Standard,
        examples: [
        {
            label: brace_delimited_stack,
            packages: ["base", "ams"],
            input: r"x_1+\cdots+x_m \brace y_1+\cdots+y_n",
            expected: r"\genfrac{\{}{\}}{0pt}{}{x_1+\cdots+x_m}{y_1+\cdots+y_n}",
        },
        ]
    }
    // END: Generated examples

}
