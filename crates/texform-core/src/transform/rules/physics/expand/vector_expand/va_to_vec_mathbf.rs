//! Canonicalize va to an explicit vec-wrapped mathbf form.
//!
//! ```yaml
//! proposal: va-to-vec-mathbf
//! consumes:
//!   eliminates: cmd:va
//!   touches: null
//! produces:
//!   - cmd:vec
//!   - cmd:mathbf
//!   - cmd:boldsymbol
//! rewrite_patterns:
//!   - {label: va, from: '\va{#1}', to: '\vec{\mathbf{#1}}'}
//!   - {label: va-star, from: '\va*{#1}', to: '\vec{\boldsymbol{#1}}'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::boldsymbol;
use texform_specs::builtin::physics;

use super::helpers::{replace_with_wrapped_vector_style, vector_args};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Canonicalize va to an explicit vec-wrapped mathbf form.
    pub static VA_TO_VEC_MATHBF: VaToVecMathbfRule {
        key: Physics / "va-to-vec-mathbf",
        tier: Expand,
        summary: "Canonicalize va to an explicit vec-wrapped mathbf form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::VA],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::VEC, &base::cmd::MATHBF, &boldsymbol::cmd::BOLDSYMBOL],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::VA) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();
            let (starred, body) = vector_args(rule.meta().key, cx, &args, &subject)?;

            replace_with_wrapped_vector_style(cx, node_id, &base::cmd::VEC, starred, body);
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
        rule: VA_TO_VEC_MATHBF,
        tier: Expand,
        examples: [
        {
            label: va_angular_momentum,
            packages: ["base", "physics", "boldsymbol"],
            input: r"\va{L}=\vb{r}\times\vb{p}",
            expected: r"\vec{\mathbf{L}}=\vb{r}\times\vb{p}",
        },
        {
            label: va_star_bold_italic,
            packages: ["base", "physics", "boldsymbol"],
            input: r"\va*{p}+\va*{\alpha}",
            expected: r"\vec{\boldsymbol{p}}+\vec{\boldsymbol{\alpha}}",
        },
        ]
    }
    // END: Generated examples

}
