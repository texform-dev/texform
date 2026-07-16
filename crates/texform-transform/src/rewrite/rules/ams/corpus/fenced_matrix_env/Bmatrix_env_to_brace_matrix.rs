//! Rewrite the fenced Bmatrix environment to explicit braces around a core matrix environment.
//!
//! ```yaml
//! proposal: Bmatrix-env-to-brace-matrix
//! triggers:
//!   - env:Bmatrix
//! consumes:
//!   eliminates: env:Bmatrix
//!   touches: null
//! produces:
//!   - env:matrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{Bmatrix} #1 \end{Bmatrix}', to: '\left\{\begin{matrix} #1 \end{matrix}\right\}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::replace_with_fenced_matrix_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static BMATRIX_ENV_TO_BRACE_MATRIX: BmatrixEnvToBraceMatrixRule {
        key: Ams / "Bmatrix-env-to-brace-matrix",
        level: Corpus,
        summary: "Rewrite the fenced Bmatrix environment to explicit braces around a core matrix environment.",
        fidelity: Render,
        enabled_by_packages: [Ams],
        triggers: env_targets![&ams::env::BMATRIX_2],
        consumes: RuleConsumes {
            eliminates: env_targets![&ams::env::BMATRIX_2],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::MATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &ams::env::BMATRIX_2) else {
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
                Delimiter::Control("{".to_string()),
                Delimiter::Control("}".to_string()),
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
        rule: BMATRIX_ENV_TO_BRACE_MATRIX,
        level: Corpus,
        examples: [
        {
            label: bmatrix,
            packages: ["base", "ams"],
            input: r"\begin{Bmatrix} a & b \\ c & d \end{Bmatrix}",
            expected: r"\left\{\begin{matrix} a & b \\ c & d \end{matrix}\right\}",
        },
        ]
    }
    // END: Generated examples
}
