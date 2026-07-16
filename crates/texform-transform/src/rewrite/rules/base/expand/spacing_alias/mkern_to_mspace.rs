//! Collapse mkern to the explicit mspace command.
//!
//! In base math mode, this rewrite drops MathJax's `nobreak` flag from
//! `\mkern`. The AST also loses whether a scalar dimension was braced, so
//! consuming the unbraced text separator can drop a space after a braced source.
//!
//! ```yaml
//! proposal: mkern-to-mspace
//! triggers:
//!   - cmd:mkern
//! consumes:
//!   eliminates: cmd:mkern
//!   touches: null
//! produces: cmd:mspace
//! rewrite_patterns:
//!   - {from: '\mkern #1', to: '\mspace{#1}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::{consume_following_text_separator, required_dimension};
use crate::rewrite::helpers::{dimension_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static MKERN_TO_MSPACE: MkernToMspaceRule {
        key: Base / "mkern-to-mspace",
        level: Expand,
        summary: "Collapse mkern to the explicit mspace command.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::MKERN],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::MKERN],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::MSPACE],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::MKERN) else {
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
        rule: MKERN_TO_MSPACE,
        level: Expand,
        examples: [
        {
            label: mkern_between_text_glyphs,
            packages: ["base", "textmacros"],
            input: r"\text{A\mkern 1em B}",
            expected: r"\text{A\mspace{1em}B}",
        },
        {
            label: mkern_mu_length,
            packages: ["base", "textmacros"],
            input: r"A\mkern 2mu B",
            expected: r"A\mspace{2mu} B",
        },
        {
            label: text_mkern_braced_space_loss,
            packages: ["base", "textmacros"],
            input: r"\text{A\mkern{1em} B}",
            expected: r"\text{A\mspace{1em}B}",
        },
        ]
    }
    // END: Generated examples
}
