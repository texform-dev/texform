//! Collapse llless to the shorter lll character.
//!
//! ```yaml
//! proposal: llless-to-lll
//! triggers:
//!   - char:llless
//! consumes:
//!   eliminates: char:llless
//!   touches: null
//! produces: char:lll
//! rewrite_patterns:
//!   - {from: \llless, to: \lll}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static LLLESS_TO_LLL: LllessToLllRule {
        key: Ams / "llless-to-lll",
        class: Standard,
        summary: "Collapse llless to the shorter lll character.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::LLLESS],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::LLLESS],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::LLL],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::LLLESS.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::LLL.name));
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
        rule: LLLESS_TO_LLL,
        class: Standard,
        examples: [
        {
            label: llless_character_alias,
            packages: ["base", "ams"],
            input: r"A \llless B",
            expected: r"A \lll B",
        },
        ]
    }
    // END: Generated examples
}
