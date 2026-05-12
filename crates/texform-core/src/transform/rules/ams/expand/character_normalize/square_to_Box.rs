//! Normalize square to Box for corpus form.
//!
//! ```yaml
//! proposal: square-to-Box
//! triggers:
//!   - char:square
//! consumes:
//!   eliminates: char:square
//!   touches: null
//! produces: char:Box
//! rewrite_patterns:
//!   - {from: \square, to: \Box}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static SQUARE_TO_BOX: SquareToBoxRule {
        key: Ams / "square-to-Box",
        class: Expand,
        summary: "Normalize square to Box for corpus form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::SQUARE],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::SQUARE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::BOX],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::SQUARE.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::BOX.name));
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
        rule: SQUARE_TO_BOX,
        class: Expand,
        examples: [
        {
            label: square_character_alias,
            packages: ["base", "ams"],
            input: r"A \square B",
            expected: r"A \Box B",
        },
        ]
    }
    // END: Generated examples
}
