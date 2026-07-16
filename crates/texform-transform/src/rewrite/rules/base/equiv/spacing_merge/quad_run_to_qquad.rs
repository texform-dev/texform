//! Merge adjacent quad pairs into qquad for equivalence normalization.
//!
//! ```yaml
//! proposal: quad-run-to-qquad
//! triggers:
//!   - cmd:quad
//! consumes:
//!   eliminates: null
//!   touches: cmd:quad
//! produces: cmd:qquad
//! rewrite_patterns:
//!   - {from: \quad\quad, to: \qquad}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static QUAD_RUN_TO_QQUAD: QuadRunToQquadRule {
        key: Base / "quad-run-to-qquad",
        level: Equiv,
        summary: "Merge adjacent quad pairs into qquad for equivalence normalization.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::QUAD],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::QUAD],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::QQUAD],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::QUAD) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\quad")?;

            let Some(next) = cx.ast.next_sibling(node_id) else {
                return Ok(RuleEffect::Skipped);
            };
            let Some(next_command) = cx.match_command(next, &base::cmd::QUAD) else {
                return Ok(RuleEffect::Skipped);
            };
            if !next_command.args.is_empty() {
                return Ok(RuleEffect::Skipped);
            }

            cx.ast.replace_node(node_id, bare_command_node(base::cmd::QQUAD.name));
            cx.ast.remove_node(next);
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
        rule: QUAD_RUN_TO_QQUAD,
        level: Equiv,
        examples: [
        {
            label: quad_pair,
            packages: ["base"],
            input: r"A\quad\quad B",
            expected: r"A\qquad B",
        },
        {
            label: text_quad_pair,
            packages: ["base", "textmacros"],
            input: r"\text{A\quad\quad B}",
            expected: r"\text{A\qquad B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: QUAD_RUN_TO_QQUAD,
        level: Equiv,
        examples: [
        {
            label: greedy_triple_leaves_trailing_quad,
            packages: ["base"],
            input: r"A\quad\quad\quad B",
            expected: r"A\qquad\quad B",
        },
        {
            label: singleton_is_preserved,
            packages: ["base"],
            input: r"A\quad B",
            expected: r"A\quad B",
        },
        {
            label: explicit_group_boundary_is_preserved,
            packages: ["base"],
            input: r"A\quad{\quad}B",
            expected: r"A\quad{\quad}B",
        },
        {
            label: merges_inside_script_group,
            packages: ["base"],
            input: r"x^{\quad\quad}",
            expected: r"x^{\qquad}",
        },
        {
            label: argument_slots_do_not_share_siblings,
            packages: ["base"],
            input: r"\frac{\quad}{\quad}",
            expected: r"\frac{\quad}{\quad}",
        },
        ]
    }
}
