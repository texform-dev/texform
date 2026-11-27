//! Rewrite the fenced mathtools Vsmallmatrix environment to explicit delimiters around an AMS smallmatrix environment.
//!
//! ```yaml
//! proposal: Vsmallmatrix-env-to-Vert-smallmatrix
//! triggers:
//!   - env:Vsmallmatrix
//! consumes:
//!   eliminates: env:Vsmallmatrix
//!   touches: null
//! produces:
//!   - env:smallmatrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{Vsmallmatrix} #1 \end{Vsmallmatrix}', to: '\left\Vert\begin{smallmatrix} #1 \end{smallmatrix}\right\Vert'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;
use texform_knowledge::builtin::mathtools;

use super::helpers::replace_with_fenced_smallmatrix_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static VSMALLMATRIX_ENV_TO_VERT_SMALLMATRIX: VsmallmatrixEnvToVertSmallmatrixRule {
        key: Mathtools / "Vsmallmatrix-env-to-Vert-smallmatrix",
        level: Corpus,
        summary: "Rewrite the fenced mathtools Vsmallmatrix environment to explicit delimiters around an AMS smallmatrix environment.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::VSMALLMATRIX_2],
        consumes: RuleConsumes {
            eliminates: env_targets![&mathtools::env::VSMALLMATRIX_2],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::SMALLMATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::VSMALLMATRIX_2) else {
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
                Delimiter::Control("Vert".to_string()),
                Delimiter::Control("Vert".to_string()),
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
        rule: VSMALLMATRIX_ENV_TO_VERT_SMALLMATRIX,
        level: Corpus,
        examples: [
        {
            label: standalone,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{Vsmallmatrix} a & b \\ c & d \end{Vsmallmatrix}",
            expected: r"\left\Vert\begin{smallmatrix} a & b \\ c & d \end{smallmatrix}\right\Vert",
        },
        {
            label: inline_neighbors,
            packages: ["base", "ams", "mathtools"],
            input: r"x=\begin{Vsmallmatrix} 1 & 0 \\ 0 & 1 \end{Vsmallmatrix} y",
            expected: r"x=\left\Vert\begin{smallmatrix} 1 & 0 \\ 0 & 1 \end{smallmatrix}\right\Vert y",
        },
        ]
    }
    // END: Generated examples

}
