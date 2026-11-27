//! Rewrite the LaTeX kernel \ddag to the \ddagger math character in math mode.
//!
//! ```yaml
//! proposal: ddag-to-ddagger
//! triggers:
//!   - char:ddag
//! consumes:
//!   eliminates: null
//!   touches: char:ddag
//! produces: char:ddagger
//! rewrite_patterns:
//!   - {from: \ddag, to: \ddagger}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{ContentMode, Node};
use crate::rewrite::helpers::{bare_command_node, content_mode_at};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static DDAG_TO_DDAGGER: DdagToDdaggerRule {
        key: Base / "ddag-to-ddagger",
        level: Authoring,
        summary: "Rewrite the LaTeX kernel \\ddag to the \\ddagger math character in math mode.",
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::DDAG],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: char_targets![&base::chars::DDAG],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::DDAGGER],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::DDAG.name];
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

            cx.ast.replace_node(node_id, bare_command_node(base::chars::DDAGGER.name));
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
        rule: DDAG_TO_DDAGGER,
        level: Authoring,
        examples: [
        {
            label: math_superscript,
            packages: ["base"],
            input: r"A^\ddag B",
            expected: r"A^\ddagger B",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: DDAG_TO_DDAGGER,
        level: Authoring,
        examples: [
        {
            label: text_mode_ddag_is_preserved,
            packages: ["base"],
            input: r"\text{A\ddag}",
            expected: r"\text{A\ddag}",
        },
        {
            label: textmacros_text_mode_ddag_is_preserved,
            packages: ["base", "textmacros"],
            input: r"\mbox{A\ddag B}",
            expected: r"\mbox{A\ddag B}",
        },
        {
            label: inline_math_in_text_is_rewritten,
            packages: ["base", "textmacros"],
            input: r"\text{A$\ddag$}",
            expected: r"\text{A$\ddagger$}",
        },
        {
            label: math_argument_is_rewritten,
            packages: ["base"],
            input: r"\sqrt\ddag + \frac{\ddag}{2}",
            expected: r"\sqrt\ddagger + \frac{\ddagger}{2}",
        },
        ]
    }
}
