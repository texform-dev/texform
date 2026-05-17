//! Expand ket to an explicit bar and angle-bracket fence form.
//!
//! ```yaml
//! proposal: braket-ket-expand
//! triggers:
//!   - cmd:ket
//! consumes:
//!   eliminates: cmd:ket
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\ket{#1}', to: '\left| #1 \right\rangle'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::braket;

use super::helpers::{replace_with_fixed_ket, required_math_arg};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BRAKET_KET_EXPAND: BraketKetExpandRule {
        key: Braket / "braket-ket-expand",
        class: Expand,
        summary: "Expand ket to an explicit bar and angle-bracket fence form.",
        safety: Lossless,
        enabled_by_packages: [Braket],
        triggers: cmd_targets![&braket::cmd::KET],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&braket::cmd::KET],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &braket::cmd::KET) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            let args = command.args.to_vec();
            cx.for_rule(Self::KEY).expect_arg_len(&args, 1, &subject)?;
            let body = required_math_arg(Self::KEY, cx, &args[0], &subject, "body")?;
            replace_with_fixed_ket(cx, node_id, body);
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
        rule: BRAKET_KET_EXPAND,
        class: Expand,
        examples: [
        {
            label: ket_braket_package,
            packages: ["base", "braket"],
            input: r"A\ket{\phi}",
            expected: r"A\vert \phi \rangle",
        },
        ]
    }
    // END: Generated examples
}
