//! Collapse unlhd to the descriptive trianglelefteq character.
//!
//! ```yaml
//! proposal: unlhd-to-trianglelefteq
//! triggers:
//!   - char:unlhd
//! consumes:
//!   eliminates: char:unlhd
//!   touches: null
//! produces: char:trianglelefteq
//! rewrite_patterns:
//!   - {from: \unlhd, to: \trianglelefteq}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static UNLHD_TO_TRIANGLELEFTEQ: UnlhdToTrianglelefteqRule {
        key: Ams / "unlhd-to-trianglelefteq",
        class: Standard,
        summary: "Collapse unlhd to the descriptive trianglelefteq character.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::UNLHD],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::UNLHD],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::TRIANGLELEFTEQ],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::UNLHD.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::TRIANGLELEFTEQ.name));
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
        rule: UNLHD_TO_TRIANGLELEFTEQ,
        class: Standard,
        examples: [
        {
            label: unlhd_character_alias,
            packages: ["base", "ams"],
            input: r"A \unlhd B",
            expected: r"A \trianglelefteq B",
        },
        ]
    }
    // END: Generated examples
}
