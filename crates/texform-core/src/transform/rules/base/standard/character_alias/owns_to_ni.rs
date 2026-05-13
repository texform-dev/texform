//! Collapse owns to the shorter ni character.
//!
//! ```yaml
//! proposal: owns-to-ni
//! triggers:
//!   - char:owns
//! consumes:
//!   eliminates: char:owns
//!   touches: null
//! produces: char:ni
//! rewrite_patterns:
//!   - {from: \owns, to: \ni}
//! ```

use texform_specs::builtin::base;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static OWNS_TO_NI: OwnsToNiRule {
        key: Base / "owns-to-ni",
        class: Standard,
        summary: "Collapse owns to the shorter ni character.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::OWNS],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::OWNS],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::NI],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::OWNS.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::NI.name));
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
        rule: OWNS_TO_NI,
        class: Standard,
        examples: [
        {
            label: owns_character_alias,
            packages: ["base"],
            input: r"A \owns B",
            expected: r"A \ni B",
        },
        ]
    }
    // END: Generated examples
}
