//! Rewrite the fenced vmatrix environment to explicit vertical bars around a core matrix environment.
//!
//! ```yaml
//! proposal: vmatrix-env-to-vert-matrix
//! triggers:
//!   - env:vmatrix
//! consumes:
//!   eliminates: env:vmatrix
//!   touches: null
//! produces:
//!   - env:matrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{vmatrix} #1 \end{vmatrix}', to: '\left|\begin{matrix} #1 \end{matrix}\right|'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::replace_with_fenced_matrix_env;
use crate::ast::Delimiter;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static VMATRIX_ENV_TO_VERT_MATRIX: VmatrixEnvToVertMatrixRule {
        key: Ams / "vmatrix-env-to-vert-matrix",
        level: Equiv,
        summary: "Rewrite the fenced vmatrix environment to explicit vertical bars around a core matrix environment.",
        fidelity: Full,
        enabled_by_packages: [Ams],
        triggers: env_targets![&ams::env::VMATRIX],
        consumes: RuleConsumes {
            eliminates: env_targets![&ams::env::VMATRIX],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::MATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &ams::env::VMATRIX) else {
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
                Delimiter::Char('|'),
                Delimiter::Char('|'),
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
        level: Equiv,
        examples: [
        {
            label: vmatrix,
            packages: ["base", "ams"],
            input: r"\begin{vmatrix} a & b \\ c & d \end{vmatrix}",
            expected: r"\left|\begin{matrix} a & b \\ c & d \end{matrix}\right|",
        },
        ]
    }
    // END: Generated examples
}
