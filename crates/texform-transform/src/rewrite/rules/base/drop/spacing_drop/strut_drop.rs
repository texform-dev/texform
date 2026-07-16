//! Drop the fixed empty strut node in cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: strut-drop
//! triggers:
//!   - cmd:strut
//! consumes:
//!   eliminates: cmd:strut
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \strut, to: ''}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_layout_hint;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static STRUT_DROP: StrutDropRule {
        key: Base / "strut-drop",
        level: Drop,
        summary: "Drop the fixed empty strut node in cleanup-oriented normalization.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::STRUT],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::STRUT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::STRUT) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\strut")?;

            drop_layout_hint(cx.ast, node_id);
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
        rule: STRUT_DROP,
        level: Drop,
        examples: [
        {
            label: strut_after_tall_term,
            packages: ["base"],
            input: r"x_1 + y^2\strut + z_3",
            expected: r"x_1 + y^2 + z_3",
        },
        {
            label: strut_inside_radical_changes_root_size,
            packages: ["base"],
            input: r"\sqrt{x\strut}+\sqrt{x^i\strut}",
            expected: r"\sqrt{x}+\sqrt{x^i}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: STRUT_DROP,
        level: Drop,
        examples: [
        {
            label: strut_as_script_base,
            packages: ["base"],
            input: r"\strut^2",
            expected: r"{}^2",
        },
        ]
    }
}
