//! Collapse ne to the explicit neq relation character.
//!
//! ```yaml
//! proposal: ne-to-neq
//! triggers:
//!   - char:ne
//! consumes:
//!   eliminates: char:ne
//!   touches: null
//! produces: char:neq
//! rewrite_patterns:
//!   - {from: \ne, to: \neq}
//! ```

use texform_specs::builtin::base;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static NE_TO_NEQ: NeToNeqRule {
        key: Base / "ne-to-neq",
        class: Standard,
        summary: "Collapse ne to the explicit neq relation character.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::NE],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::NE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::NEQ],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::NE.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::NEQ.name));
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
        rule: NE_TO_NEQ,
        class: Standard,
        examples: [
        {
            label: ne_character_alias,
            packages: ["base"],
            input: r"A \ne B",
            expected: r"A \neq B",
        },
        ]
    }
    // END: Generated examples
}
