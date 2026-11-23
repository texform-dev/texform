//! Expand stackrel to an explicit relation-level stacked operator form.
//!
//! Accepts both standard LaTeX2e `\stackrel{#1}{#2}` and the `stackrel.sty`
//! (Oberdiek) optional subscript syntax `\stackrel[#1]{#2}{#3}` (`o m m`),
//! desugaring to standard TeX/MathJax `\mathop{...}\limits_{...}^{...}` that
//! renders without loading `stackrel.sty`. An absent or empty optional
//! subscript `[]` omits the subscript, matching `stackrel.sty`'s `\ifx...\@empty`
//! check.
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
//!   - {label: standard, from: '\stackrel{#1}{#2}', to: '\mathrel{\mathop{#2}\limits^{#1}}'}
//!   - {label: optional-below, from: '\stackrel[#1]{#2}{#3}', to: '\mathrel{\mathop{#3}\limits_{#1}^{#2}}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::{is_empty_math_group, stacked_operator_command};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static STACKREL_EXPAND: StackrelExpandRule {
        key: Base / "stackrel-expand",
        level: Faithful,
        summary: "Expand stackrel to an explicit relation-level stacked operator form.",
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
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 3, r"\stackrel")?;
            let below = cx
                .for_rule(Self::KEY)
                .optional_math_content(&command.args[0], r"\stackrel", "below")?;
            let above = cx
                .for_rule(Self::KEY)
                .mandatory_math_content(&command.args[1], r"\stackrel", "above")?;
            let operator = cx
                .for_rule(Self::KEY)
                .mandatory_math_content(&command.args[2], r"\stackrel", "operator")?;
            let below = below
                .filter(|&id| !is_empty_math_group(cx.ast, id))
                .map(|id| cx.ast.clone_subtree(id));
            let above = cx.ast.clone_subtree(above);
            let operator = cx.ast.clone_subtree(operator);
            let replacement =
                stacked_operator_command(cx, &base::cmd::MATHREL, operator, above, below);

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
        level: Faithful,
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
        level: Faithful,
        examples: [
        {
            label: stackrel_preserves_grouped_above_content,
            packages: ["base"],
            input: r"x\stackrel{a+b}{\sim}y",
            expected: r"x\mathrel{\mathop{\sim}\limits^{a+b}}y",
        },
        {
            label: stackrel_optional_below_subscript,
            packages: ["base"],
            input: r"\stackrel[m=1]{M}{\sum}",
            expected: r"\mathrel{\mathop{\sum}\limits_{m=1}^{M}}",
        },
        {
            label: stackrel_optional_below_with_leading_space,
            packages: ["base"],
            input: r"\stackrel [ m = 1 ] { M } { \sum }",
            expected: r"\mathrel{\mathop{\sum}\limits_{m=1}^{M}}",
        },
        {
            label: stackrel_empty_optional_below_omits_subscript,
            packages: ["base"],
            input: r"\stackrel[]{a}{b}",
            expected: r"\mathrel{\mathop{b}\limits^{a}}",
        },
        {
            label: stackrel_following_bracket_stays_sibling,
            packages: ["base"],
            input: r"\stackrel{a}{b}[x]",
            expected: r"\mathrel{\mathop{b}\limits^{a}}[x]",
        },
        ]
    }
}
