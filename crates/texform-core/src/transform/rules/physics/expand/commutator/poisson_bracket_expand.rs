//! Expand poisson brackets to explicit brace fences.
//!
//! ```yaml
//! proposal: poisson-bracket-expand
//! consumes:
//!   eliminates: cmd:pb
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: poisson-bracket-auto-sized, from: '\pb{#1}{#2}', to: '\left\{#1,#2\right\}'}
//!   - {label: poisson-bracket-fixed-size, from: '\pb*{#1}{#2}', to: '\{#1,#2\}'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{
    FixedFenceToken, replace_with_binary_bracket_fence, required_braced_math_arg,
    required_math_arg,
};
use crate::ast::Delimiter;
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand poisson brackets to explicit brace fences.
    pub static POISSON_BRACKET_EXPAND: PoissonBracketExpandRule {
        key: Physics / "poisson-bracket-expand",
        tier: Expand,
        summary: "Expand poisson brackets to explicit brace fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::PB],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::PB) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();

            cx.expect_arg_len(rule.meta().key, &args, 3, &subject)?;
            let starred = star_arg_value(rule.meta().key, cx, &args[0], &subject)?;
            let left = required_braced_math_arg(rule.meta().key, cx, &args[1], &subject, "left operand")?;
            let right = required_math_arg(rule.meta().key, cx, &args[2], &subject, "right operand")?;

            replace_with_binary_bracket_fence(
                cx,
                node_id,
                starred,
                left,
                right,
                Delimiter::Control("{".to_string()),
                Delimiter::Control("}".to_string()),
                FixedFenceToken::Control("{"),
                FixedFenceToken::Control("}"),
            );
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
        rule: POISSON_BRACKET_EXPAND,
        tier: Expand,
        examples: [
        {
            label: poisson_bracket_canonical,
            packages: ["base", "physics"],
            input: r"\pb{q_i}{p_j}=\delta_{ij}",
            expected: r"\left\{q_i,p_j\right\}=\delta_{ij}",
        },
        {
            label: poisson_bracket_bare_second_operand,
            packages: ["base", "physics"],
            input: r"\pb{f}g",
            expected: r"\left\{f,g\right\}",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn poisson_bracket_star_fixed_size() {
        let actual = transform_serialized(r"\pb*{f}{g}=0");

        assert_eq!(actual, r"\{ f , g \} = 0");
    }

    #[test]
    fn poisson_bracket_star_power_context() {
        let actual = transform_serialized(r"\pb*{f}{g}^2");

        assert_eq!(actual, r"\{ f , g \} ^ { 2 }");
    }

    fn transform_serialized(input: &str) -> String {
        use crate::transform::TransformRule as _;

        let parse_ctx = crate::parse::ParseContext::from_packages(&["base", "physics"]);
        let transform_ctx =
            crate::transform::TransformContextBuilder::from_tiers(&[crate::transform::RuleTier::Expand])
                .only(POISSON_BRACKET_EXPAND.meta().key)
                .build_with(&parse_ctx)
                .expect("transform context should build");
        let mut ast = parse_ctx.parse_to_ast(input, true).expect("parse input should succeed");

        crate::transform::transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("poisson-bracket-expand transform should succeed");

        crate::serialize::serialize(&ast)
    }
}
