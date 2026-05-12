//! Collapse lnot to the standard neg character.
//!
//! ```yaml
//! proposal: lnot-to-neg
//! triggers:
//!   - char:lnot
//! consumes:
//!   eliminates: char:lnot
//!   touches: null
//! produces: char:neg
//! rewrite_patterns:
//!   - {from: \lnot, to: \neg}
//! ```

use texform_specs::builtin::base;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static LNOT_TO_NEG: LnotToNegRule {
        key: Base / "lnot-to-neg",
        class: Standard,
        summary: "Collapse lnot to the standard neg character.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::LNOT],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::LNOT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::NEG],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::LNOT.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::NEG.name));
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
        rule: LNOT_TO_NEG,
        class: Standard,
        examples: [
        {
            label: lnot_character_alias,
            packages: ["base"],
            input: r"A \lnot B",
            expected: r"A \neg B",
        },
        ]
    }
    // END: Generated examples
}
