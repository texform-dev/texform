//! Rewrite plain TeX buildrel syntax to stackrel.
//!
//! ```yaml
//! proposal: buildrel-to-stackrel
//! triggers:
//!   - cmd:buildrel
//! consumes:
//!   eliminates: cmd:buildrel
//!   touches: null
//! produces: cmd:stackrel
//! rewrite_patterns:
//!   - {label: operator, from: '\buildrel #1 \over #2', to: '\stackrel{#1}{#2}'}
//!   - {label: operator-external-scripts, from: '\buildrel #1 \over #2_{#3}^{#4}', to: '\stackrel{#1}{#2}_{#3}^{#4}'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter};
use crate::rewrite::helpers::{mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BUILDREL_TO_STACKREL: BuildrelToStackrelRule {
        key: Base / "buildrel-to-stackrel",
        level: Authoring,
        summary: "Rewrite plain TeX buildrel syntax to stackrel.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::BUILDREL],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BUILDREL],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::STACKREL],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::BUILDREL) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 2, r"\buildrel")?;
            let above = match &command.args[0] {
                Some(Argument {
                    kind: ArgumentKind::Until { close: Delimiter::Control(name) },
                    value: ArgumentValue::MathContent(above),
                    ..
                }) if name == "over" => *above,
                _ => return Err(cx.for_rule(Self::KEY).invalid_shape(r"\buildrel above content should end at \over")),
            };
            let operator = cx.for_rule(Self::KEY).mandatory_math_content(&command.args[1], r"\buildrel", "operator")?;

            cx.ast.replace_node(
                node_id,
                prefix_command_node(
                    &base::cmd::STACKREL,
                    vec![
                        mandatory_content_slot(above, ContentMode::Math),
                        mandatory_content_slot(operator, ContentMode::Math),
                    ],
                ),
            );
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
        rule: BUILDREL_TO_STACKREL,
        level: Authoring,
        examples: [
        {
            label: asymptotic_relation_stack,
            packages: ["base"],
            input: r"A_n \buildrel n\to\infty \over = B_n",
            expected: r"A_n \stackrel{n\to\infty}{=} B_n",
        },
        {
            label: arrow_relation_stack,
            packages: ["base"],
            input: r"X \buildrel \phi \over \longrightarrow Y",
            expected: r"X \stackrel{\phi}{\longrightarrow} Y",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: BUILDREL_TO_STACKREL,
        level: Authoring,
        examples: [
        {
            label: braced_over_stays_in_above_content,
            packages: ["base"],
            input: r"\buildrel{a\over b}\over=",
            expected: r"\stackrel{a\over b}{=}",
        },
        {
            label: buildrel_on_the_right_of_an_outer_over,
            packages: ["base"],
            input: r"a\over\buildrel b\over c",
            expected: r"a\over\stackrel{b}{c}",
        },
        {
            label: following_scripts_attach_to_the_relation,
            packages: ["base"],
            input: r"\buildrel x\over =^2",
            expected: r"\stackrel{x}{=}^2",
        },
        {
            label: unbraced_delimited_operator_keeps_its_delimiters,
            packages: ["base"],
            input: r"\buildrel a\over\left(x\right)",
            expected: r"\stackrel{a}{\left(x\right)}",
        },
        {
            label: grouped_operator_keeps_its_scripts,
            packages: ["base"],
            input: r"\buildrel x\over{=^2}",
            expected: r"\stackrel{x}{=^2}",
        },
        {
            label: buildrel_inside_a_command_argument,
            packages: ["base"],
            input: r"\frac{\buildrel a\over =}{b}",
            expected: r"\frac{\stackrel{a}{=}}{b}",
        },
        {
            label: repeated_buildrel_keeps_chain_order,
            packages: ["base"],
            input: r"K\buildrel f\over\longrightarrow K\buildrel g\over\longrightarrow K",
            expected: r"K\stackrel{f}{\longrightarrow} K\stackrel{g}{\longrightarrow} K",
        },
        ]
    }
}
