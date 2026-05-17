//! Normalize gets to leftarrow for corpus form.
//!
//! ```yaml
//! proposal: gets-to-leftarrow
//! triggers:
//!   - char:gets
//! consumes:
//!   eliminates: char:gets
//!   touches: null
//! produces: char:leftarrow
//! rewrite_patterns:
//!   - {from: \gets, to: \leftarrow}
//! ```

use texform_specs::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static GETS_TO_LEFTARROW: GetsToLeftarrowRule {
        key: Base / "gets-to-leftarrow",
        class: Expand,
        summary: "Normalize gets to leftarrow for corpus form.",
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::GETS],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::GETS],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::LEFTARROW],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::GETS.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::LEFTARROW.name));
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
        rule: GETS_TO_LEFTARROW,
        class: Expand,
        examples: [
        {
            label: gets_character_alias,
            packages: ["base"],
            input: r"A \gets B",
            expected: r"A \leftarrow B",
        },
        ]
    }
    // END: Generated examples
}
