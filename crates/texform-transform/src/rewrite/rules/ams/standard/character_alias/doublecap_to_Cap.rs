//! Collapse doublecap to the more common Cap character.
//!
//! ```yaml
//! proposal: doublecap-to-Cap
//! triggers:
//!   - char:doublecap
//! consumes:
//!   eliminates: char:doublecap
//!   touches: null
//! produces: char:Cap
//! rewrite_patterns:
//!   - {from: \doublecap, to: \Cap}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static DOUBLECAP_TO_CAP: DoublecapToCapRule {
        key: Ams / "doublecap-to-Cap",
        class: Standard,
        summary: "Collapse doublecap to the more common Cap character.",
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::DOUBLECAP],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::DOUBLECAP],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::CAP],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::DOUBLECAP.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::CAP.name));
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
        rule: DOUBLECAP_TO_CAP,
        class: Standard,
        examples: [
        {
            label: doublecap_character_alias,
            packages: ["base", "ams"],
            input: r"A \doublecap B",
            expected: r"A \Cap B",
        },
        ]
    }
    // END: Generated examples
}
