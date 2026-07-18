//! Drop canonical small spacers in math mode and collapse each text-mode run to one ordinary space.
//!
//! ```yaml
//! proposal: small-spacer-drop
//! triggers:
//!   - 'cmd:,'
//!   - 'cmd::'
//!   - cmd:;
//!   - cmd:!
//! consumes:
//!   eliminates: ['cmd:,', 'cmd::', cmd:;, cmd:!]
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: math-comma-space, from: '\,', to: ''}
//!   - {label: math-colon-space, from: '\:', to: ''}
//!   - {label: math-semicolon-space, from: \;, to: ''}
//!   - {label: math-neg-comma-space, from: \!, to: ''}
//!   - {label: text-small-spacer-run, from: '\,\;\!', to: ' '}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_spacer;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static SMALL_SPACER_DROP: SmallSpacerDropRule {
        key: Base / "small-spacer-drop",
        level: Equiv,
        summary: "Drop canonical small spacers in math mode and collapse each text-mode run to one ordinary space.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::_COMMA, &base::cmd::_COLON, &base::cmd::_SEMICOLON, &base::cmd::_EXCLAMATION],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::_COMMA, &base::cmd::_COLON, &base::cmd::_SEMICOLON, &base::cmd::_EXCLAMATION],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.active_command(node_id) else {
                return Ok(RuleEffect::Skipped);
            };
            let spacer_names = [
                base::cmd::_COMMA.name,
                base::cmd::_COLON.name,
                base::cmd::_SEMICOLON.name,
                base::cmd::_EXCLAMATION.name,
            ];
            if !spacer_names.contains(&command.name) {
                return Ok(RuleEffect::Skipped);
            }
            let subject = format!(r"\{}", command.name);
            cx.for_rule(Self::KEY).expect_no_args(cx.ast.arg_slots(node_id), &subject)?;

            Ok(if drop_spacer(cx.ast, node_id, &spacer_names) {
                RuleEffect::Applied
            } else {
                RuleEffect::Skipped
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SMALL_SPACER_DROP,
        level: Equiv,
        examples: [
        {
            label: math_comma_space,
            packages: ["base"],
            input: r"A\,B",
            expected: r"AB",
        },
        {
            label: math_colon_space,
            packages: ["base"],
            input: r"A\:B",
            expected: r"AB",
        },
        {
            label: math_semicolon_space,
            packages: ["base"],
            input: r"A\;B",
            expected: r"AB",
        },
        {
            label: math_neg_comma_space,
            packages: ["base"],
            input: r"A\!B",
            expected: r"AB",
        },
        {
            label: text_mixed_run,
            packages: ["base", "textmacros"],
            input: r"\text{A\,\:\;\!B}",
            expected: r"\text{A B}",
        },
        {
            label: text_singleton_run,
            packages: ["base", "textmacros"],
            input: r"\text{A\,B}",
            expected: r"\text{A B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: SMALL_SPACER_DROP,
        level: Equiv,
        examples: [
        {
            label: math_script_base,
            packages: ["base"],
            input: r"\!^2",
            expected: r"{}^2",
        },
        {
            label: text_nodes_split_runs,
            packages: ["base", "textmacros"],
            input: r"\text{A\,X\;B}",
            expected: r"\text{A X B}",
        },
        ]
    }
}
