//! Rewrite abs to explicit vertical bar fences.
//!
//! ```yaml
//! proposal: abs-to-vert-fence
//! consumes:
//!   eliminates: cmd:abs
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: auto-sized, from: '\abs{#1}', to: '\left| #1 \right|'}
//!   - {label: fixed-size, from: '\abs*{#1}', to: '| #1 |'}
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
    /// Rewrite abs to explicit vertical bar fences.
    pub static ABS_TO_VERT_FENCE: AbsToVertFenceRule {
        key: Physics / "abs-to-vert-fence",
        tier: Expand,
        summary: "Rewrite abs to explicit vertical bar fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::ABS],
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
                &physics::cmd::ABS,
                Delimiter::Char('|'),
                Delimiter::Char('|'),
                FixedFenceToken::Char('|'),
                FixedFenceToken::Char('|'),
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
        rule: ABS_TO_VERT_FENCE,
        tier: Expand,
        examples: [
        {
            label: abs_inequality,
            packages: ["base", "physics"],
            input: r"\abs{x-y}<\varepsilon",
            expected: r"\left| x-y \right|<\varepsilon",
        },
        {
            label: abs_fraction_body,
            packages: ["base", "physics"],
            input: r"\abs{\frac{x}{y}}=1",
            expected: r"\left| \frac{x}{y} \right|=1",
        },
        {
            label: abs_star_fixed_size,
            packages: ["base", "physics"],
            input: r"\abs*{x-y}<\varepsilon",
            expected: r"| x-y |<\varepsilon",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: ABS_TO_VERT_FENCE,
        tier: Expand,
        examples: [
        {
            label: abs_star_power_context,
            packages: ["base", "physics"],
            input: r"\abs*{x-y}^2",
            expected: r"| x-y |^2",
        },
        ]
    }
}
