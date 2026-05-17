//! Collapse vartriangleright to the more common rhd character.
//!
//! ```yaml
//! proposal: vartriangleright-to-rhd
//! triggers:
//!   - char:vartriangleright
//! consumes:
//!   eliminates: char:vartriangleright
//!   touches: null
//! produces: char:rhd
//! rewrite_patterns:
//!   - {from: \vartriangleright, to: \rhd}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static VARTRIANGLERIGHT_TO_RHD: VartrianglerightToRhdRule {
        key: Ams / "vartriangleright-to-rhd",
        class: Standard,
        summary: "Collapse vartriangleright to the more common rhd character.",
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::VARTRIANGLERIGHT],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::VARTRIANGLERIGHT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::RHD],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::VARTRIANGLERIGHT.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::RHD.name));
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
        rule: VARTRIANGLERIGHT_TO_RHD,
        class: Standard,
        examples: [
        {
            label: vartriangleright_character_alias,
            packages: ["base", "ams"],
            input: r"A \vartriangleright B",
            expected: r"A \rhd B",
        },
        ]
    }
    // END: Generated examples
}
