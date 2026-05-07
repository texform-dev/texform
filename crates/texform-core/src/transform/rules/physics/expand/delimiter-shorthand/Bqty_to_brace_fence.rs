//! Rewrite Bqty to explicit brace fences.
//!
//! ```yaml
//! proposal: Bqty-to-brace-fence
//! consumes:
//!   eliminates: cmd:Bqty
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: auto-sized, from: '\Bqty{#1}', to: '\left\{ #1 \right\}'}
//!   - {label: fixed-size, from: '\Bqty*{#1}', to: '\{ #1 \}'}
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
    /// Rewrite Bqty to explicit brace fences.
    pub static BQTY_TO_BRACE_FENCE: BqtyToBraceFenceRule {
        key: Physics / "Bqty-to-brace-fence",
        tier: Expand,
        summary: "Rewrite Bqty to explicit brace fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::BQTY_2],
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
                &physics::cmd::BQTY_2,
                Delimiter::Control("{".to_string()),
                Delimiter::Control("}".to_string()),
                FixedFenceToken::Control("{"),
                FixedFenceToken::Control("}"),
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
        rule: BQTY_TO_BRACE_FENCE,
        tier: Expand,
        examples: [
        {
            label: bqty,
            packages: ["base", "physics"],
            input: r"\Bqty{a+b}",
            expected: r"\left\{ a+b \right\}",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn bqty_star_set_builder_serializes_visible_brace_commands() {
        let actual = transform_serialized(r"\Bqty*{x\mid x>0}");

        assert_eq!(actual, r"\{ x \mid x > 0 \}");
    }

    #[test]
    fn bqty_star_power_context_serializes_visible_brace_commands() {
        let actual = transform_serialized(r"\Bqty*{x>0}^2");

        assert_eq!(actual, r"\{ x > 0 \} ^ { 2 }");
    }

    fn transform_serialized(input: &str) -> String {
        use crate::transform::TransformRule as _;

        let parse_ctx = crate::parse::ParseContext::from_packages(&["base", "physics"]);
        let transform_ctx = crate::transform::TransformContextBuilder::from_tiers(&[
            crate::transform::RuleTier::Expand,
        ])
        .only(BQTY_TO_BRACE_FENCE.meta().key)
        .build_with(&parse_ctx)
        .expect("transform context should build");
        let mut ast = parse_ctx.parse_to_ast(input, true).expect("parse input should succeed");

        crate::transform::transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("Bqty-to-brace-fence transform should succeed");

        crate::serialize::serialize(&ast)
    }
}
