//! Rewrite \not\in to the canonical \notin character.
//!
//! ```yaml
//! proposal: not-in-to-notin
//! triggers:
//!   - cmd:not
//! consumes:
//!   eliminates: null
//!   touches: [cmd:not, char:in]
//! produces: char:notin
//! rewrite_patterns:
//!   - {label: command, from: \not\in, to: \notin}
//!   - {label: literal, from: \not∈, to: \notin}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{char_targets, cmd_targets, define_rule};

use super::helpers::{following_character_atom, replace_not_pair};

define_rule! {
    pub static NOT_IN_TO_NOTIN: NotInToNotinRule {
        key: Base / "not-in-to-notin",
        level: Authoring,
        summary: "Rewrite \\not\\in to the canonical \\notin character.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NOT],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: &[RuleTarget::Command(&base::cmd::NOT), RuleTarget::Character(&base::chars::IN)],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::NOTIN],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::NOT) else {
                return Ok(RuleEffect::Skipped);
            };
            if !command.args.is_empty() {
                return Ok(RuleEffect::Skipped);
            }
            let Some(in_atom) = following_character_atom(
                cx,
                node_id,
                '\u{2208}',
                &[&base::chars::IN],
            ) else {
                return Ok(RuleEffect::Skipped);
            };

            replace_not_pair(cx, node_id, in_atom, &base::chars::NOTIN);
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
        rule: NOT_IN_TO_NOTIN,
        level: Authoring,
        examples: [
        {
            label: command,
            packages: ["base"],
            input: r"x\not\in A",
            expected: r"x\notin A",
        },
        {
            label: literal,
            packages: ["base"],
            input: r"x\not∈ A",
            expected: r"x\notin A",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NOT_IN_TO_NOTIN,
        level: Authoring,
        examples: [
        {
            label: grouped_in_is_preserved,
            packages: ["base"],
            input: r"x\not{\in} A",
            expected: r"x\not{\in} A",
        },
        {
            label: cross_group_boundary_is_preserved,
            packages: ["base"],
            input: r"x{\not}\in A",
            expected: r"x{\not}\in A",
        },
        {
            label: scripted_not_is_preserved,
            packages: ["base"],
            input: r"x\not^a\in A",
            expected: r"x\not^a\in A",
        },
        {
            label: wrong_following_atom_is_preserved,
            packages: ["base"],
            input: r"x\not\ni A",
            expected: r"x\not\ni A",
        },
        ]
    }
}
