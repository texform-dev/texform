//! Expand braket to an explicit angle-bracket form with a middle bar.
//!
//! ```yaml
//! proposal: braket-braket-expand
//! triggers:
//!   - cmd:braket
//! consumes:
//!   eliminates: cmd:braket
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\braket{#1}', to: '\left\langle #1 \right\rangle'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::braket;

use super::helpers::{replace_with_braket, required_math_arg};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static BRAKET_BRAKET_EXPAND: BraketBraketExpandRule {
        key: Braket / "braket-braket-expand",
        class: Expand,
        summary: "Expand braket to an explicit angle-bracket form with a middle bar.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Braket],
        triggers: cmd_targets![&braket::cmd::BRAKET],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&braket::cmd::BRAKET],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &braket::cmd::BRAKET) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();
            cx.for_rule(Self::KEY).expect_arg_len(&args, 1, &subject)?;
            let body = required_math_arg(Self::KEY, cx, &args[0], &subject, "body")?;
            replace_with_braket(cx, node_id, body);
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
        rule: BRAKET_BRAKET_EXPAND,
        class: Expand,
        examples: [
        {
            label: braket_package_single_argument,
            packages: ["base", "braket"],
            input: r"\braket{u|v}=0",
            expected: r"\left\langle u\middle\vert v \right\rangle=0",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: BRAKET_BRAKET_EXPAND,
        class: Expand,
        examples: [
        {
            label: braket_package_overlap_without_bar,
            packages: ["base", "braket"],
            input: r"\braket{u}=1",
            expected: r"\left\langle u \right\rangle=1",
        },
        ]
    }
}
