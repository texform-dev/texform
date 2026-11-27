//! Rewrite the fenced mathtools Vmatrix* environment to explicit delimiters around a core matrix* environment.
//!
//! ```yaml
//! proposal: Vmatrix-star-env-to-Vert-matrix-star
//! triggers:
//!   - env:Vmatrix*
//! consumes:
//!   eliminates: env:Vmatrix*
//!   touches: null
//! produces:
//!   - env:matrix*
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: no-alignment, from: '\begin{Vmatrix*} #1 \end{Vmatrix*}', to: '\left\Vert\begin{matrix*} #1 \end{matrix*}\right\Vert'}
//!   - {label: alignment, from: '\begin{Vmatrix*}[#1] #2 \end{Vmatrix*}', to: '\left\Vert\begin{matrix*}[#1] #2 \end{matrix*}\right\Vert'}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::mathtools;

use super::helpers::replace_with_fenced_matrix_star_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static VMATRIX_STAR_ENV_TO_VERT_MATRIX_STAR: VmatrixStarEnvToVertMatrixStarRule {
        key: Mathtools / "Vmatrix-star-env-to-Vert-matrix-star",
        level: Corpus,
        summary: "Rewrite the fenced mathtools Vmatrix* environment to explicit delimiters around a core matrix* environment.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::VMATRIX_STAR_2],
        consumes: RuleConsumes {
            eliminates: env_targets![&mathtools::env::VMATRIX_STAR_2],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&mathtools::env::MATRIX_STAR), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::VMATRIX_STAR_2) else {
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
        rule: VMATRIX_STAR_ENV_TO_VERT_MATRIX_STAR,
        level: Corpus,
        examples: [
        {
            label: no_alignment,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{Vmatrix*} a & b \\ c & d \end{Vmatrix*}",
            expected: r"\left\Vert\begin{matrix*} a & b \\ c & d \end{matrix*}\right\Vert",
        },
        {
            label: alignment,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{Vmatrix*}[r] -1 & 2 \\ 3 & -4 \end{Vmatrix*}",
            expected: r"\left\Vert\begin{matrix*}[r] -1 & 2 \\ 3 & -4 \end{matrix*}\right\Vert",
        },
        ]
    }
    // END: Generated examples

}
