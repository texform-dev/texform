//! Drop qquad in math mode and collapse each text-mode qquad run to one ordinary space.
//!
//! ```yaml
//! proposal: qquad-drop
//! triggers:
//!   - cmd:qquad
//! consumes:
//!   eliminates: cmd:qquad
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: math-qquad, from: \qquad, to: ''}
//!   - {label: text-qquad-run, from: \qquad\qquad, to: ' '}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_spacer;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static QQUAD_DROP: QquadDropRule {
        key: Base / "qquad-drop",
        level: Equiv,
        summary: "Drop qquad in math mode and collapse each text-mode qquad run to one ordinary space.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::QQUAD],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::QQUAD],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::QQUAD) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\qquad")?;

            Ok(if drop_spacer(cx.ast, node_id, &[base::cmd::QQUAD.name]) {
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
        rule: QQUAD_DROP,
        level: Equiv,
        examples: [
        {
            label: math_qquad,
            packages: ["base"],
            input: r"A\qquad B",
            expected: r"AB",
        },
        {
            label: text_qquad_run,
            packages: ["base", "textmacros"],
            input: r"\text{A\qquad\qquad B}",
            expected: r"\text{A B}",
        },
        {
            label: text_qquad_singleton,
            packages: ["base", "textmacros"],
            input: r"\text{A\qquad B}",
            expected: r"\text{A B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: QQUAD_DROP,
        level: Equiv,
        examples: [
        {
            label: math_script_base,
            packages: ["base"],
            input: r"\qquad^2",
            expected: r"{}^2",
        },
        {
            label: unbraced_text_argument_separator,
            packages: ["base", "textmacros"],
            input: r"\text{A\u\qquad B}",
            expected: r"\text{A\u{ }B}",
        },
        ]
    }
}
