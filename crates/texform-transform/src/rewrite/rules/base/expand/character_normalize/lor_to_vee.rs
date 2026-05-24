//! Normalize lor to vee for corpus form.
//!
//! ```yaml
//! proposal: lor-to-vee
//! triggers:
//!   - char:lor
//! consumes:
//!   eliminates: char:lor
//!   touches: null
//! produces: char:vee
//! rewrite_patterns:
//!   - {from: \lor, to: \vee}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static LOR_TO_VEE: LorToVeeRule {
        key: Base / "lor-to-vee",
        class: Expand,
        summary: "Normalize lor to vee for corpus form.",
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::LOR],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::LOR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::VEE],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::LOR.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::VEE.name));
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
        rule: LOR_TO_VEE,
        class: Expand,
        examples: [
        {
            label: lor_character_alias,
            packages: ["base"],
            input: r"A \lor B",
            expected: r"A \vee B",
        },
        ]
    }
    // END: Generated examples
}
