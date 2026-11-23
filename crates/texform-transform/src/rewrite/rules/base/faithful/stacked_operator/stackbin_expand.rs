//! Expand stackbin to an explicit binary-level stacked operator form.
//!
//! Accepts both standard LaTeX `\stackbin{#1}{#2}` and the `stackrel.sty`
//! (Oberdiek) optional subscript syntax `\stackbin[#1]{#2}{#3}` (`o m m`),
//! desugaring to standard TeX/MathJax `\mathop{...}\limits_{...}^{...}` that
//! renders without loading `stackrel.sty`. An absent or empty optional
//! subscript `[]` omits the subscript, matching `stackrel.sty`'s `\ifx...\@empty`
//! check.
//!
//! ```yaml
//! proposal: stackbin-expand
//! triggers:
//!   - cmd:stackbin
//! consumes:
//!   eliminates: cmd:stackbin
//!   touches: null
//! produces:
//!   - cmd:mathbin
//!   - cmd:mathop
//!   - cmd:limits
//! rewrite_patterns:
//!   - {label: standard, from: '\stackbin{#1}{#2}', to: '\mathbin{\mathop{#2}\limits^{#1}}'}
//!   - {label: optional-below, from: '\stackbin[#1]{#2}{#3}', to: '\mathbin{\mathop{#3}\limits_{#1}^{#2}}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::{is_empty_math_group, stacked_operator_command};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static STACKBIN_EXPAND: StackbinExpandRule {
        key: Base / "stackbin-expand",
        level: Faithful,
        summary: "Expand stackbin to an explicit binary-level stacked operator form.",
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::STACKBIN],
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
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 3, r"\stackbin")?;
            let below = cx
                .for_rule(Self::KEY)
                .optional_math_content(&command.args[0], r"\stackbin", "below")?;
            let above = cx
                .for_rule(Self::KEY)
                .mandatory_math_content(&command.args[1], r"\stackbin", "above")?;
            let operator = cx
                .for_rule(Self::KEY)
                .mandatory_math_content(&command.args[2], r"\stackbin", "operator")?;
            let below = below
                .filter(|&id| !is_empty_math_group(cx.ast, id))
                .map(|id| cx.ast.clone_subtree(id));
            let above = cx.ast.clone_subtree(above);
            let operator = cx.ast.clone_subtree(operator);
            let replacement =
                stacked_operator_command(cx, &base::cmd::MATHBIN, operator, above, below);

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
        rule: STACKBIN_EXPAND,
        level: Faithful,
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
        level: Faithful,
        examples: [
        {
            label: stackbin_preserves_compound_operator,
            packages: ["base"],
            input: r"A\stackbin{*}{\circ\!\circ}B",
            expected: r"A\mathbin{\mathop{\circ\!\circ}\limits^{*}}B",
        },
        {
            label: stackbin_optional_below_subscript,
            packages: ["base"],
            input: r"\stackbin[x]{y}{\otimes}",
            expected: r"\mathbin{\mathop{\otimes}\limits_{x}^{y}}",
        },
        {
            label: stackbin_empty_optional_below_omits_subscript,
            packages: ["base"],
            input: r"A\stackbin[]{y}{\otimes}B",
            expected: r"A\mathbin{\mathop{\otimes}\limits^{y}}B",
        },
        ]
    }
}
