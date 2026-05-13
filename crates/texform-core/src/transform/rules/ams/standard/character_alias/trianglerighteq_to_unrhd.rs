//! Collapse trianglerighteq to the more common unrhd character.
//!
//! ```yaml
//! proposal: trianglerighteq-to-unrhd
//! triggers:
//!   - char:trianglerighteq
//! consumes:
//!   eliminates: char:trianglerighteq
//!   touches: null
//! produces: char:unrhd
//! rewrite_patterns:
//!   - {from: \trianglerighteq, to: \unrhd}
//! ```

use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static TRIANGLERIGHTEQ_TO_UNRHD: TrianglerighteqToUnrhdRule {
        key: Ams / "trianglerighteq-to-unrhd",
        class: Standard,
        summary: "Collapse trianglerighteq to the more common unrhd character.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::TRIANGLERIGHTEQ],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::TRIANGLERIGHTEQ],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::UNRHD],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::TRIANGLERIGHTEQ.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(ams::chars::UNRHD.name));
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
        rule: TRIANGLERIGHTEQ_TO_UNRHD,
        class: Standard,
        examples: [
        {
            label: trianglerighteq_character_alias,
            packages: ["base", "ams"],
            input: r"A \trianglerighteq B",
            expected: r"A \unrhd B",
        },
        ]
    }
    // END: Generated examples
}
