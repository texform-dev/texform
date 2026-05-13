//! Normalize vartriangle to bigtriangleup for corpus form.
//!
//! ```yaml
//! proposal: vartriangle-to-bigtriangleup
//! triggers:
//!   - char:vartriangle
//! consumes:
//!   eliminates: char:vartriangle
//!   touches: null
//! produces: char:bigtriangleup
//! rewrite_patterns:
//!   - {from: \vartriangle, to: \bigtriangleup}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use crate::ast::Node;
use crate::transform::helpers::bare_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{char_targets, define_rule};

define_rule! {
    pub static VARTRIANGLE_TO_BIGTRIANGLEUP: VartriangleToBigtriangleupRule {
        key: Ams / "vartriangle-to-bigtriangleup",
        class: Expand,
        summary: "Normalize vartriangle to bigtriangleup for corpus form.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: char_targets![&ams::chars::VARTRIANGLE],
        consumes: RuleConsumes {
            eliminates: char_targets![&ams::chars::VARTRIANGLE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::BIGTRIANGLEUP],
        },
        apply(rule, cx, node_id) {
            let alias_names = [ams::chars::VARTRIANGLE.name];
            let (subject, args) = match cx.node(node_id) {
                Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => {
                    (format!("\\{name}"), args)
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, &subject)?;

            cx.ast.replace_node(node_id, bare_command_node(base::chars::BIGTRIANGLEUP.name));
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
        rule: VARTRIANGLE_TO_BIGTRIANGLEUP,
        class: Expand,
        examples: [
        {
            label: vartriangle_character_alias,
            packages: ["base", "ams"],
            input: r"A \vartriangle B",
            expected: r"A \bigtriangleup B",
        },
        {
            label: vartriangle_script_position,
            packages: ["base", "ams"],
            input: r"x_{\vartriangle}",
            expected: r"x_{\bigtriangleup}",
        },
        ]
    }
    // END: Generated examples
}
