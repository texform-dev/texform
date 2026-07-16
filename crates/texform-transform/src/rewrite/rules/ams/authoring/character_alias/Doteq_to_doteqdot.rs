//! Collapse Doteq to the descriptive doteqdot character.
//!
//! ```yaml
//! proposal: Doteq-to-doteqdot
//! triggers:
//!   - char:Doteq
//! consumes:
//!   eliminates: char:Doteq
//!   touches: null
//! produces: char:doteqdot
//! rewrite_patterns:
//!   - {from: \Doteq, to: \doteqdot}
//! ```

use texform_knowledge::builtin::ams;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static DOTEQ_TO_DOTEQDOT: DoteqToDoteqdotRule {
        key: Ams / "Doteq-to-doteqdot",
        level: Authoring,
        summary: "Collapse Doteq to the descriptive doteqdot character.",
        fidelity: Render,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::DOTEQ],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::DOTEQ],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::DOTEQDOT],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::DOTEQ.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::DOTEQDOT.name));
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
        rule: DOTEQ_TO_DOTEQDOT,
        level: Authoring,
        examples: [
        {
            label: doteq_character_alias,
            packages: ["base", "ams"],
            input: r"A \Doteq B",
            expected: r"A \doteqdot B",
        },
        ]
    }
    // END: Generated examples
}
