//! Rewrite plain-TeX cases to the standard cases environment with row breaks.
//!
//! ```yaml
//! proposal: cases-to-cases-env
//! triggers:
//!   - cmd:cases
//! consumes:
//!   eliminates: [cmd:cases, cmd:cr]
//!   touches: null
//! produces: env:cases
//! rewrite_patterns:
//!   - {from: '\cases{#1 \cr #2}', to: '\begin{cases} #1 \\ #2 \end{cases}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::rewrite_cr_body_to_environment;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule, env_targets};

define_rule! {
    pub static CASES_TO_CASES_ENV: CasesToCasesEnvRule {
        key: Base / "cases-to-cases-env",
        class: Standard,
        summary: "Rewrite plain-TeX cases to the standard cases environment with row breaks.",
        safety: Semantic,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::CASES],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::CASES, &base::cmd::CR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::CASES],
        },
        apply(rule, cx, node_id) {
            rewrite_cr_body_to_environment(
                Self::KEY,
                cx,
                node_id,
                &base::cmd::CASES,
                &ams::env::CASES,
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
        rule: CASES_TO_CASES_ENV,
        class: Standard,
        examples: [
        {
            label: three_branch_piecewise,
            packages: ["base", "ams"],
            input: r"\cases{x^2 & x>1 \cr 0 & x=0 \cr -x & x<0}",
            expected: r"\begin{cases} x^2 & x>1 \\ 0 & x=0 \\ -x & x<0 \end{cases}",
        },
        {
            label: ams_cases_env_out_of_scope,
            packages: ["base", "ams"],
            input: r"\begin{cases} x & x>0 \\ -x & x<0 \end{cases}",
            expected: r"\begin{cases} x & x>0 \\ -x & x<0 \end{cases}",
        },
        ]
    }
    // END: Generated examples
}
