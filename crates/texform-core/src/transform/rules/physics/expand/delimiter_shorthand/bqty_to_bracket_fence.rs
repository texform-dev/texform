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

use super::helpers::{FencePair, FixedFenceToken, replace_with_delimiter_shorthand};
use crate::ast::{ArgumentKind, ArgumentValue, Delimiter};
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
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
            let Some(command) = cx.match_command(node_id, &physics::cmd::BQTY) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();

            cx.expect_arg_len(rule.meta().key, &args, 2, &subject)?;
            let starred = star_arg_value(rule.meta().key, cx, &args[0], &subject)?;
            let body = match &args[1] {
                Some(arg) if arg.kind == ArgumentKind::Group => match arg.value {
                    ArgumentValue::MathContent(node_id) => node_id,
                    _ => return Err(cx.invalid_shape(
                        rule.meta().key,
                        format!("{subject} body should be math content"),
                    )),
                },
                _ => return Err(cx.invalid_shape(
                    rule.meta().key,
                    format!("{subject} body should be a required braced math group"),
                )),
            };

            replace_with_delimiter_shorthand(
                cx,
                node_id,
                starred,
                body,
                FencePair {
                    auto_left: Delimiter::Char('['),
                    auto_right: Delimiter::Char(']'),
                    fixed_left: FixedFenceToken::Char('['),
                    fixed_right: FixedFenceToken::Char(']'),
                },
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
