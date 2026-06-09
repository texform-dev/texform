//! Collapse gggtr to the shorter ggg character.
//!
//! ```yaml
//! proposal: gggtr-to-ggg
//! triggers:
//!   - char:gggtr
//! consumes:
//!   eliminates: char:gggtr
//!   touches: null
//! produces: char:ggg
//! rewrite_patterns:
//!   - {from: \gggtr, to: \ggg}
//! ```

use texform_knowledge::builtin::ams;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static GGGTR_TO_GGG: GggtrToGggRule {
        key: Ams / "gggtr-to-ggg",
        level: Standard,
        summary: "Collapse gggtr to the shorter ggg character.",
        fidelity: Full,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::GGGTR],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::GGGTR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::GGG],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::GGGTR.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::GGG.name));
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
        rule: GGGTR_TO_GGG,
        level: Standard,
        examples: [
        {
            label: gggtr_character_alias,
            packages: ["base", "ams"],
            input: r"A \gggtr B",
            expected: r"A \ggg B",
        },
        ]
    }
    // END: Generated examples
}
