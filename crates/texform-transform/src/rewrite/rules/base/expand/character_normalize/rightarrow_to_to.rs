//! Normalize rightarrow to the shorter to arrow character for corpus form.
//!
//! ```yaml
//! proposal: rightarrow-to-to
//! triggers:
//!   - char:rightarrow
//! consumes:
//!   eliminates: char:rightarrow
//!   touches: null
//! produces: char:to
//! rewrite_patterns:
//!   - {from: \rightarrow, to: \to}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static RIGHTARROW_TO_TO: RightarrowToToRule {
        key: Base / "rightarrow-to-to",
        class: Expand,
        summary: "Normalize rightarrow to the shorter to arrow character for corpus form.",
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::RIGHTARROW],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::RIGHTARROW],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::TO],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::RIGHTARROW.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::TO.name));
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
        rule: RIGHTARROW_TO_TO,
        class: Expand,
        examples: [
        {
            label: rightarrow_character_alias,
            packages: ["base"],
            input: r"A \rightarrow B",
            expected: r"A \to B",
        },
        ]
    }
    // END: Generated examples
}
