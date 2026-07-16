//! Collapse hskip to the explicit hspace command.
//!
//! The AST does not retain whether a scalar dimension was braced. This rule
//! follows MathJax's common unbraced primitive form by consuming one following
//! text separator; a braced source can therefore lose an intentional space.
//!
//! ```yaml
//! proposal: hskip-to-hspace
//! triggers:
//!   - cmd:hskip
//! consumes:
//!   eliminates: cmd:hskip
//!   touches: null
//! produces: cmd:hspace
//! rewrite_patterns:
//!   - {from: '\hskip #1', to: '\hspace{#1}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::consume_following_text_separator;
use crate::ast::{ArgumentKind, ArgumentValue};
use crate::rewrite::helpers::{dimension_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static HSKIP_TO_HSPACE: HskipToHspaceRule {
        key: Base / "hskip-to-hspace",
        level: Standard,
        summary: "Collapse hskip to the explicit hspace command.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::HSKIP],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::HSKIP],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::HSPACE],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::HSKIP) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            cx.for_rule(Self::KEY)
                .expect_arg_len(command.args, 1, &subject)?;
            let dimension = match &command.args[0] {
                Some(arg) if arg.kind == ArgumentKind::Mandatory => match &arg.value {
                    ArgumentValue::Dimension(value) => value.clone(),
                    _ => return Err(cx.for_rule(Self::KEY).invalid_shape(format!(
                        "{subject} argument should carry a dimension"
                    ))),
                },
                _ => return Err(cx.for_rule(Self::KEY).invalid_shape(format!(
                    "{subject} should carry a mandatory dimension argument"
                ))),
            };

            cx.ast.replace_node(
                node_id,
                prefix_command_node(&base::cmd::HSPACE, vec![dimension_slot(dimension)]),
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
        rule: HSKIP_TO_HSPACE,
        level: Standard,
        examples: [
        {
            label: hskip_between_symbols,
            packages: ["base"],
            input: r"A \hskip 1em B",
            expected: r"A \hspace{1em} B",
        },
        {
            label: hskip_decimal_em_length,
            packages: ["base"],
            input: r"u\hskip 0.25em v",
            expected: r"u\hspace{0.25em} v",
        },
        {
            label: text_hskip_between_glyphs,
            packages: ["base", "textmacros"],
            input: r"\text{A\hskip 1em B}",
            expected: r"\text{A\hspace{1em}B}",
        },
        {
            label: text_hskip_braced_space_loss,
            packages: ["base", "textmacros"],
            input: r"\text{A\hskip{1em} B}",
            expected: r"\text{A\hspace{1em}B}",
        },
        ]
    }
    // END: Generated examples
}
