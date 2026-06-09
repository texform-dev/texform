//! Collapse ge to the explicit geq relation character.
//!
//! ```yaml
//! proposal: ge-to-geq
//! triggers:
//!   - char:ge
//! consumes:
//!   eliminates: char:ge
//!   touches: null
//! produces: char:geq
//! rewrite_patterns:
//!   - {from: \ge, to: \geq}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static GE_TO_GEQ: GeToGeqRule {
        key: Base / "ge-to-geq",
        level: Standard,
        summary: "Collapse ge to the explicit geq relation character.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::GE],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::GE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::GEQ],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::GE.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::GEQ.name));
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
        rule: GE_TO_GEQ,
        level: Standard,
        examples: [
        {
            label: ge_character_alias,
            packages: ["base"],
            input: r"A \ge B",
            expected: r"A \geq B",
        },
        ]
    }
    // END: Generated examples
}
