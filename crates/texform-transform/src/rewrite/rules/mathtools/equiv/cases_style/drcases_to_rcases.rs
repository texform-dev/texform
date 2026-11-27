//! Rewrite the display-style mathtools drcases environment to mathtools rcases.
//!
//! ```yaml
//! proposal: drcases-to-rcases
//! triggers:
//!   - env:drcases
//! consumes:
//!   eliminates: env:drcases
//!   touches: null
//! produces: env:rcases
//! rewrite_patterns:
//!   - {from: '\begin{drcases} #1 \end{drcases}', to: '\begin{rcases} #1 \end{rcases}'}
//! ```

use texform_knowledge::builtin::mathtools;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static DRCASES_TO_RCASES: DrcasesToRcasesRule {
        key: Mathtools / "drcases-to-rcases",
        level: Equiv,
        summary: "Rewrite the display-style mathtools drcases environment to mathtools rcases.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::DRCASES],
        consumes: RuleConsumes {
            eliminates: env_targets![&mathtools::env::DRCASES],
            touches: &[],
        },
        produces: RuleProduces {
            targets: env_targets![&mathtools::env::RCASES],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::DRCASES) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\begin{{{}}}", env.name);
            cx.for_rule(Self::KEY).expect_no_args(env.args, &subject)?;
            let body = env.body;

            cx.ast.replace_node(
                node_id,
                Node::Environment {
                    name: mathtools::env::RCASES.name.to_string(),
                    args: Vec::new(),
                    known: true,
                    body,
                },
            );
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
        rule: DRCASES_TO_RCASES,
        level: Equiv,
        examples: [
        {
            label: plain_cells,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{drcases} x & x>0 \\ -x & x\le 0 \end{drcases}",
            expected: r"\begin{rcases} x & x>0 \\ -x & x\le 0 \end{rcases}",
        },
        {
            label: display_style_cells,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{drcases} \frac{1}{2} & \sum_{i=1}^{n} x_i>0 \\ 0 & \text{otherwise} \end{drcases}",
            expected: r"\begin{rcases} \frac{1}{2} & \sum_{i=1}^{n} x_i>0 \\ 0 & \text{otherwise} \end{rcases}",
        },
        ]
    }
    // END: Generated examples

}
