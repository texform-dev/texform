//! Rewrite the fenced mathtools bsmallmatrix environment to explicit delimiters around an AMS smallmatrix environment.
//!
//! ```yaml
//! proposal: bsmallmatrix-env-to-bracket-smallmatrix
//! triggers:
//!   - env:bsmallmatrix
//! consumes:
//!   eliminates: env:bsmallmatrix
//!   touches: null
//! produces:
//!   - env:smallmatrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{bsmallmatrix} #1 \end{bsmallmatrix}', to: '\left[\begin{smallmatrix} #1 \end{smallmatrix}\right]'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;
use texform_knowledge::builtin::mathtools;

use super::helpers::replace_with_fenced_smallmatrix_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static BSMALLMATRIX_ENV_TO_BRACKET_SMALLMATRIX: BsmallmatrixEnvToBracketSmallmatrixRule {
        key: Mathtools / "bsmallmatrix-env-to-bracket-smallmatrix",
        level: Corpus,
        summary: "Rewrite the fenced mathtools bsmallmatrix environment to explicit delimiters around an AMS smallmatrix environment.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::BSMALLMATRIX],
        consumes: RuleConsumes {
            eliminates: env_targets![&mathtools::env::BSMALLMATRIX],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::SMALLMATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::BSMALLMATRIX) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\begin{{{}}}", env.name);
            let args = env.args.to_vec();
            let body = env.body;

            cx.for_rule(Self::KEY).expect_no_args(&args, &subject)?;
            replace_with_fenced_smallmatrix_env(
                cx,
                node_id,
                body,
                Delimiter::Char('['),
                Delimiter::Char(']'),
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
        rule: BSMALLMATRIX_ENV_TO_BRACKET_SMALLMATRIX,
        level: Corpus,
        examples: [
        {
            label: standalone,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{bsmallmatrix} a & b \\ c & d \end{bsmallmatrix}",
            expected: r"\left[\begin{smallmatrix} a & b \\ c & d \end{smallmatrix}\right]",
        },
        {
            label: inline_neighbors,
            packages: ["base", "ams", "mathtools"],
            input: r"x=\begin{bsmallmatrix} 1 & 0 \\ 0 & 1 \end{bsmallmatrix} y",
            expected: r"x=\left[\begin{smallmatrix} 1 & 0 \\ 0 & 1 \end{smallmatrix}\right] y",
        },
        ]
    }
    // END: Generated examples

}
