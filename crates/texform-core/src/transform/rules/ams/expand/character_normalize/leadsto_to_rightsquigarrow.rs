//! Normalize leadsto to the more common rightsquigarrow corpus form.
//!
//! ```yaml
//! proposal: leadsto-to-rightsquigarrow
//! triggers:
//!   - char:leadsto
//! consumes:
//!   eliminates: char:leadsto
//!   touches: null
//! produces: char:rightsquigarrow
//! rewrite_patterns:
//!   - {from: \leadsto, to: \rightsquigarrow}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static LEADSTO_TO_RIGHTSQUIGARROW: LeadstoToRightsquigarrowRule {
        key: Ams / "leadsto-to-rightsquigarrow",
        class: Expand,
        summary: "Normalize leadsto to the more common rightsquigarrow corpus form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::LEADSTO],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::LEADSTO],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::RIGHTSQUIGARROW],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::LEADSTO.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::RIGHTSQUIGARROW.name));
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
        rule: LEADSTO_TO_RIGHTSQUIGARROW,
        class: Expand,
        examples: [
        {
            label: leadsto_character_alias,
            packages: ["base", "ams"],
            input: r"A \leadsto B",
            expected: r"A \rightsquigarrow B",
        },
        ]
    }
    // END: Generated examples
}
