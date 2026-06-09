//! Collapse le to the explicit leq relation character.
//!
//! ```yaml
//! proposal: le-to-leq
//! triggers:
//!   - char:le
//! consumes:
//!   eliminates: char:le
//!   touches: null
//! produces: char:leq
//! rewrite_patterns:
//!   - {from: \le, to: \leq}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static LE_TO_LEQ: LeToLeqRule {
        key: Base / "le-to-leq",
        level: Standard,
        summary: "Collapse le to the explicit leq relation character.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::LE],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::LE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::LEQ],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::LE.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::LEQ.name));
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
        rule: LE_TO_LEQ,
        level: Standard,
        examples: [
        {
            label: le_character_alias,
            packages: ["base"],
            input: r"A \le B",
            expected: r"A \leq B",
        },
        ]
    }
    // END: Generated examples
}
