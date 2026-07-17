//! Rewrite \not\exists to the canonical AMS \nexists character when the target is available.
//!
//! ```yaml
//! proposal: not-exists-to-nexists
//! triggers:
//!   - cmd:not
//! consumes:
//!   eliminates: null
//!   touches: [cmd:not, char:exists]
//! produces: char:nexists
//! rewrite_patterns:
//!   - {label: command, from: \not\exists, to: \nexists}
//!   - {label: literal, from: \not∃, to: \nexists}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{char_targets, cmd_targets, define_rule};

use super::helpers::{following_character_atom, replace_not_pair};

define_rule! {
    pub static NOT_EXISTS_TO_NEXISTS: NotExistsToNexistsRule {
        key: Base / "not-exists-to-nexists",
        level: Authoring,
        summary: "Rewrite \\not\\exists to the canonical AMS \\nexists character when the target is available.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NOT],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: &[RuleTarget::Command(&base::cmd::NOT), RuleTarget::Character(&base::chars::EXISTS)],
        },
        produces: RuleProduces {
            targets: char_targets![&ams::chars::NEXISTS],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::NOT) else {
                return Ok(RuleEffect::Skipped);
            };
            if !command.args.is_empty() {
                return Ok(RuleEffect::Skipped);
            }
            let Some(exists) = following_character_atom(
                cx,
                node_id,
                '\u{2203}',
                &[&base::chars::EXISTS],
            ) else {
                return Ok(RuleEffect::Skipped);
            };

            replace_not_pair(cx, node_id, exists, &ams::chars::NEXISTS);
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
        rule: NOT_EXISTS_TO_NEXISTS,
        level: Authoring,
        examples: [
        {
            label: command,
            packages: ["base", "ams"],
            input: r"\not\exists x",
            expected: r"\nexists x",
        },
        {
            label: literal,
            packages: ["base", "ams"],
            input: r"\not∃ x",
            expected: r"\nexists x",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NOT_EXISTS_TO_NEXISTS,
        level: Authoring,
        examples: [
        {
            label: grouped_exists_is_preserved,
            packages: ["base", "ams"],
            input: r"\not{\exists} x",
            expected: r"\not{\exists} x",
        },
        {
            label: cross_group_boundary_is_preserved,
            packages: ["base", "ams"],
            input: r"{\not}\exists x",
            expected: r"{\not}\exists x",
        },
        {
            label: scripted_not_is_preserved,
            packages: ["base", "ams"],
            input: r"\not^a\exists x",
            expected: r"\not^a\exists x",
        },
        {
            label: wrong_following_atom_is_preserved,
            packages: ["base", "ams"],
            input: r"\not\forall x",
            expected: r"\not\forall x",
        },
        ]
    }

    #[test]
    fn produced_target_requires_ams() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let config = BuildConfig::profile(Profile::Authoring)
            .only_rule_for_tests(NOT_EXISTS_TO_NEXISTS.meta().key);
        let error = match TransformContext::from_build_config(config, &parse_ctx) {
            Ok(_) => panic!("nexists rewrite should be unavailable without AMS"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            TransformBuildError::Rewrite(PlanBuildError::SelectedRuleUnavailable {
                rule: NOT_EXISTS_TO_NEXISTS.meta().key,
                reason: RuleAvailabilityFailure::ProducedTargetUnavailable {
                    target: RuleTarget::Character(&ams::chars::NEXISTS).key(),
                    active: vec![crate::PackageName::Base],
                },
            })
        );
    }
}
