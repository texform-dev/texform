//! Rewrite the fenced mathtools psmallmatrix environment to explicit delimiters around an AMS smallmatrix environment.
//!
//! ```yaml
//! proposal: psmallmatrix-env-to-paren-smallmatrix
//! triggers:
//!   - env:psmallmatrix
//! consumes:
//!   eliminates: env:psmallmatrix
//!   touches: null
//! produces:
//!   - env:smallmatrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{psmallmatrix} #1 \end{psmallmatrix}', to: '\left(\begin{smallmatrix} #1 \end{smallmatrix}\right)'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;
use texform_knowledge::builtin::mathtools;

use super::helpers::replace_with_fenced_smallmatrix_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static PSMALLMATRIX_ENV_TO_PAREN_SMALLMATRIX: PsmallmatrixEnvToParenSmallmatrixRule {
        key: Mathtools / "psmallmatrix-env-to-paren-smallmatrix",
        level: Corpus,
        summary: "Rewrite the fenced mathtools psmallmatrix environment to explicit delimiters around an AMS smallmatrix environment.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::PSMALLMATRIX],
        consumes: RuleConsumes {
            eliminates: env_targets![&mathtools::env::PSMALLMATRIX],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::SMALLMATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::PSMALLMATRIX) else {
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
                Delimiter::Char('('),
                Delimiter::Char(')'),
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
        rule: PSMALLMATRIX_ENV_TO_PAREN_SMALLMATRIX,
        level: Corpus,
        examples: [
        {
            label: standalone,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{psmallmatrix} a & b \\ c & d \end{psmallmatrix}",
            expected: r"\left(\begin{smallmatrix} a & b \\ c & d \end{smallmatrix}\right)",
        },
        {
            label: inline_neighbors,
            packages: ["base", "ams", "mathtools"],
            input: r"x=\begin{psmallmatrix} 1 & 0 \\ 0 & 1 \end{psmallmatrix} y",
            expected: r"x=\left(\begin{smallmatrix} 1 & 0 \\ 0 & 1 \end{smallmatrix}\right) y",
        },
        ]
    }
    // END: Generated examples

}
