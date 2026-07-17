//! Rewrite not followed by a U+2192 right arrow to the canonical AMS nrightarrow character.
//!
//! ```yaml
//! proposal: not-right-arrow-to-nrightarrow
//! triggers:
//!   - cmd:not
//! consumes:
//!   eliminates: null
//!   touches: [cmd:not, char:to, char:rightarrow]
//! produces: char:nrightarrow
//! rewrite_patterns:
//!   - {label: to-command, from: \not\to, to: \nrightarrow}
//!   - {label: rightarrow-command, from: \not\rightarrow, to: \nrightarrow}
//!   - {label: literal, from: \not→, to: \nrightarrow}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{char_targets, cmd_targets, define_rule};

use super::helpers::{following_character_atom, replace_not_pair};

define_rule! {
    pub static NOT_RIGHT_ARROW_TO_NRIGHTARROW: NotRightArrowToNrightarrowRule {
        key: Base / "not-right-arrow-to-nrightarrow",
        level: Authoring,
        summary: "Rewrite not followed by a U+2192 right arrow to the canonical AMS nrightarrow character.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NOT],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: &[RuleTarget::Command(&base::cmd::NOT), RuleTarget::Character(&base::chars::TO), RuleTarget::Character(&base::chars::RIGHTARROW)],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::NRIGHTARROW],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::NOT) else {
                return Ok(RuleEffect::Skipped);
            };
            if !command.args.is_empty() {
                return Ok(RuleEffect::Skipped);
            }
            let Some(right_arrow) = following_character_atom(
                cx,
                node_id,
                '\u{2192}',
                &[&base::chars::TO, &base::chars::RIGHTARROW],
            ) else {
                return Ok(RuleEffect::Skipped);
            };

            replace_not_pair(cx, node_id, right_arrow, &ams::chars::NRIGHTARROW);
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::rewrite::{PlanBuildError, RewriteRule as _, RuleAvailabilityFailure, transform_examples};
    use crate::{BuildConfig, Profile, TransformBuildError, TransformContext};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: NOT_RIGHT_ARROW_TO_NRIGHTARROW,
        level: Authoring,
        examples: [
        {
            label: to_command,
            packages: ["base", "ams"],
            input: r"a\not\to b",
            expected: r"a\nrightarrow b",
        },
        {
            label: rightarrow_command,
            packages: ["base", "ams"],
            input: r"a\not\rightarrow b",
            expected: r"a\nrightarrow b",
        },
        {
            label: literal,
            packages: ["base", "ams"],
            input: r"a\not→ b",
            expected: r"a\nrightarrow b",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NOT_RIGHT_ARROW_TO_NRIGHTARROW,
        level: Authoring,
        examples: [
        {
            label: grouped_arrow_is_preserved,
            packages: ["base", "ams"],
            input: r"a\not{\to}b",
            expected: r"a\not{\to}b",
        },
        {
            label: cross_group_boundary_is_preserved,
            packages: ["base", "ams"],
            input: r"a{\not}\to b",
            expected: r"a{\not}\to b",
        },
        {
            label: scripted_not_is_preserved,
            packages: ["base", "ams"],
            input: r"a\not^x\to b",
            expected: r"a\not^x\to b",
        },
        {
            label: wrong_following_atom_is_preserved,
            packages: ["base", "ams"],
            input: r"a\not\leftarrow b",
            expected: r"a\not\leftarrow b",
        },
        ]
    }

    #[test]
    fn produced_target_requires_ams() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let config = BuildConfig::profile(Profile::Authoring)
            .only_rule_for_tests(NOT_RIGHT_ARROW_TO_NRIGHTARROW.meta().key);
        let error = match TransformContext::from_build_config(config, &parse_ctx) {
            Ok(_) => panic!("nrightarrow rewrite should be unavailable without AMS"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            TransformBuildError::Rewrite(PlanBuildError::SelectedRuleUnavailable {
                rule: NOT_RIGHT_ARROW_TO_NRIGHTARROW.meta().key,
                reason: RuleAvailabilityFailure::ProducedTargetUnavailable {
                    target: RuleTarget::Character(&ams::chars::NRIGHTARROW).key(),
                    active: vec![crate::PackageName::Base],
                },
            })
        );
    }
}
