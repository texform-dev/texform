//! Rewrite norm to explicit double-vertical-bar fences.
//!
//! ```yaml
//! proposal: norm-to-double-vert-fence
//! consumes:
//!   eliminates: cmd:norm
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: auto-sized, from: '\norm{#1}', to: '\left\| #1 \right\|'}
//!   - {label: fixed-size, from: '\norm*{#1}', to: '\| #1 \|'}
//! ```
//!
//! The fixed-size starred form follows MathJax's physics Quantity expansion.
//! XeTeX's physics.sty starred branch uses \left...\smash{...}\right...\vphantom{...},
//! so it is not byte-equivalent to this MathJax-aligned rewrite.

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{FixedFenceToken, expand_delimiter_shorthand};
use crate::ast::Delimiter;
use crate::transform::rule::{RuleConsumes, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite norm to explicit double-vertical-bar fences.
    pub static NORM_TO_DOUBLE_VERT_FENCE: NormToDoubleVertFenceRule {
        key: Physics / "norm-to-double-vert-fence",
        tier: Expand,
        summary: "Rewrite norm to explicit double-vertical-bar fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::NORM],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            expand_delimiter_shorthand(
                rule.meta().key,
                cx,
                node_id,
                &physics::cmd::NORM,
                Delimiter::Control("|".to_string()),
                Delimiter::Control("|".to_string()),
                FixedFenceToken::Control("|"),
                FixedFenceToken::Control("|"),
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
        rule: NORM_TO_DOUBLE_VERT_FENCE,
        tier: Expand,
        examples: [
        {
            label: norm,
            packages: ["base", "physics"],
            input: r"\norm{v}",
            expected: r"\left\| v \right\|",
        },
        {
            label: norm_subscripted,
            packages: ["base", "physics"],
            input: r"\norm{A x-b}_2",
            expected: r"\left\| A x-b \right\|_2",
        },
        {
            label: norm_star_subscripted,
            packages: ["base", "physics"],
            input: r"\norm*{A x-b}_2",
            expected: r"\| A x-b \|_2",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NORM_TO_DOUBLE_VERT_FENCE,
        tier: Expand,
        examples: [
        {
            label: norm_star_power_context,
            packages: ["base", "physics"],
            input: r"\norm*{v}^2",
            expected: r"\| v \|^2",
        },
        ]
    }
}
