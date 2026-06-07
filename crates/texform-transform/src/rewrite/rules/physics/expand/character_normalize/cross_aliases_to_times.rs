//! Normalize physics cross-product glyph aliases to times for corpus form.
//!
//! ```yaml
//! proposal: cross-aliases-to-times
//! triggers:
//!   - char:cp
//!   - char:cross
//!   - char:crossproduct
//! consumes:
//!   eliminates: [char:cp, char:cross, char:crossproduct]
//!   touches: null
//! produces: char:times
//! rewrite_patterns:
//!   - {label: cp, from: \cp, to: \times}
//!   - {label: cross, from: \cross, to: \times}
//!   - {label: crossproduct, from: \crossproduct, to: \times}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static CROSS_ALIASES_TO_TIMES: CrossAliasesToTimesRule {
        key: Physics / "cross-aliases-to-times",
        level: Expand,
        summary: "Normalize physics cross-product glyph aliases to times for corpus form.",
        fidelity: Lossless,
        enabled_by_packages: [Physics],
        triggers: char_targets![&physics::chars::CP, &physics::chars::CROSS, &physics::chars::CROSSPRODUCT],
        consumes: RuleConsumes {
            eliminates: char_targets![&physics::chars::CP, &physics::chars::CROSS, &physics::chars::CROSSPRODUCT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::TIMES],
        },
        apply(rule, cx, node_id) {
            let alias_names = [physics::chars::CP.name, physics::chars::CROSS.name, physics::chars::CROSSPRODUCT.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::TIMES.name));
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
        rule: CROSS_ALIASES_TO_TIMES,
        level: Expand,
        examples: [
        {
            label: cp_character_alias,
            packages: ["base", "physics"],
            input: r"A \cp B",
            expected: r"A \times B",
        },
        {
            label: cross_character_alias,
            packages: ["base", "physics"],
            input: r"A \cross B",
            expected: r"A \times B",
        },
        {
            label: crossproduct_character_alias,
            packages: ["base", "physics"],
            input: r"A \crossproduct B",
            expected: r"A \times B",
        },
        ]
    }
    // END: Generated examples
}
