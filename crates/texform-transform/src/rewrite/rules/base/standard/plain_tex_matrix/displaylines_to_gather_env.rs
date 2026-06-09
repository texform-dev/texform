//! Rewrite displaylines to the standard gather environment.
//!
//! This rule declares `fidelity: Semantic` because fidelity is the worst-case
//! guarantee over the declared input domain. Bodies that use manual layout
//! commands such as `\hfill` or `\llap` can make MathJax reflow or overlap
//! hand-written equation numbers after the rewrite to `gather`. Ordinary
//! samples without those constructs are usually only spacing-level different
//! and closer to `Approximate`, but `Faithful` must not run a rewrite below its
//! `Approximate` floor, so this rule starts at `Drop`.
//!
//! ```yaml
//! proposal: displaylines-to-gather-env
//! triggers:
//!   - cmd:displaylines
//! consumes:
//!   eliminates: cmd:displaylines
//!   touches: cmd:cr
//! produces:
//!   - env:gather
//!   - cmd:notag
//! rewrite_patterns:
//!   - {from: '\displaylines{#1 \cr #2}', to: '\begin{gather} #1 \notag \\ #2 \notag \end{gather}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::{
    cr_rows, mandatory_math_body, linebreak_command, notag_command, replace_with_environment,
};
use crate::rewrite::rule::{RuleConsumes, RuleProduces, RuleTarget};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static DISPLAYLINES_TO_GATHER_ENV: DisplaylinesToGatherEnvRule {
        key: Base / "displaylines-to-gather-env",
        level: Drop,
        summary: "Rewrite displaylines to the standard gather environment.",
        fidelity: Semantic,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::DISPLAYLINES],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::DISPLAYLINES],
            touches: cmd_targets![&base::cmd::CR],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::GATHER), RuleTarget::Command(&ams::cmd::NOTAG)],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::DISPLAYLINES) else {
                return Ok(crate::rewrite::rule::RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 1, r"\displaylines")?;
            let body = mandatory_math_body(
                Self::KEY,
                cx,
                &command.args[0],
                base::cmd::DISPLAYLINES.name,
            )?;
            let rows = cr_rows(cx, body);
            let row_count = rows.len();
            let mut children = Vec::new();

            for (index, row) in rows.into_iter().enumerate() {
                children.extend(row);
                children.push(cx.ast.new_node(notag_command()));
                if index + 1 < row_count {
                    children.push(cx.ast.new_node(linebreak_command()));
                }
            }

            replace_with_environment(cx, node_id, &ams::env::GATHER, Vec::new(), children);
            Ok(crate::rewrite::rule::RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DISPLAYLINES_TO_GATHER_ENV,
        level: Drop,
        examples: [
        {
            label: multi_line_derivation,
            packages: ["base", "ams"],
            input: r"\displaylines{S_n=\sum_{k=1}^{n} k \cr =\frac{n(n+1)}{2} \cr =\Theta(n^2)}",
            expected: r"\begin{gather} S_n=\sum_{k=1}^{n} k \notag \\ =\frac{n(n+1)}{2} \notag \\ =\Theta(n^2) \notag \end{gather}",
        },
        ]
    }
    // END: Generated examples
}
