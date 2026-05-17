//! Expand stackrel to an explicit relation-class stacked operator form.
//!
//! ```yaml
//! proposal: stackrel-expand
//! triggers:
//!   - cmd:stackrel
//! consumes:
//!   eliminates: cmd:stackrel
//!   touches: null
//! produces:
//!   - cmd:mathrel
//!   - cmd:mathop
//!   - cmd:limits
//! rewrite_patterns:
//!   - {from: '\stackrel{#1}{#2}', to: '\mathrel{\mathop{#2}\limits^{#1}}'}
//! ```

use texform_specs::builtin::base;

use super::helpers::stacked_operator_command;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static STACKREL_EXPAND: StackrelExpandRule {
        key: Base / "stackrel-expand",
        class: Expand,
        summary: "Expand stackrel to an explicit relation-class stacked operator form.",
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::STACKREL],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::STACKREL],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::MATHREL, &base::cmd::MATHOP, &base::cmd::LIMITS],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::STACKREL) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 2, r"\stackrel")?;
            let above = cx.for_rule(Self::KEY).mandatory_math_content(&command.args[0], r"\stackrel", "above")?;
            let operator = cx.for_rule(Self::KEY).mandatory_math_content(&command.args[1], r"\stackrel", "operator")?;
            let above = cx.ast.clone_subtree(above);
            let operator = cx.ast.clone_subtree(operator);
            let replacement = stacked_operator_command(cx, &base::cmd::MATHREL, operator, above);

            cx.ast.replace_node_drop_detached_children(node_id, replacement);
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
        rule: STACKREL_EXPAND,
        class: Expand,
        examples: [
        {
            label: stackrel_relation_context,
            packages: ["base"],
            input: r"a\stackrel{?}{\le}b",
            expected: r"a\mathrel{\mathop{\le}\limits^{?}}b",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: STACKREL_EXPAND,
        class: Expand,
        examples: [
        {
            label: stackrel_preserves_grouped_above_content,
            packages: ["base"],
            input: r"x\stackrel{a+b}{\sim}y",
            expected: r"x\mathrel{\mathop{\sim}\limits^{a+b}}y",
        },
        ]
    }
}
