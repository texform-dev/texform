//! Collapse mskip to the explicit mspace command.
//!
//! The AST does not retain whether a scalar dimension was braced. This rule
//! follows MathJax's common unbraced primitive form by consuming one following
//! text separator; a braced source can therefore lose an intentional space.
//!
//! ```yaml
//! proposal: mskip-to-mspace
//! triggers:
//!   - cmd:mskip
//! consumes:
//!   eliminates: cmd:mskip
//!   touches: null
//! produces: cmd:mspace
//! rewrite_patterns:
//!   - {from: '\mskip #1', to: '\mspace{#1}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::{consume_following_text_separator, required_dimension};
use crate::rewrite::helpers::{dimension_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static MSKIP_TO_MSPACE: MskipToMspaceRule {
        key: Base / "mskip-to-mspace",
        level: Expand,
        summary: "Collapse mskip to the explicit mspace command.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::MSKIP],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::MSKIP],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::MSPACE],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::MSKIP) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            cx.for_rule(Self::KEY)
                .expect_arg_len(command.args, 1, &subject)?;
            let dimension = required_dimension(Self::KEY, cx, &command.args[0], &subject)?;

            cx.ast.replace_node(
                node_id,
                prefix_command_node(&base::cmd::MSPACE, vec![dimension_slot(dimension)]),
            );
            consume_following_text_separator(cx.ast, node_id);
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
        rule: MSKIP_TO_MSPACE,
        level: Expand,
        examples: [
        {
            label: mskip_between_text_words,
            packages: ["base", "textmacros"],
            input: r"\text{Left\mskip 1em Right}",
            expected: r"\text{Left\mspace{1em}Right}",
        },
        {
            label: mskip_mu_length,
            packages: ["base", "textmacros"],
            input: r"X\mskip 3mu Y",
            expected: r"X\mspace{3mu} Y",
        },
        {
            label: text_mskip_braced_space_loss,
            packages: ["base", "textmacros"],
            input: r"\text{A\mskip{1em} B}",
            expected: r"\text{A\mspace{1em}B}",
        },
        ]
    }
    // END: Generated examples
}
