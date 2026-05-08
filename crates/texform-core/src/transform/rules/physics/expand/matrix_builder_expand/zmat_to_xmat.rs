//! Expand zmat to the explicit xmat zero-filled builder.
//!
//! ```yaml
//! proposal: zmat-to-xmat
//! consumes:
//!   eliminates: cmd:zmat
//!   touches: null
//! produces: cmd:xmat
//! rewrite_patterns:
//!   - {from: '\zmat{#1}{#2}', to: '\xmat{0}{#1}{#2}'}
//! ```

use texform_specs::builtin::physics;

use crate::ast::{ArgumentSlot, ContentMode, Node};
use crate::transform::helpers::{mandatory_content, prefix_command, star};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::rule_context::RuleContext;
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand zmat to the explicit xmat zero-filled builder.
    pub static ZMAT_TO_XMAT: ZmatToXmatRule {
        key: Physics / "zmat-to-xmat",
        tier: Expand,
        summary: "Expand zmat to the explicit xmat zero-filled builder.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::ZMAT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::XMAT],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::ZMAT) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\{}", command.name);
            let args = command.args.to_vec();

            cx.expect_arg_len(rule.meta().key, &args, 2, &subject)?;
            let mut xmat_args = vec![star(false), zero_math_arg(cx)];
            xmat_args.extend(args);

            cx.ast
                .replace_node(node_id, prefix_command(&physics::cmd::XMAT, xmat_args));
            Ok(RuleEffect::Applied)
        }
    }
}

fn zero_math_arg(cx: &mut RuleContext<'_>) -> ArgumentSlot {
    let zero = cx.ast.new_node(Node::Char('0'));
    mandatory_content(zero, ContentMode::Math)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ZMAT_TO_XMAT,
        tier: Expand,
        examples: [
        {
            label: zmat_zero_matrix,
            packages: ["base", "physics"],
            input: r"Z=\smqty(\zmat{2}{3})",
            expected: r"Z=\smqty(\xmat{0}{2}{3})",
        },
        ]
    }
    // END: Generated examples
}
