//! Rewrite the fenced Vmatrix environment to explicit double bars around a core matrix environment.
//!
//! ```yaml
//! proposal: Vmatrix-env-to-Vert-matrix
//! triggers:
//!   - env:Vmatrix
//! consumes:
//!   eliminates: env:Vmatrix
//!   touches: null
//! produces:
//!   - env:matrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{Vmatrix} #1 \end{Vmatrix}', to: '\left\Vert\begin{matrix} #1 \end{matrix}\right\Vert'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::replace_with_fenced_matrix_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static VMATRIX_ENV_TO_VERT_MATRIX: VmatrixEnvToVertMatrixRule {
        key: Ams / "Vmatrix-env-to-Vert-matrix",
        level: Corpus,
        summary: "Rewrite the fenced Vmatrix environment to explicit double bars around a core matrix environment.",
        fidelity: Render,
        enabled_by_packages: [Ams],
        triggers: env_targets![&ams::env::VMATRIX_2],
        consumes: RuleConsumes {
            eliminates: env_targets![&ams::env::VMATRIX_2],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::MATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &ams::env::VMATRIX_2) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\begin{{{}}}", env.name);
            let args = env.args.to_vec();
            let body = env.body;

            cx.for_rule(Self::KEY).expect_no_args(&args, &subject)?;
            replace_with_fenced_matrix_env(
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
        rule: VMATRIX_ENV_TO_VERT_MATRIX,
        level: Corpus,
        examples: [
        {
            label: vmatrix,
            packages: ["base", "ams"],
            input: r"\begin{Vmatrix} a & b \\ c & d \end{Vmatrix}",
            expected: r"\left\Vert\begin{matrix} a & b \\ c & d \end{matrix}\right\Vert",
        },
        ]
    }
    // END: Generated examples
}
