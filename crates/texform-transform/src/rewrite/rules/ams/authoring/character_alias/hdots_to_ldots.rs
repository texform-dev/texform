//! Collapse the amsmath hdots character to ldots.
//!
//! ```yaml
//! proposal: hdots-to-ldots
//! triggers:
//!   - char:hdots
//! consumes:
//!   eliminates: char:hdots
//!   touches: null
//! produces: char:ldots
//! rewrite_patterns:
//!   - {from: \hdots, to: \ldots}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static HDOTS_TO_LDOTS: HdotsToLdotsRule {
        key: Ams / "hdots-to-ldots",
        level: Authoring,
        summary: "Collapse the amsmath hdots character to ldots.",
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::HDOTS],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::HDOTS],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::LDOTS],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::HDOTS.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::LDOTS.name));
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
        rule: HDOTS_TO_LDOTS,
        level: Authoring,
        examples: [
        {
            label: hdots_character_alias,
            packages: ["base", "ams"],
            input: r"a_1, \hdots, a_n",
            expected: r"a_1, \ldots, a_n",
        },
        ]
    }
    // END: Generated examples
}
