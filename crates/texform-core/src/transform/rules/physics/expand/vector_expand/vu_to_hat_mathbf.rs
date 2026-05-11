//! Canonicalize vu to an explicit hat-wrapped mathbf form.
//!
//! ```yaml
//! proposal: vu-to-hat-mathbf
//! triggers:
//!   - cmd:vu
//! consumes:
//!   eliminates: cmd:vu
//!   touches: null
//! produces:
//!   - cmd:hat
//!   - cmd:mathbf
//!   - cmd:boldsymbol
//! rewrite_patterns:
//!   - {label: vu, from: '\vu{#1}', to: '\hat{\mathbf{#1}}'}
//!   - {label: vu-star, from: '\vu*{#1}', to: '\hat{\boldsymbol{#1}}'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::boldsymbol;
use texform_specs::builtin::physics;

use super::helpers::{replace_with_wrapped_vector_style, vector_args};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static VU_TO_HAT_MATHBF: VuToHatMathbfRule {
        key: Physics / "vu-to-hat-mathbf",
        class: Expand,
        summary: "Canonicalize vu to an explicit hat-wrapped mathbf form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::VU],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::VU],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::HAT, &base::cmd::MATHBF, &boldsymbol::cmd::BOLDSYMBOL],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::VU) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();
            let (starred, body) = vector_args(Self::KEY, cx, &args, &subject)?;

            replace_with_wrapped_vector_style(cx, node_id, &base::cmd::HAT, starred, body);
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
        rule: VU_TO_HAT_MATHBF,
        class: Expand,
        examples: [
        {
            label: vu_boundary_normal,
            packages: ["base", "physics", "boldsymbol"],
            input: r"\vu{n}\cdot\vb{E}=0",
            expected: r"\hat{\mathbf{n}}\cdot\vb{E}=0",
        },
        {
            label: vu_star_bold_italic,
            packages: ["base", "physics", "boldsymbol"],
            input: r"\vu*{p}\cdot\vu*{\alpha}",
            expected: r"\hat{\boldsymbol{p}}\cdot\hat{\boldsymbol{\alpha}}",
        },
        ]
    }
    // END: Generated examples

}
