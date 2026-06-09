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

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use super::helpers::{FencePair, replace_with_delimiter_shorthand};
use crate::ast::{ArgumentKind, ArgumentValue, Delimiter};
use crate::rewrite::helpers::FenceToken;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NORM_TO_DOUBLE_VERT_FENCE: NormToDoubleVertFenceRule {
        key: Physics / "norm-to-double-vert-fence",
        level: Expand,
        summary: "Rewrite norm to explicit double-vertical-bar fences.",
        fidelity: Full,
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
            let subject = command.subject();
            let args = command.args.to_vec();

            cx.for_rule(Self::KEY).expect_arg_len(&args, 2, &subject)?;
            let starred = cx.for_rule(Self::KEY).star_arg_value(&args[0], &subject)?;
            let body = match &args[1] {
                Some(arg) if arg.kind == ArgumentKind::Group => match arg.value {
                    ArgumentValue::MathContent(node_id) => node_id,
                    _ => return Err(cx.for_rule(Self::KEY).invalid_shape(format!("{subject} body should be math content"))),
                },
                _ => return Err(cx.for_rule(Self::KEY).invalid_shape(format!("{subject} body should be a required braced math group"))),
            };

            replace_with_delimiter_shorthand(
                cx,
                node_id,
                starred,
                body,
                FencePair {
                    auto_left: Delimiter::Control("|".to_string()),
                    auto_right: Delimiter::Control("|".to_string()),
                    fixed_left: FenceToken::Control("|"),
                    fixed_right: FenceToken::Control("|"),
                },
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: NORM_TO_DOUBLE_VERT_FENCE,
        level: Expand,
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
        level: Expand,
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
