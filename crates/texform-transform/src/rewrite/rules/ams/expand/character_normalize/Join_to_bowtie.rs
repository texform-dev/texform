//! Normalize Join to bowtie for corpus form.
//!
//! ```yaml
//! proposal: Join-to-bowtie
//! triggers:
//!   - char:Join
//! consumes:
//!   eliminates: char:Join
//!   touches: null
//! produces: char:bowtie
//! rewrite_patterns:
//!   - {from: \Join, to: \bowtie}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static JOIN_TO_BOWTIE: JoinToBowtieRule {
        key: Ams / "Join-to-bowtie",
        level: Expand,
        summary: "Normalize Join to bowtie for corpus form.",
        fidelity: Full,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::JOIN],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::JOIN],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::BOWTIE],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::JOIN.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::BOWTIE.name));
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
        rule: JOIN_TO_BOWTIE,
        level: Expand,
        examples: [
        {
            label: join_character_alias,
            packages: ["base", "ams"],
            input: r"A \Join B",
            expected: r"A \bowtie B",
        },
        ]
    }
    // END: Generated examples
}
