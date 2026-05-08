//! Expand stackbin to an explicit binary-class stacked operator form.
//!
//! ```yaml
//! proposal: stackbin-expand
//! consumes:
//!   eliminates: cmd:stackbin
//!   touches: null
//! produces:
//!   - cmd:mathbin
//!   - cmd:mathop
//!   - cmd:limits
//! rewrite_patterns:
//!   - {from: '\stackbin{#1}{#2}', to: '\mathbin{\mathop{#2}\limits^{#1}}'}
//! ```

use texform_specs::builtin::base;

use super::helpers::stacked_operator_command;
use crate::transform::helpers::{replace_node_discarding_detached_children, required_math_content};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand stackbin to an explicit binary-class stacked operator form.
    pub static STACKBIN_EXPAND: StackbinExpandRule {
        key: Base / "stackbin-expand",
        tier: Expand,
        summary: "Expand stackbin to an explicit binary-class stacked operator form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::STACKBIN],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::MATHBIN, &base::cmd::MATHOP, &base::cmd::LIMITS],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::STACKBIN) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_arg_len(rule.meta().key, command.args, 2, r"\stackbin")?;
            let above = required_math_content(rule.meta().key, cx, &command.args[0], r"\stackbin", "above")?;
            let operator = required_math_content(rule.meta().key, cx, &command.args[1], r"\stackbin", "operator")?;
            let above = cx.ast.clone_subtree(above);
            let operator = cx.ast.clone_subtree(operator);
            let replacement = stacked_operator_command(cx, &base::cmd::MATHBIN, operator, above);

            replace_node_discarding_detached_children(cx, node_id, replacement);
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
        rule: STACKBIN_EXPAND,
        tier: Expand,
        examples: [
        {
            label: stackbin_binary_context,
            packages: ["base"],
            input: r"A\stackbin{\circ}{\otimes}B",
            expected: r"A\mathbin{\mathop{\otimes}\limits^{\circ}}B",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: STACKBIN_EXPAND,
        tier: Expand,
        examples: [
        {
            label: stackbin_preserves_compound_operator,
            packages: ["base"],
            input: r"A\stackbin{*}{\circ\!\circ}B",
            expected: r"A\mathbin{\mathop{\circ\!\circ}\limits^{*}}B",
        },
        ]
    }
}
