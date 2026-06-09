//! Normalize AMS centerdot to the base cdot character for corpus form.
//!
//! ```yaml
//! proposal: centerdot-to-cdot
//! triggers:
//!   - char:centerdot
//! consumes:
//!   eliminates: char:centerdot
//!   touches: null
//! produces: char:cdot
//! rewrite_patterns:
//!   - {from: \centerdot, to: \cdot}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static CENTERDOT_TO_CDOT: CenterdotToCdotRule {
        key: Ams / "centerdot-to-cdot",
        level: Expand,
        summary: "Normalize AMS centerdot to the base cdot character for corpus form.",
        fidelity: Full,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::CENTERDOT],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::CENTERDOT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::CDOT],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::CENTERDOT.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::CDOT.name));
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
        rule: CENTERDOT_TO_CDOT,
        level: Expand,
        examples: [
        {
            label: centerdot_character_alias,
            packages: ["base", "ams"],
            input: r"A \centerdot B",
            expected: r"A \cdot B",
        },
        {
            label: centerdot_script_position,
            packages: ["base", "ams"],
            input: r"x_{\centerdot}",
            expected: r"x_{\cdot}",
        },
        ]
    }
    // END: Generated examples
}
