//! Rewrite displaylines to the standard gather environment.
//!
//! ```yaml
//! proposal: displaylines-to-gather-env
//! consumes:
//!   eliminates: [cmd:displaylines, cmd:cr]
//!   touches: null
//! produces:
//!   - env:gather
//!   - cmd:notag
//! rewrite_patterns:
//!   - {from: '\displaylines{#1 \cr #2}', to: '\begin{gather} #1 \notag \\ #2 \notag \end{gather}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{
    cr_rows, mandatory_math_body, linebreak_command, notag_command, replace_with_environment,
};
use crate::transform::rule::{RuleConsumes, RuleProduces, RuleTarget};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite displaylines to the standard gather environment.
    pub static DISPLAYLINES_TO_GATHER_ENV: DisplaylinesToGatherEnvRule {
        key: Base / "displaylines-to-gather-env",
        tier: Base,
        summary: "Rewrite displaylines to the standard gather environment.",
        phase: Normalize,
        safety: Semantic,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::DISPLAYLINES, &base::cmd::CR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::GATHER), RuleTarget::Command(&ams::cmd::NOTAG)],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::DISPLAYLINES) else {
                return Ok(crate::transform::rule::RuleEffect::Skipped);
            };
            cx.expect_arg_len(rule.meta().key, command.args, 1, r"\displaylines")?;
            let body = mandatory_math_body(
                rule.meta().key,
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
            Ok(crate::transform::rule::RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DISPLAYLINES_TO_GATHER_ENV,
        tier: Base,
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
