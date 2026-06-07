//! Rewrite eqalign to the standard aligned environment.
//!
//! ```yaml
//! proposal: eqalign-to-aligned-env
//! triggers:
//!   - cmd:eqalign
//! consumes:
//!   eliminates: [cmd:eqalign, cmd:cr]
//!   touches: null
//! produces: env:aligned
//! rewrite_patterns:
//!   - {from: '\eqalign{#1 \cr #2}', to: '\begin{aligned} #1 \\ #2 \end{aligned}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::rewrite_cr_body_to_environment;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule, env_targets};

define_rule! {
    pub static EQALIGN_TO_ALIGNED_ENV: EqalignToAlignedEnvRule {
        key: Base / "eqalign-to-aligned-env",
        level: Standard,
        summary: "Rewrite eqalign to the standard aligned environment.",
        fidelity: Semantic,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::EQALIGN],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::EQALIGN],
            touches: cmd_targets![&base::cmd::CR],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::ALIGNED],
        },
        apply(rule, cx, node_id) {
            rewrite_cr_body_to_environment(
                Self::KEY,
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
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: EQALIGN_TO_ALIGNED_ENV,
        level: Standard,
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
