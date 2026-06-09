//! Expand eval notation to the explicit fence-and-bar form.
//!
//! ```yaml
//! proposal: eval-expand
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
//!   - {label: braced, from: '\eval{#1}#2', to: '\left.#1\vphantom{\int}\right|#2'}
//!   - {label: braced-star, from: '\eval*{#1}#2', to: '\left.\smash{#1}\vphantom{\int}\right|#2'}
//!   - {label: paren, from: \eval(#1|#2, to: '\left(#1\vphantom{\int}\right|#2'}
//!   - {label: paren-star, from: \eval*(#1|#2, to: '\left(\smash{#1}\vphantom{\int}\right|#2'}
//!   - {label: bracket, from: '\eval[#1|#2', to: '\left[#1\vphantom{\int}\right|#2'}
//!   - {label: bracket-star, from: '\eval*[#1|#2', to: '\left[\smash{#1}\vphantom{\int}\right|#2'}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use super::helpers::replace_with_eval_fence;
use crate::ast::{ArgumentKind, ArgumentSlot, ArgumentValue, Delimiter, NodeId};
use crate::rewrite::RuleError;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleKey, RuleProduces};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static EVAL_EXPAND: EvalExpandRule {
        key: Physics / "eval-expand",
        level: Expand,
        summary: "Expand eval notation to the explicit fence-and-bar form.",
        fidelity: Full,
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
            expand_eval(Self::KEY, cx, node_id)
        }
    }
}

fn expand_eval(
    rule: RuleKey,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
) -> Result<RuleEffect, RuleError> {
    let Some(command) = cx.match_command(node_id, &physics::cmd::EVAL) else {
        return Ok(RuleEffect::Skipped);
    };
    let subject = command.subject();
    let args = command.args.to_vec();

    cx.for_rule(rule).expect_arg_len(&args, 2, &subject)?;
    let starred = cx.for_rule(rule).star_arg_value(&args[0], &subject)?;
    let Some((body, left)) = eval_body(rule, cx, &args[1], &subject)? else {
        return Ok(RuleEffect::Skipped);
    };

    replace_with_eval_fence(cx, node_id, starred, body, left, Delimiter::Char('|'));
    Ok(RuleEffect::Applied)
}

fn eval_body(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
) -> Result<Option<(NodeId, Delimiter)>, RuleError> {
    let Some(arg) = slot else {
        return Ok(None);
    };
    match &arg.kind {
        ArgumentKind::Paired { open, close }
            if *open == Delimiter::Char('{') && *close == Delimiter::Char('}') =>
        {
            match arg.value {
                ArgumentValue::MathContent(body) => Ok(Some((body, Delimiter::None))),
                _ => Err(cx.for_rule(rule).invalid_shape(format!(
                    "{subject} braced eval body should be math content"
                ))),
            }
        }
        ArgumentKind::Paired { open, close }
            if *close == Delimiter::Char('|')
                && (*open == Delimiter::Char('(') || *open == Delimiter::Char('[')) =>
        {
            match arg.value {
                ArgumentValue::MathContent(body) => Ok(Some((body, open.clone()))),
                _ => Err(cx.for_rule(rule).invalid_shape(format!(
                    "{subject} paired eval body should be math content"
                ))),
            }
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: EVAL_EXPAND,
        level: Expand,
        examples: [
        {
            label: braced_eval_antiderivative,
            packages: ["base", "physics"],
            input: r"\eval{\frac{x^2}{2}}_0^1=\frac{1}{2}",
            expected: r"\left.\frac{x^2}{2}\vphantom{\int}\right|_0^1=\frac{1}{2}",
        },
        {
            label: braced_eval_star,
            packages: ["base", "physics"],
            input: r"\eval*{F(x)}_a^b",
            expected: r"\left.\smash{F(x)}\vphantom{\int}\right|_a^b",
        },
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
