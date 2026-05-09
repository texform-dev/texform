//! Rewrite eqalign to the standard aligned environment.
//!
//! ```yaml
//! proposal: eqalign-to-aligned-env
//! consumes:
//!   eliminates: [cmd:eqalign, cmd:cr]
//!   touches: null
//! produces: env:aligned
//! rewrite_patterns:
//!   - {from: '\eqalign{#1 \cr #2}', to: '\begin{aligned} #1 \\ #2 \end{aligned}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::rewrite_cr_body_to_environment;
use crate::transform::rule::{RuleConsumes, RuleProduces};
use crate::transform::{cmd_targets, define_rule, env_targets};

define_rule! {
    /// Rewrite eqalign to the standard aligned environment.
    pub static EQALIGN_TO_ALIGNED_ENV: EqalignToAlignedEnvRule {
        key: Base / "eqalign-to-aligned-env",
        class: Standard,
        summary: "Rewrite eqalign to the standard aligned environment.",
        phase: Normalize,
        safety: Semantic,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::EQALIGN, &base::cmd::CR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::ALIGNED],
        },
        apply(rule, cx, node_id) {
            rewrite_cr_body_to_environment(
                rule.meta().key,
                cx,
                node_id,
                &base::cmd::EQALIGN,
                &ams::env::ALIGNED,
                vec![None],
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
        rule: EQALIGN_TO_ALIGNED_ENV,
        class: Standard,
        examples: [
        {
            label: three_line_aligned_derivatives,
            packages: ["base", "ams"],
            input: r"\eqalign{f(x)&=x^3-1 \cr f'(x)&=3x^2 \cr f''(x)&=6x}",
            expected: r"\begin{aligned} f(x)&=x^3-1 \\ f'(x)&=3x^2 \\ f''(x)&=6x \end{aligned}",
        },
        {
            label: continued_alignment,
            packages: ["base", "ams"],
            input: r"\eqalign{a&=b+c \cr &=d}",
            expected: r"\begin{aligned} a&=b+c \\ &=d \end{aligned}",
        },
        ]
    }
    // END: Generated examples
}
