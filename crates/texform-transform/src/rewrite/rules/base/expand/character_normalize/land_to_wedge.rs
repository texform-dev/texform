//! Normalize land to wedge for corpus form.
//!
//! ```yaml
//! proposal: land-to-wedge
//! triggers:
//!   - char:land
//! consumes:
//!   eliminates: char:land
//!   touches: null
//! produces: char:wedge
//! rewrite_patterns:
//!   - {from: \land, to: \wedge}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static LAND_TO_WEDGE: LandToWedgeRule {
        key: Base / "land-to-wedge",
        level: Expand,
        summary: "Normalize land to wedge for corpus form.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::LAND],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::LAND],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::WEDGE],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::LAND.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::WEDGE.name));
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
        rule: LAND_TO_WEDGE,
        level: Expand,
        examples: [
        {
            label: land_character_alias,
            packages: ["base"],
            input: r"A \land B",
            expected: r"A \wedge B",
        },
        ]
    }
    // END: Generated examples
}
