//! Normalize restriction to upharpoonright for corpus form.
//!
//! ```yaml
//! proposal: restriction-to-upharpoonright
//! triggers:
//!   - char:restriction
//! consumes:
//!   eliminates: char:restriction
//!   touches: null
//! produces: char:upharpoonright
//! rewrite_patterns:
//!   - {from: \restriction, to: \upharpoonright}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static RESTRICTION_TO_UPHARPOONRIGHT: RestrictionToUpharpoonrightRule {
        key: Ams / "restriction-to-upharpoonright",
        class: Expand,
        summary: "Normalize restriction to upharpoonright for corpus form.",
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::RESTRICTION],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::RESTRICTION],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::UPHARPOONRIGHT],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::RESTRICTION.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::UPHARPOONRIGHT.name));
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
        rule: RESTRICTION_TO_UPHARPOONRIGHT,
        class: Expand,
        examples: [
        {
            label: restriction_character_alias,
            packages: ["base", "ams"],
            input: r"A \restriction B",
            expected: r"A \upharpoonright B",
        },
        ]
    }
    // END: Generated examples
}
