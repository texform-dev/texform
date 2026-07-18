//! Drop continued-fraction styling from centered AMS cfrac forms.
//!
//! ```yaml
//! proposal: cfrac-to-frac
//! triggers:
//!   - cmd:cfrac
//! consumes:
//!   eliminates: null
//!   touches: cmd:cfrac
//! produces: cmd:frac
//! rewrite_patterns:
//!   - {label: default-alignment, from: '\cfrac{#1}{#2}', to: '\frac{#1}{#2}'}
//!   - {label: empty-alignment, from: '\cfrac[]{#1}{#2}', to: '\frac{#1}{#2}'}
//! ```

use texform_knowledge::builtin::{ams, base};

use crate::ast::{ArgumentKind, ArgumentValue, ContentMode};
use crate::rewrite::helpers::{mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static CFRAC_TO_FRAC: CfracToFracRule {
        key: Ams / "cfrac-to-frac",
        level: Equiv,
        summary: "Drop continued-fraction styling from centered AMS cfrac forms.",
        fidelity: Math,
        enabled_by_packages: [Ams],
        triggers: cmd_targets![&ams::cmd::CFRAC],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&ams::cmd::CFRAC],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &ams::cmd::CFRAC) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let scoped = cx.for_rule(Self::KEY);
            scoped.expect_arg_len(command.args, 3, &subject)?;
            let centered = match &command.args[0] {
                None => true,
                Some(argument) if argument.kind == ArgumentKind::Optional => {
                    match &argument.value {
                        ArgumentValue::CSName(alignment) => alignment.is_empty(),
                        _ => {
                            return Err(scoped.invalid_shape(
                                r"\cfrac alignment should be a control-sequence name",
                            ));
                        }
                    }
                }
                _ => {
                    return Err(scoped.invalid_shape(
                        r"\cfrac alignment should be an optional argument",
                    ));
                }
            };
            let numerator = scoped.mandatory_math_content(
                &command.args[1],
                &subject,
                "numerator",
            )?;
            let denominator = scoped.mandatory_math_content(
                &command.args[2],
                &subject,
                "denominator",
            )?;

            if !centered {
                return Ok(RuleEffect::Skipped);
            }

            let replacement = prefix_command_node(
                &base::cmd::FRAC,
                vec![
                    mandatory_content_slot(numerator, ContentMode::Math),
                    mandatory_content_slot(denominator, ContentMode::Math),
                ],
            );
            cx.ast
                .replace_node_drop_detached_children(node_id, replacement);
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
        rule: CFRAC_TO_FRAC,
        level: Equiv,
        examples: [
        {
            label: default_alignment,
            packages: ["base", "ams"],
            input: r"\cfrac{1}{2}",
            expected: r"\frac{1}{2}",
        },
        {
            label: empty_alignment,
            packages: ["base", "ams"],
            input: r"\cfrac[]{1}{2}",
            expected: r"\frac{1}{2}",
        },
        {
            label: left_alignment_preserved,
            packages: ["base", "ams"],
            input: r"\cfrac[l]{1}{2}",
            expected: r"\cfrac[l]{1}{2}",
        },
        {
            label: right_alignment_preserved,
            packages: ["base", "ams"],
            input: r"\cfrac[r]{1}{2}",
            expected: r"\cfrac[r]{1}{2}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: CFRAC_TO_FRAC,
        level: Equiv,
        examples: [
        {
            label: unknown_alignment_preserved,
            packages: ["base", "ams"],
            input: r"\cfrac[x]{1}{2}",
            expected: r"\cfrac[x]{1}{2}",
        },
        {
            label: grouped_empty_alignment_preserved,
            packages: ["base", "ams"],
            input: r"\cfrac[{}]{1}{2}",
            expected: r"\cfrac[{}]{1}{2}",
        },
        {
            label: rewrites_centered_child_inside_aligned_parent,
            packages: ["base", "ams"],
            input: r"\cfrac[l]{1}{1+\cfrac[]{1}{2}}",
            expected: r"\cfrac[l]{1}{1+\frac{1}{2}}",
        },
        ]
    }
}
