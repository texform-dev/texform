//! Rewrite pqty to explicit parenthesis fences.
//!
//! ```yaml
//! proposal: pqty-to-paren-fence
//! triggers:
//!   - cmd:pqty
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

use super::helpers::{FencePair, replace_with_delimiter_shorthand};
use crate::ast::{ArgumentKind, ArgumentValue, Delimiter};
use crate::transform::helpers::{FenceToken, star_arg_value};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static PQTY_TO_PAREN_FENCE: PqtyToParenFenceRule {
        key: Physics / "pqty-to-paren-fence",
        class: Expand,
        summary: "Rewrite pqty to explicit parenthesis fences.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::PQTY],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::PQTY],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::PQTY) else {
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
                    auto_left: Delimiter::Char('('),
                    auto_right: Delimiter::Char(')'),
                    fixed_left: FenceToken::Char('('),
                    fixed_right: FenceToken::Char(')'),
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
        rule: PQTY_TO_PAREN_FENCE,
        class: Expand,
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
        class: Expand,
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
