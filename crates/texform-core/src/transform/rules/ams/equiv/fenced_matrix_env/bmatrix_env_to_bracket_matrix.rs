//! Rewrite the fenced bmatrix environment to explicit brackets around a core matrix environment.
//!
//! ```yaml
//! proposal: bmatrix-env-to-bracket-matrix
//! triggers:
//!   - env:bmatrix
//! consumes:
//!   eliminates: env:bmatrix
//!   touches: null
//! produces:
//!   - env:matrix
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\begin{bmatrix} #1 \end{bmatrix}', to: '\left[\begin{matrix} #1 \end{matrix}\right]'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::replace_with_fenced_matrix_env;
use crate::ast::Delimiter;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::transform::{define_rule, env_targets};

define_rule! {
    pub static BMATRIX_ENV_TO_BRACKET_MATRIX: BmatrixEnvToBracketMatrixRule {
        key: Ams / "bmatrix-env-to-bracket-matrix",
        class: Equiv,
        summary: "Rewrite the fenced bmatrix environment to explicit brackets around a core matrix environment.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: env_targets![&ams::env::BMATRIX],
        consumes: RuleConsumes {
            eliminates: env_targets![&ams::env::BMATRIX],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::MATRIX), RuleTarget::Command(&base::cmd::LEFT), RuleTarget::Command(&base::cmd::RIGHT)],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &ams::env::BMATRIX) else {
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
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BMATRIX_ENV_TO_BRACKET_MATRIX,
        class: Equiv,
        examples: [
        {
            label: bmatrix,
            packages: ["base", "ams"],
            input: r"\begin{bmatrix} a & b \\ c & d \end{bmatrix}",
            expected: r"\left[\begin{matrix} a & b \\ c & d \end{matrix}\right]",
        },
        ]
    }
    // END: Generated examples
}
