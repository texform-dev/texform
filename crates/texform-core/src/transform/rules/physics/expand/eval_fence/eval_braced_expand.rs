//! Expand braced eval notation to the explicit fence-and-bar form.
//!
//! ```yaml
//! proposal: eval-braced-expand
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
    /// Expand braced eval notation to the explicit fence-and-bar form.
    pub static EVAL_BRACED_EXPAND: EvalBracedExpandRule {
        key: Physics / "eval-braced-expand",
        class: Expand,
        summary: "Expand braced eval notation to the explicit fence-and-bar form.",
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
            expand_braced_eval(rule.meta().key, cx, node_id)
        }
    }
}

fn expand_braced_eval(
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
    let Some(body) = braced_eval_body(rule, cx, &args[1], &subject)? else {
        return Ok(RuleEffect::Skipped);
    };

    replace_with_eval_fence(
        cx,
        node_id,
        starred,
        body,
        Delimiter::None,
        Delimiter::Char('|'),
    );
    Ok(RuleEffect::Applied)
}

fn braced_eval_body(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
) -> Result<Option<NodeId>, TransformError> {
    let Some(arg) = slot else {
        return Ok(None);
    };
    match &arg.kind {
        ArgumentKind::Paired { open, close }
            if *open == Delimiter::Char('{') && *close == Delimiter::Char('}') =>
        {
            match arg.value {
                ArgumentValue::MathContent(node_id) => Ok(Some(node_id)),
                _ => Err(cx.invalid_shape(
                    rule,
                    format!("{subject} braced eval body should be math content"),
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
        rule: EVAL_BRACED_EXPAND,
        class: Expand,
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
        ]
    }
    // END: Generated examples
}
