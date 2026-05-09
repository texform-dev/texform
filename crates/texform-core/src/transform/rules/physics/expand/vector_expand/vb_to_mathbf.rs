//! Canonicalize vb to an explicit mathbf wrapper.
//!
//! ```yaml
//! proposal: vb-to-mathbf
//! triggers:
//!   - cmd:vb
//! consumes:
//!   eliminates: cmd:vb
//!   touches: null
//! produces:
//!   - cmd:mathbf
//!   - cmd:boldsymbol
//! rewrite_patterns:
//!   - {label: vb, from: '\vb{#1}', to: '\mathbf{#1}'}
//!   - {label: vb-star, from: '\vb*{#1}', to: '\boldsymbol{#1}'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::boldsymbol;
use texform_specs::builtin::physics;

use super::helpers::{replace_with_vector_style, vector_args};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Canonicalize vb to an explicit mathbf wrapper.
    pub static VB_TO_MATHBF: VbToMathbfRule {
        key: Physics / "vb-to-mathbf",
        class: Expand,
        summary: "Canonicalize vb to an explicit mathbf wrapper.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::VB],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::VB],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::MATHBF, &boldsymbol::cmd::BOLDSYMBOL],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::VB) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();
            let (starred, body) = vector_args(rule.meta().key, cx, &args, &subject)?;

            replace_with_vector_style(cx, node_id, starred, body);
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
        rule: VB_TO_MATHBF,
        class: Expand,
        examples: [
        {
            label: vb_momentum,
            packages: ["base", "physics", "boldsymbol"],
            input: r"\vb{p}=mv",
            expected: r"\mathbf{p}=mv",
        },
        {
            label: vb_star_bold_italic,
            packages: ["base", "physics", "boldsymbol"],
            input: r"\vb*{p}+\vb*{\alpha}",
            expected: r"\boldsymbol{p}+\boldsymbol{\alpha}",
        },
        ]
    }
    // END: Generated examples

}
