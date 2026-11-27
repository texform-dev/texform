//! Rewrite the LaTeX kernel \dag to the \dagger math character in math mode.
//!
//! ```yaml
//! proposal: dag-to-dagger
//! triggers:
//!   - char:dag
//! consumes:
//!   eliminates: null
//!   touches: char:dag
//! produces: char:dagger
//! rewrite_patterns:
//!   - {from: \dag, to: \dagger}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{ContentMode, Node};
use crate::rewrite::helpers::{bare_command_node, content_mode_at};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static DAG_TO_DAGGER: DagToDaggerRule {
        key: Base / "dag-to-dagger",
        level: Authoring,
        summary: "Rewrite the LaTeX kernel \\dag to the \\dagger math character in math mode.",
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::DAG],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: char_targets![&base::chars::DAG],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::DAGGER],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::DAG.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            // Text mode keeps the kernel command: there it prints the text-font
            // dagger, which the math-font character does not reproduce.
            if content_mode_at(cx.ast, node_id) != Some(ContentMode::Math) {
                return Ok(RuleEffect::Skipped);
            }

            cx.ast.replace_node(node_id, bare_command_node(base::chars::DAGGER.name));
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
        rule: DAG_TO_DAGGER,
        level: Authoring,
        examples: [
        {
            label: math_superscript,
            packages: ["base"],
            input: r"A^\dag B",
            expected: r"A^\dagger B",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: DAG_TO_DAGGER,
        level: Authoring,
        examples: [
        {
            label: text_mode_dag_is_preserved,
            packages: ["base"],
            input: r"\text{A\dag}",
            expected: r"\text{A\dag}",
        },
        {
            label: textmacros_text_mode_dag_is_preserved,
            packages: ["base", "textmacros"],
            input: r"\mbox{A\dag B}",
            expected: r"\mbox{A\dag B}",
        },
        {
            label: inline_math_in_text_is_rewritten,
            packages: ["base", "textmacros"],
            input: r"\text{A$\dag$}",
            expected: r"\text{A$\dagger$}",
        },
        {
            label: math_argument_is_rewritten,
            packages: ["base"],
            input: r"\sqrt\dag + \frac{\dag}{2}",
            expected: r"\sqrt\dagger + \frac{\dagger}{2}",
        },
        ]
    }
}
