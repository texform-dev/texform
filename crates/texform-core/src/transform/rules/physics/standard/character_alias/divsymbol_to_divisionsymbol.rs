//! Collapse divsymbol to the documented divisionsymbol character alias.
//!
//! ```yaml
//! proposal: divsymbol-to-divisionsymbol
//! triggers:
//!   - char:divsymbol
//! consumes:
//!   eliminates: char:divsymbol
//!   touches: null
//! produces: char:divisionsymbol
//! rewrite_patterns:
//!   - {from: \divsymbol, to: \divisionsymbol}
//! ```

use texform_specs::builtin::physics;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static DIVSYMBOL_TO_DIVISIONSYMBOL: DivsymbolToDivisionsymbolRule {
        key: Physics / "divsymbol-to-divisionsymbol",
        class: Standard,
        summary: "Collapse divsymbol to the documented divisionsymbol character alias.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: char_targets![&physics::chars::DIVSYMBOL],
        consumes: RuleConsumes {
            eliminates: char_targets![&physics::chars::DIVSYMBOL],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&physics::chars::DIVISIONSYMBOL],
        },
        apply(rule, cx, node_id) {
            let alias_names = [physics::chars::DIVSYMBOL.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(physics::chars::DIVISIONSYMBOL.name));
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
        rule: DIVSYMBOL_TO_DIVISIONSYMBOL,
        class: Standard,
        examples: [
        {
            label: divsymbol_character_alias,
            packages: ["base", "physics"],
            input: r"A \divsymbol B",
            expected: r"A \divisionsymbol B",
        },
        ]
    }
    // END: Generated examples
}
