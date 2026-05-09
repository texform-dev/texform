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

use super::helpers::{FencePair, FixedFenceToken, replace_with_delimiter_shorthand};
use crate::ast::{ArgumentKind, ArgumentValue, Delimiter};
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
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
            let Some(command) = cx.match_command(node_id, &physics::cmd::ABS) else {
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
                    auto_left: Delimiter::Char('|'),
                    auto_right: Delimiter::Char('|'),
                    fixed_left: FixedFenceToken::Char('|'),
                    fixed_right: FixedFenceToken::Char('|'),
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
