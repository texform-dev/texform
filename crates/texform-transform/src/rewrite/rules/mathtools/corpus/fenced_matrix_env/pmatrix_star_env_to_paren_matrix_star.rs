//! Rewrite the fenced mathtools pmatrix* environment to explicit delimiters around a core matrix* environment.
//!
//! ```yaml
//! proposal: pmatrix-star-env-to-paren-matrix-star
//! triggers:
//!   - env:pmatrix*
//! consumes:
//!   eliminates: env:pmatrix*
//!   touches: null
//! produces:
//!   - env:matrix*
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: no-alignment, from: '\begin{pmatrix*} #1 \end{pmatrix*}', to: '\left(\begin{matrix*} #1 \end{matrix*}\right)'}
//!   - {label: alignment, from: '\begin{pmatrix*}[#1] #2 \end{pmatrix*}', to: '\left(\begin{matrix*}[#1] #2 \end{matrix*}\right)'}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::mathtools;

use super::helpers::replace_with_fenced_matrix_star_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static PMATRIX_STAR_ENV_TO_PAREN_MATRIX_STAR: PmatrixStarEnvToParenMatrixStarRule {
        key: Mathtools / "pmatrix-star-env-to-paren-matrix-star",
        level: Corpus,
        summary: "Rewrite the fenced mathtools pmatrix* environment to explicit delimiters around a core matrix* environment.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::PMATRIX_STAR],
        consumes: RuleConsumes {
            eliminates: env_targets![&mathtools::env::PMATRIX_STAR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&mathtools::env::MATRIX_STAR), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::PMATRIX_STAR) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\begin{{{}}}", env.name);
            let args = env.args.to_vec();
            let body = env.body;

            // The optional column alignment is forwarded unchanged; a redundant
            // `[c]` is left to `mathtools/matrix-star-default-align-to-matrix`.
            cx.for_rule(Self::KEY).expect_arg_len(&args, 1, &subject)?;
            replace_with_fenced_matrix_star_env(
                cx,
                node_id,
                args,
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
        rule: PMATRIX_STAR_ENV_TO_PAREN_MATRIX_STAR,
        level: Corpus,
        examples: [
        {
            label: no_alignment,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{pmatrix*} a & b \\ c & d \end{pmatrix*}",
            expected: r"\left(\begin{matrix*} a & b \\ c & d \end{matrix*}\right)",
        },
        {
            label: alignment,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{pmatrix*}[r] -1 & 2 \\ 3 & -4 \end{pmatrix*}",
            expected: r"\left(\begin{matrix*}[r] -1 & 2 \\ 3 & -4 \end{matrix*}\right)",
        },
        ]
    }
    // END: Generated examples

}
