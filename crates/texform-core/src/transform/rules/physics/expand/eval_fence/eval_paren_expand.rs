//! Expand paren-style eval notation to the explicit fence-and-bar form.
//!
//! ```yaml
//! proposal: eval-paren-expand
//! triggers:
//!   - cmd:eval
//! consumes:
//!   eliminates: cmd:eval
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//!   - cmd:vphantom
//!   - cmd:smash
//! rewrite_patterns:
//!   - {label: paren, from: \eval(#1|#2, to: '\left(#1\vphantom{\int}\right|#2'}
//!   - {label: paren-star, from: \eval*(#1|#2, to: '\left(\smash{#1}\vphantom{\int}\right|#2'}
//!   - {label: bracket, from: '\eval[#1|#2', to: '\left[#1\vphantom{\int}\right|#2'}
//!   - {label: bracket-star, from: '\eval*[#1|#2', to: '\left[\smash{#1}\vphantom{\int}\right|#2'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::replace_with_eval_fence;
use crate::ast::{ArgumentKind, ArgumentSlot, ArgumentValue, Delimiter, NodeId};
use crate::transform::engine::TransformError;
use crate::transform::helpers::star_arg_value;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleKey, RuleProduces};
use crate::transform::rule_context::RuleContext;
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand paren-style eval notation to the explicit fence-and-bar form.
    pub static EVAL_PAREN_EXPAND: EvalParenExpandRule {
        key: Physics / "eval-paren-expand",
        class: Expand,
        summary: "Expand paren-style eval notation to the explicit fence-and-bar form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::EVAL],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::EVAL],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT, &base::cmd::VPHANTOM, &base::cmd::SMASH],
        },
        apply(rule, cx, node_id) {
            expand_paired_eval(rule.meta().key, cx, node_id)
        }
    }
}

fn expand_paired_eval(
    rule: RuleKey,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
) -> Result<RuleEffect, TransformError> {
    let Some(command) = cx.match_command(node_id, &physics::cmd::EVAL) else {
        return Ok(RuleEffect::Skipped);
    };
    let subject = format!(r"\{}", command.name);
    let args = command.args.to_vec();

    cx.expect_arg_len(rule, &args, 2, &subject)?;
    let starred = star_arg_value(rule, cx, &args[0], &subject)?;
    let Some((body, left)) = paired_eval_body(rule, cx, &args[1], &subject)? else {
        return Ok(RuleEffect::Skipped);
    };

    replace_with_eval_fence(cx, node_id, starred, body, left, Delimiter::Char('|'));
    Ok(RuleEffect::Applied)
}

fn paired_eval_body(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
) -> Result<Option<(NodeId, Delimiter)>, TransformError> {
    let Some(arg) = slot else {
        return Ok(None);
    };
    match &arg.kind {
        ArgumentKind::Paired { open, close }
            if *close == Delimiter::Char('|')
                && (*open == Delimiter::Char('(') || *open == Delimiter::Char('[')) =>
        {
            match arg.value {
                ArgumentValue::MathContent(body) => Ok(Some((body, open.clone()))),
                _ => Err(cx.invalid_shape(
                    rule,
                    format!("{subject} paired eval body should be math content"),
                )),
            }
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: EVAL_PAREN_EXPAND,
        class: Expand,
        examples: [
        {
            label: paren_eval_trig,
            packages: ["base", "physics"],
            input: r"\eval(\sin x|_0^{\pi}=0",
            expected: r"\left(\sin x\vphantom{\int}\right|_0^{\pi}=0",
        },
        {
            label: paren_eval_star,
            packages: ["base", "physics"],
            input: r"\eval*(\sin x|_0^{\pi}=0",
            expected: r"\left(\smash{\sin x}\vphantom{\int}\right|_0^{\pi}=0",
        },
        {
            label: bracket_eval_point,
            packages: ["base", "physics"],
            input: r"\eval[f(x)|_{x=0}=1",
            expected: r"\left[f(x)\vphantom{\int}\right|_{x=0}=1",
        },
        {
            label: bracket_eval_star,
            packages: ["base", "physics"],
            input: r"\eval*[f(x)|_{x=0}=1",
            expected: r"\left[\smash{f(x)}\vphantom{\int}\right|_{x=0}=1",
        },
        ]
    }
    // END: Generated examples
}
