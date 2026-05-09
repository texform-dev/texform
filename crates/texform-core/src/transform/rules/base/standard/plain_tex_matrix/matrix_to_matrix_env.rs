//! Rewrite plain-TeX matrix to the standard matrix environment with row breaks.
//!
//! ```yaml
//! proposal: matrix-to-matrix-env
//! triggers:
//!   - cmd:matrix
//! consumes:
//!   eliminates: [cmd:matrix, cmd:cr]
//!   touches: null
//! produces: env:matrix
//! rewrite_patterns:
//!   - {from: '\matrix{#1 \cr #2}', to: '\begin{matrix} #1 \\ #2 \end{matrix}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::rewrite_cr_body_to_environment;
use crate::transform::rule::{RuleConsumes, RuleProduces};
use crate::transform::{cmd_targets, define_rule, env_targets};

define_rule! {
    /// Rewrite plain-TeX matrix to the standard matrix environment with row breaks.
    pub static MATRIX_TO_MATRIX_ENV: MatrixToMatrixEnvRule {
        key: Base / "matrix-to-matrix-env",
        class: Standard,
        summary: "Rewrite plain-TeX matrix to the standard matrix environment with row breaks.",
        phase: Normalize,
        safety: Semantic,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::MATRIX],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::MATRIX, &base::cmd::CR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::MATRIX],
        },
        apply(rule, cx, node_id) {
            rewrite_cr_body_to_environment(
                rule.meta().key,
                cx,
                node_id,
                &base::cmd::MATRIX,
                &ams::env::MATRIX,
                Vec::new(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: MATRIX_TO_MATRIX_ENV,
        class: Standard,
        examples: [
        {
            label: three_by_three_upper_triangular,
            packages: ["base", "ams"],
            input: r"\matrix{1 & x & x^2 \cr 0 & 1 & 2x \cr 0 & 0 & 1}",
            expected: r"\begin{matrix} 1 & x & x^2 \\ 0 & 1 & 2x \\ 0 & 0 & 1 \end{matrix}",
        },
        {
            label: ams_matrix_env_out_of_scope,
            packages: ["base", "ams"],
            input: r"\begin{matrix} a & b \\ c & d \end{matrix}",
            expected: r"\begin{matrix} a & b \\ c & d \end{matrix}",
        },
        ]
    }
    // END: Generated examples
}
