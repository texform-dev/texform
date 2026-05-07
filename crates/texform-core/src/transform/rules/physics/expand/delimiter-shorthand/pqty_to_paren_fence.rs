//! Rewrite pqty to explicit parenthesis fences.
//!
//! ```yaml
//! proposal: pqty-to-paren-fence
//! consumes:
//!   eliminates: cmd:pqty
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: auto-sized, from: '\pqty{#1}', to: '\left( #1 \right)'}
//!   - {label: fixed-size, from: '\pqty*{#1}', to: '( #1 )'}
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
    /// Rewrite pqty to explicit parenthesis fences.
    pub static PQTY_TO_PAREN_FENCE: PqtyToParenFenceRule {
        key: Physics / "pqty-to-paren-fence",
        tier: Expand,
        summary: "Rewrite pqty to explicit parenthesis fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::PQTY],
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
                &physics::cmd::PQTY,
                Delimiter::Char('('),
                Delimiter::Char(')'),
                FixedFenceToken::Char('('),
                FixedFenceToken::Char(')'),
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
        rule: PQTY_TO_PAREN_FENCE,
        tier: Expand,
        examples: [
        {
            label: pqty,
            packages: ["base", "physics"],
            input: r"\pqty{a+b}",
            expected: r"\left( a+b \right)",
        },
        {
            label: pqty_star_fixed_size,
            packages: ["base", "physics"],
            input: r"\pqty*{a+b}",
            expected: r"( a+b )",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: PQTY_TO_PAREN_FENCE,
        tier: Expand,
        examples: [
        {
            label: pqty_star_power_context,
            packages: ["base", "physics"],
            input: r"\pqty*{a+b}^2",
            expected: r"( a+b )^2",
        },
        ]
    }
}
