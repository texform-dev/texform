//! Expand expectation-value helpers to explicit angle brackets.
//!
//! ```yaml
//! proposal: expectation-expand
//! triggers:
//!   - cmd:ev
//! consumes:
//!   eliminates: cmd:ev
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:middle
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: expectation-body-auto-sized, from: '\ev{#1}', to: '\left\langle #1 \right\rangle'}
//!   - {label: expectation-body-fixed-size, from: '\ev*{#1}', to: '\langle #1 \rangle'}
//!   - {label: expectation-state-auto-sized, from: '\ev{#1}{#2}', to: '\left\langle #2 \right\vert #1 \left\vert #2 \right\rangle'}
//!   - {label: expectation-state-fixed-size, from: '\ev*{#1}{#2}', to: '\langle #2 \vert #1 \vert #2 \rangle'}
//!   - {label: expectation-state-middle-sized, from: '\ev**{#1}{#2}', to: '\left\langle #2 \middle\vert #1 \middle\vert #2 \right\rangle'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use super::helpers::{
    BraketSize, optional_group_arg, required_math_arg, replace_with_expectation_body,
    replace_with_expectation_state,
};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static EXPECTATION_EXPAND: ExpectationExpandRule {
        key: Physics / "expectation-expand",
        class: Expand,
        summary: "Expand expectation-value helpers to explicit angle brackets.",
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::EV],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::EV],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::MIDDLE, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::EV) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();
            cx.for_rule(Self::KEY).expect_arg_len(&args, 4, &subject)?;

            let first_star = cx.for_rule(Self::KEY).star_arg_value(&args[0], &subject)?;
            let second_star = cx.for_rule(Self::KEY).star_arg_value(&args[1], &subject)?;
            let body = required_math_arg(Self::KEY, cx, &args[2], &subject, "body")?;
            let state = optional_group_arg(Self::KEY, cx, &args[3], &subject, "state")?;

            match state {
                Some(state) => {
                    let size = if second_star {
                        BraketSize::Middle
                    } else if first_star {
                        BraketSize::Fixed
                    } else {
                        BraketSize::Auto
                    };
                    replace_with_expectation_state(cx, node_id, size, body, state);
                }
                None => {
                    let size = if first_star {
                        BraketSize::Fixed
                    } else {
                        BraketSize::Auto
                    };
                    replace_with_expectation_body(cx, node_id, size, body);
                }
            }

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
        rule: EXPECTATION_EXPAND,
        class: Expand,
        examples: [
        {
            label: expectation_state_label,
            packages: ["base", "physics"],
            input: r"\ev{H}_{\psi}=E_0",
            expected: r"\left\langle H \right\rangle_{\psi}=E_0",
        },
        {
            label: expectation_state_argument,
            packages: ["base", "physics"],
            input: r"\ev{H}{\psi}=E_0",
            expected: r"\left\langle \psi \right\vert H \left\vert \psi \right\rangle=E_0",
        },
        {
            label: expectation_star_body_fixed_size,
            packages: ["base", "physics"],
            input: r"\ev*{A(t)}",
            expected: r"\langle A(t) \rangle",
        },
        {
            label: expectation_star_state_fixed_size,
            packages: ["base", "physics"],
            input: r"\ev*{H}{\psi}",
            expected: r"\langle \psi \vert H \vert \psi \rangle",
        },
        {
            label: expectation_double_star_state_middle_sized,
            packages: ["base", "physics"],
            input: r"\ev**{H}{\psi}",
            expected: r"\left\langle \psi \middle\vert H \middle\vert \psi \right\rangle",
        },
        ]
    }
    // END: Generated examples
}
