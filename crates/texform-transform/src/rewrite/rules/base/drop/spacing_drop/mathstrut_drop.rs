//! Drop the invisible mathstrut layout helper in cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: mathstrut-drop
//! triggers:
//!   - cmd:mathstrut
//! consumes:
//!   eliminates: cmd:mathstrut
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \mathstrut, to: ''}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_layout_hint;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static MATHSTRUT_DROP: MathstrutDropRule {
        key: Base / "mathstrut-drop",
        level: Drop,
        summary: "Drop the invisible mathstrut layout helper in cleanup-oriented normalization.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::MATHSTRUT],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::MATHSTRUT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::MATHSTRUT) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\mathstrut")?;

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
        rule: MATHSTRUT_DROP,
        level: Drop,
        examples: [
        {
            label: mathstrut_after_tall_term,
            packages: ["base"],
            input: r"x_1 + y^2\mathstrut + z_3",
            expected: r"x_1 + y^2 + z_3",
        },
        {
            label: mathstrut_inside_radical_changes_root_size,
            packages: ["base"],
            input: r"\sqrt{x\mathstrut}+\sqrt{x^i\mathstrut}",
            expected: r"\sqrt{x}+\sqrt{x^i}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: MATHSTRUT_DROP,
        level: Drop,
        examples: [
        {
            label: mathstrut_as_script_base,
            packages: ["base"],
            input: r"\mathstrut^2",
            expected: r"{}^2",
        },
        ]
    }
}
