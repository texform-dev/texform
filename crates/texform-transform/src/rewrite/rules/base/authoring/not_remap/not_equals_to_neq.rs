//! Rewrite \not followed by an equals sign to the canonical \neq character.
//!
//! ```yaml
//! proposal: not-equals-to-neq
//! triggers:
//!   - cmd:not
//! consumes:
//!   eliminates: null
//!   touches: cmd:not
//! produces: char:neq
//! rewrite_patterns:
//!   - {from: \not=, to: \neq}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, cmd_targets, define_rule};

use super::helpers::{following_character_atom, replace_not_pair};

define_rule! {
    pub static NOT_EQUALS_TO_NEQ: NotEqualsToNeqRule {
        key: Base / "not-equals-to-neq",
        level: Authoring,
        summary: "Rewrite \\not followed by an equals sign to the canonical \\neq character.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NOT],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::NOT],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::NEQ],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::NOT) else {
                return Ok(RuleEffect::Skipped);
            };
            if !command.args.is_empty() {
                return Ok(RuleEffect::Skipped);
            }
            let Some(equals) = following_character_atom(cx, node_id, '=', &[]) else {
                return Ok(RuleEffect::Skipped);
            };

            replace_not_pair(cx, node_id, equals, &base::chars::NEQ);
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
        rule: NOT_EQUALS_TO_NEQ,
        level: Authoring,
        examples: [
        {
            label: equals,
            packages: ["base"],
            input: r"a\not=b",
            expected: r"a\neq b",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NOT_EQUALS_TO_NEQ,
        level: Authoring,
        examples: [
        {
            label: grouped_equals_is_preserved,
            packages: ["base"],
            input: r"a\not{=}b",
            expected: r"a\not{=}b",
        },
        {
            label: cross_group_boundary_is_preserved,
            packages: ["base"],
            input: r"a{\not}=b",
            expected: r"a{\not}=b",
        },
        {
            label: scripted_not_is_preserved,
            packages: ["base"],
            input: r"a\not^x=b",
            expected: r"a\not^x=b",
        },
        {
            label: wrong_following_atom_is_preserved,
            packages: ["base"],
            input: r"a\not<b",
            expected: r"a\not<b",
        },
        ]
    }
}
