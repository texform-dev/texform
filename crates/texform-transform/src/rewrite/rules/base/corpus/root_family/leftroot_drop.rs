//! Drop the leftroot index-position hint in cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: leftroot-drop
//! triggers:
//!   - cmd:leftroot
//! consumes:
//!   eliminates: cmd:leftroot
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: '\leftroot{#1}', to: ''}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::helpers::remove_node_preserving_slot;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static LEFTROOT_DROP: LeftrootDropRule {
        key: Base / "leftroot-drop",
        level: Corpus,
        summary: "Drop the leftroot index-position hint in cleanup-oriented normalization.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::LEFTROOT],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::LEFTROOT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::LEFTROOT) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 1, r"\leftroot")?;

            remove_node_preserving_slot(cx.ast, node_id);
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
        rule: LEFTROOT_DROP,
        level: Corpus,
        examples: [
        {
            label: leftroot_inside_sqrt_degree,
            packages: ["base"],
            input: r"\sqrt[\leftroot{-2}3]{k}",
            expected: r"\sqrt[3]{k}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: LEFTROOT_DROP,
        level: Corpus,
        examples: [
        {
            label: leftroot_as_script_base,
            packages: ["base"],
            input: r"\leftroot{-2}^x",
            expected: r"{}^x",
        },
        {
            label: misplaced_leftroot,
            packages: ["base"],
            input: r"\leftroot{2}x",
            expected: r"x",
        },
        {
            label: unbraced_integer_in_degree,
            packages: ["base"],
            input: r"\sqrt[\leftroot2x]{k}",
            expected: r"\sqrt[x]{k}",
        },
        {
            label: preserves_companion_uproot,
            packages: ["base"],
            input: r"\sqrt[\leftroot{-2}\uproot{2}3]{k}",
            expected: r"\sqrt[\uproot{2}3]{k}",
        },
        {
            label: leftroot_inside_root_of_degree,
            packages: ["base"],
            input: r"\root n\leftroot{-2}\of k",
            expected: r"\root n\of k",
        },
        ]
    }
}
