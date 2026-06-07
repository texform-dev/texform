//! Expand matrix elements to explicit angle-bracket and bar fences.
//!
//! ```yaml
//! proposal: matrixel-expand
//! triggers:
//!   - cmd:matrixel
//! consumes:
//!   eliminates: cmd:matrixel
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:middle
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: matrixel-auto-sized, from: '\matrixel{#1}{#2}{#3}', to: '\left\langle #1 \right\vert #2 \left\vert #3 \right\rangle'}
//!   - {label: matrixel-fixed-size, from: '\matrixel*{#1}{#2}{#3}', to: '\langle #1 \vert #2 \vert #3 \rangle'}
//!   - {label: matrixel-middle-sized, from: '\matrixel**{#1}{#2}{#3}', to: '\left\langle #1 \middle\vert #2 \middle\vert #3 \right\rangle'}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use super::helpers::{BraketSize, required_math_arg, replace_with_matrix_element};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static MATRIXEL_EXPAND: MatrixelExpandRule {
        key: Physics / "matrixel-expand",
        level: Expand,
        summary: "Expand matrix elements to explicit angle-bracket and bar fences.",
        fidelity: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::MATRIXEL],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::MATRIXEL],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::MIDDLE, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::MATRIXEL) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();
            cx.for_rule(Self::KEY).expect_arg_len(&args, 5, &subject)?;

            let first_star = cx.for_rule(Self::KEY).star_arg_value(&args[0], &subject)?;
            let second_star = cx.for_rule(Self::KEY).star_arg_value(&args[1], &subject)?;
            let left_state =
                required_math_arg(Self::KEY, cx, &args[2], &subject, "left state")?;
            let operator =
                required_math_arg(Self::KEY, cx, &args[3], &subject, "operator")?;
            let right_state =
                required_math_arg(Self::KEY, cx, &args[4], &subject, "right state")?;
            let size = if second_star {
                BraketSize::Middle
            } else if first_star {
                BraketSize::Fixed
            } else {
                BraketSize::Auto
            };
            replace_with_matrix_element(cx, node_id, size, left_state, operator, right_state);
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
        rule: MATRIXEL_EXPAND,
        level: Expand,
        examples: [
        {
            label: matrix_element_operator,
            packages: ["base", "physics"],
            input: r"\matrixel{n}{\hat{x}}{m}=x_{nm}",
            expected: r"\left\langle n \right\vert \hat{x} \left\vert m \right\rangle=x_{nm}",
        },
        {
            label: matrix_element_star_fixed_size,
            packages: ["base", "physics"],
            input: r"\matrixel*{n}{\hat{x}}{m}",
            expected: r"\langle n \vert \hat{x} \vert m \rangle",
        },
        {
            label: matrix_element_double_star_middle_sized,
            packages: ["base", "physics"],
            input: r"\matrixel**{n}{\hat{x}}{m}",
            expected: r"\left\langle n \middle\vert \hat{x} \middle\vert m \right\rangle",
        },
        ]
    }
    // END: Generated examples
}
