//! Rewrite plain-TeX pmatrix to the standard pmatrix environment with row breaks.
//!
//! ```yaml
//! proposal: pmatrix-to-pmatrix-env
//! triggers:
//!   - cmd:pmatrix
//! consumes:
//!   eliminates: [cmd:pmatrix, cmd:cr]
//!   touches: null
//! produces: env:pmatrix
//! rewrite_patterns:
//!   - {from: '\pmatrix{#1 \cr #2}', to: '\begin{pmatrix} #1 \\ #2 \end{pmatrix}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::rewrite_cr_body_to_environment;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule, env_targets};

define_rule! {
    pub static PMATRIX_TO_PMATRIX_ENV: PmatrixToPmatrixEnvRule {
        key: Base / "pmatrix-to-pmatrix-env",
        level: Standard,
        summary: "Rewrite plain-TeX pmatrix to the standard pmatrix environment with row breaks.",
        fidelity: Semantic,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::PMATRIX],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::PMATRIX],
            touches: cmd_targets![&base::cmd::CR],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::PMATRIX],
        },
        apply(rule, cx, node_id) {
            rewrite_cr_body_to_environment(
                Self::KEY,
                cx,
                node_id,
                &base::cmd::PMATRIX,
                &ams::env::PMATRIX,
                Vec::new(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PMATRIX_TO_PMATRIX_ENV,
        level: Standard,
        examples: [
        {
            label: three_by_three_parenthesized_matrix,
            packages: ["base", "ams"],
            input: r"\pmatrix{a_{11} & a_{12} & 0 \cr a_{21} & a_{22} & 1 \cr 0 & 1 & \lambda}",
            expected: r"\begin{pmatrix} a_{11} & a_{12} & 0 \\ a_{21} & a_{22} & 1 \\ 0 & 1 & \lambda \end{pmatrix}",
        },
        {
            label: ams_pmatrix_env_out_of_scope,
            packages: ["base", "ams"],
            input: r"\begin{pmatrix} x \\ y \end{pmatrix}",
            expected: r"\begin{pmatrix} x \\ y \end{pmatrix}",
        },
        ]
    }
    // END: Generated examples
}
