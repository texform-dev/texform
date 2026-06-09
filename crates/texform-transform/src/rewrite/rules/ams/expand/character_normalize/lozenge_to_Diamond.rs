//! Normalize lozenge to Diamond for corpus form.
//!
//! ```yaml
//! proposal: lozenge-to-Diamond
//! triggers:
//!   - char:lozenge
//! consumes:
//!   eliminates: char:lozenge
//!   touches: null
//! produces: char:Diamond
//! rewrite_patterns:
//!   - {from: \lozenge, to: \Diamond}
//! ```

use texform_knowledge::builtin::ams;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static LOZENGE_TO_DIAMOND: LozengeToDiamondRule {
        key: Ams / "lozenge-to-Diamond",
        level: Expand,
        summary: "Normalize lozenge to Diamond for corpus form.",
        fidelity: Full,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::LOZENGE],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::LOZENGE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::DIAMOND],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::LOZENGE.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::DIAMOND.name));
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
        rule: LOZENGE_TO_DIAMOND,
        level: Expand,
        examples: [
        {
            label: lozenge_character_alias,
            packages: ["base", "ams"],
            input: r"A \lozenge B",
            expected: r"A \Diamond B",
        },
        ]
    }
    // END: Generated examples
}
