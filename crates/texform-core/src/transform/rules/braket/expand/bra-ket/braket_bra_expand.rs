//! Expand bra to an explicit angle-bracket and bar fence form.
//!
//! ```yaml
//! proposal: braket-bra-expand
//! consumes:
//!   eliminates: cmd:bra
//!   touches: null
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {from: '\bra{#1}', to: '\left\langle #1 \right|'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::braket;

use super::helpers::{replace_with_fixed_bra, required_math_arg};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand bra to an explicit angle-bracket and bar fence form.
    pub static BRAKET_BRA_EXPAND: BraketBraExpandRule {
        key: Braket / "braket-bra-expand",
        tier: Expand,
        summary: "Expand bra to an explicit angle-bracket and bar fence form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Braket],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&braket::cmd::BRA],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &braket::cmd::BRA) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();
            cx.expect_arg_len(rule.meta().key, &args, 1, &subject)?;
            let body = required_math_arg(rule.meta().key, cx, &args[0], &subject, "body")?;
            replace_with_fixed_bra(cx, node_id, body);
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
        rule: BRAKET_BRA_EXPAND,
        tier: Expand,
        examples: [
        {
            label: bra_braket_package,
            packages: ["base", "braket"],
            input: r"\bra{\phi}A",
            expected: r"\langle \phi \vert A",
        },
        ]
    }
    // END: Generated examples
}
