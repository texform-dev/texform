//! Collapse dotproduct to the shorter vdot character alias.
//!
//! ```yaml
//! proposal: dotproduct-to-vdot
//! triggers:
//!   - char:dotproduct
//! consumes:
//!   eliminates: char:dotproduct
//!   touches: null
//! produces: char:vdot
//! rewrite_patterns:
//!   - {from: \dotproduct, to: \vdot}
//! ```

use texform_knowledge::builtin::physics;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static DOTPRODUCT_TO_VDOT: DotproductToVdotRule {
        key: Physics / "dotproduct-to-vdot",
        class: Standard,
        summary: "Collapse dotproduct to the shorter vdot character alias.",
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: char_targets![&physics::chars::DOTPRODUCT],
        consumes: RuleConsumes {
            eliminates: char_targets![&physics::chars::DOTPRODUCT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&physics::chars::VDOT],
        },
        apply(rule, cx, node_id) {
            let alias_names = [physics::chars::DOTPRODUCT.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(physics::chars::VDOT.name));
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
        rule: DOTPRODUCT_TO_VDOT,
        class: Standard,
        examples: [
        {
            label: dotproduct_character_alias,
            packages: ["base", "physics"],
            input: r"A \dotproduct B",
            expected: r"A \vdot B",
        },
        ]
    }
    // END: Generated examples
}
