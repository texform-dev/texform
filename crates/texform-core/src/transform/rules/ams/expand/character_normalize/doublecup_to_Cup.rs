//! Normalize doublecup to Cup for corpus form.
//!
//! ```yaml
//! proposal: doublecup-to-Cup
//! triggers:
//!   - char:doublecup
//! consumes:
//!   eliminates: char:doublecup
//!   touches: null
//! produces: char:Cup
//! rewrite_patterns:
//!   - {from: \doublecup, to: \Cup}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static DOUBLECUP_TO_CUP: DoublecupToCupRule {
        key: Ams / "doublecup-to-Cup",
        class: Expand,
        summary: "Normalize doublecup to Cup for corpus form.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::DOUBLECUP],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::DOUBLECUP],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::CUP],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::DOUBLECUP.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::CUP.name));
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DOUBLECUP_TO_CUP,
        class: Expand,
        examples: [
        {
            label: doublecup_character_alias,
            packages: ["base", "ams"],
            input: r"A \doublecup B",
            expected: r"A \Cup B",
        },
        ]
    }
    // END: Generated examples
}
