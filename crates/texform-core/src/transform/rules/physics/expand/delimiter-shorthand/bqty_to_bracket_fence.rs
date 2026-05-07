//! Rewrite bqty to explicit bracket fences.
//!
//! ```yaml
//! proposal: bqty-to-bracket-fence
//! consumes:
//!   eliminates: cmd:bqty
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: auto-sized, from: '\bqty{#1}', to: '\left[ #1 \right]'}
//!   - {label: fixed-size, from: '\bqty*{#1}', to: '[ #1 ]'}
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
    /// Rewrite bqty to explicit bracket fences.
    pub static BQTY_TO_BRACKET_FENCE: BqtyToBracketFenceRule {
        key: Physics / "bqty-to-bracket-fence",
        tier: Expand,
        summary: "Rewrite bqty to explicit bracket fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::BQTY],
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
                &physics::cmd::BQTY,
                Delimiter::Char('['),
                Delimiter::Char(']'),
                FixedFenceToken::Char('['),
                FixedFenceToken::Char(']'),
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
        rule: BQTY_TO_BRACKET_FENCE,
        tier: Expand,
        examples: [
        {
            label: bqty,
            packages: ["base", "physics"],
            input: r"\bqty{a+b}",
            expected: r"\left[ a+b \right]",
        },
        {
            label: bqty_subscript_index,
            packages: ["base", "physics"],
            input: r"A_{\bqty{i,j}}",
            expected: r"A_{\left[ i,j \right]}",
        },
        {
            label: bqty_star_subscript_index,
            packages: ["base", "physics"],
            input: r"A_{\bqty*{i,j}}",
            expected: r"A_{[ i,j ]}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: BQTY_TO_BRACKET_FENCE,
        tier: Expand,
        examples: [
        {
            label: bqty_star_power_context,
            packages: ["base", "physics"],
            input: r"\bqty*{i,j}^2",
            expected: r"[ i,j ]^2",
        },
        ]
    }
}
