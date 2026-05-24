//! Normalize semantic centered-dot ellipsis aliases to cdots for corpus form.
//!
//! ```yaml
//! proposal: semantic-dots-to-cdots
//! triggers:
//!   - char:dotsb
//!   - char:dotsm
//!   - char:dotsi
//! consumes:
//!   eliminates: [char:dotsb, char:dotsm, char:dotsi]
//!   touches: null
//! produces: char:cdots
//! rewrite_patterns:
//!   - {label: dotsb, from: \dotsb, to: \cdots}
//!   - {label: dotsm, from: \dotsm, to: \cdots}
//!   - {label: dotsi, from: \dotsi, to: \cdots}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static SEMANTIC_DOTS_TO_CDOTS: SemanticDotsToCdotsRule {
        key: Base / "semantic-dots-to-cdots",
        class: Expand,
        summary: "Normalize semantic centered-dot ellipsis aliases to cdots for corpus form.",
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::DOTSB, &base::chars::DOTSM, &base::chars::DOTSI],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::DOTSB, &base::chars::DOTSM, &base::chars::DOTSI],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::CDOTS],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::DOTSB.name, base::chars::DOTSM.name, base::chars::DOTSI.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::CDOTS.name));
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
        rule: SEMANTIC_DOTS_TO_CDOTS,
        class: Expand,
        examples: [
        {
            label: dotsb_character_alias,
            packages: ["base"],
            input: r"A \dotsb B",
            expected: r"A \cdots B",
        },
        {
            label: dotsm_character_alias,
            packages: ["base"],
            input: r"A \dotsm B",
            expected: r"A \cdots B",
        },
        {
            label: dotsi_character_alias,
            packages: ["base"],
            input: r"A \dotsi B",
            expected: r"A \cdots B",
        },
        ]
    }
    // END: Generated examples
}
