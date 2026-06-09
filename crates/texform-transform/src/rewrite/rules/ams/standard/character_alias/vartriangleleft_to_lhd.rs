//! Collapse vartriangleleft to the more common lhd character.
//!
//! ```yaml
//! proposal: vartriangleleft-to-lhd
//! triggers:
//!   - char:vartriangleleft
//! consumes:
//!   eliminates: char:vartriangleleft
//!   touches: null
//! produces: char:lhd
//! rewrite_patterns:
//!   - {from: \vartriangleleft, to: \lhd}
//! ```

use texform_knowledge::builtin::ams;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static VARTRIANGLELEFT_TO_LHD: VartriangleleftToLhdRule {
        key: Ams / "vartriangleleft-to-lhd",
        level: Standard,
        summary: "Collapse vartriangleleft to the more common lhd character.",
        fidelity: Full,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::VARTRIANGLELEFT],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::VARTRIANGLELEFT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::LHD],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::VARTRIANGLELEFT.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::LHD.name));
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
        rule: VARTRIANGLELEFT_TO_LHD,
        level: Standard,
        examples: [
        {
            label: vartriangleleft_character_alias,
            packages: ["base", "ams"],
            input: r"A \vartriangleleft B",
            expected: r"A \lhd B",
        },
        ]
    }
    // END: Generated examples
}
