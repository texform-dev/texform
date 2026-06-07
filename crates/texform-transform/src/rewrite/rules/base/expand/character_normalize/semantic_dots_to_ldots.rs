//! Normalize semantic baseline ellipsis aliases to ldots for corpus form.
//!
//! ```yaml
//! proposal: semantic-dots-to-ldots
//! triggers:
//!   - char:dotsc
//!   - char:dotso
//! consumes:
//!   eliminates: [char:dotsc, char:dotso]
//!   touches: null
//! produces: char:ldots
//! rewrite_patterns:
//!   - {label: dotsc, from: \dotsc, to: \ldots}
//!   - {label: dotso, from: \dotso, to: \ldots}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static SEMANTIC_DOTS_TO_LDOTS: SemanticDotsToLdotsRule {
        key: Base / "semantic-dots-to-ldots",
        level: Expand,
        summary: "Normalize semantic baseline ellipsis aliases to ldots for corpus form.",
        fidelity: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::DOTSC, &base::chars::DOTSO],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::DOTSC, &base::chars::DOTSO],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::LDOTS],
        },
        apply(rule, cx, node_id) {
            let alias_names = [base::chars::DOTSC.name, base::chars::DOTSO.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::LDOTS.name));
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
        rule: SEMANTIC_DOTS_TO_LDOTS,
        level: Expand,
        examples: [
        {
            label: dotsc_character_alias,
            packages: ["base"],
            input: r"A \dotsc B",
            expected: r"A \ldots B",
        },
        {
            label: dotso_character_alias,
            packages: ["base"],
            input: r"A \dotso B",
            expected: r"A \ldots B",
        },
        ]
    }
    // END: Generated examples
}
