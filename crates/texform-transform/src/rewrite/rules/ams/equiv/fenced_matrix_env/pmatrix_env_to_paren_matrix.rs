//! Rewrite the fenced pmatrix environment to explicit parens around a core matrix environment.
//!
//! ```yaml
//! proposal: pmatrix-env-to-paren-matrix
//! triggers:
//!   - env:pmatrix
//! consumes:
//!   eliminates: env:pmatrix
//!   touches: null
//! produces:
//!   - env:matrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{pmatrix} #1 \end{pmatrix}', to: '\left(\begin{matrix} #1 \end{matrix}\right)'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::replace_with_fenced_matrix_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static PMATRIX_ENV_TO_PAREN_MATRIX: PmatrixEnvToParenMatrixRule {
        key: Ams / "pmatrix-env-to-paren-matrix",
        level: Equiv,
        summary: "Rewrite the fenced pmatrix environment to explicit parens around a core matrix environment.",
        fidelity: Full,
        enabled_by_packages: [Ams],
        triggers: env_targets![&ams::env::PMATRIX],
        consumes: RuleConsumes {
            eliminates: env_targets![&ams::env::PMATRIX],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::MATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &ams::env::PMATRIX) else {
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
        rule: PMATRIX_ENV_TO_PAREN_MATRIX,
        level: Equiv,
        examples: [
        {
            label: pmatrix,
            packages: ["base", "ams"],
            input: r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
            expected: r"\left(\begin{matrix} a & b \\ c & d \end{matrix}\right)",
        },
        {
            label: pmatrix_env_ams_only,
            packages: ["base", "ams"],
            input: r"\begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}",
            expected: r"\left(\begin{matrix} 1 & 0 \\ 0 & 1 \end{matrix}\right)",
        },
        {
            label: plain_pmatrix_command_out_of_scope,
            packages: ["base", "ams"],
            input: r"\pmatrix{x \cr y}",
            expected: r"\pmatrix{x \cr y}",
        },
        ]
    }
    // END: Generated examples
}
