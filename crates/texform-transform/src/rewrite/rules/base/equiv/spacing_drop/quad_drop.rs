//! Drop quad in math mode and collapse each text-mode quad run to one ordinary space.
//!
//! ```yaml
//! proposal: quad-drop
//! triggers:
//!   - cmd:quad
//! consumes:
//!   eliminates: cmd:quad
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: math-quad, from: \quad, to: ''}
//!   - {label: text-quad-run, from: \quad\quad, to: ' '}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_spacer;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static QUAD_DROP: QuadDropRule {
        key: Base / "quad-drop",
        level: Equiv,
        summary: "Drop quad in math mode and collapse each text-mode quad run to one ordinary space.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::QUAD],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::QUAD],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::QUAD) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\quad")?;

            Ok(if drop_spacer(cx.ast, node_id, &[base::cmd::QUAD.name]) {
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
        rule: QUAD_DROP,
        level: Equiv,
        examples: [
        {
            label: math_quad,
            packages: ["base"],
            input: r"A\quad B",
            expected: r"AB",
        },
        {
            label: text_quad_run,
            packages: ["base", "textmacros"],
            input: r"\text{A\quad\quad B}",
            expected: r"\text{A B}",
        },
        {
            label: text_quad_singleton,
            packages: ["base", "textmacros"],
            input: r"\text{A\quad B}",
            expected: r"\text{A B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: QUAD_DROP,
        level: Equiv,
        examples: [
        {
            label: argument_slots_are_independent,
            packages: ["base"],
            input: r"\frac{\quad}{\quad}",
            expected: r"\frac{}{}",
        },
        {
            label: unbraced_text_argument_separator,
            packages: ["base", "textmacros"],
            input: r"\text{A\u\quad B}",
            expected: r"\text{A\u{ }B}",
        },
        ]
    }
}
