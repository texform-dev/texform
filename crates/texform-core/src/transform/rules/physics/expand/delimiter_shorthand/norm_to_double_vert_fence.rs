//! Rewrite norm to explicit double-vertical-bar fences.
//!
//! ```yaml
//! proposal: norm-to-double-vert-fence
//! triggers:
//!   - cmd:norm
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

use super::helpers::{FencePair, FixedFenceToken, replace_with_delimiter_shorthand};
use crate::ast::{ArgumentKind, ArgumentValue, Delimiter};
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite norm to explicit double-vertical-bar fences.
    pub static NORM_TO_DOUBLE_VERT_FENCE: NormToDoubleVertFenceRule {
        key: Physics / "norm-to-double-vert-fence",
        class: Expand,
        summary: "Rewrite norm to explicit double-vertical-bar fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::NORM],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::NORM],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::NORM) else {
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
                    auto_left: Delimiter::Control("|".to_string()),
                    auto_right: Delimiter::Control("|".to_string()),
                    fixed_left: FixedFenceToken::Control("|"),
                    fixed_right: FixedFenceToken::Control("|"),
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
        rule: NORM_TO_DOUBLE_VERT_FENCE,
        class: Expand,
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
        class: Expand,
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
