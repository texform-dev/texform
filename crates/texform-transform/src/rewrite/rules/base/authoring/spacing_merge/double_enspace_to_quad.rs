//! Rewrite two adjacent enspaces to one exact-width quad.
//!
//! ```yaml
//! proposal: double-enspace-to-quad
//! triggers:
//!   - cmd:enspace
//! consumes:
//!   eliminates: null
//!   touches: cmd:enspace
//! produces: cmd:quad
//! rewrite_patterns:
//!   - {from: \enspace\enspace, to: \quad}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::is_math_sibling;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static DOUBLE_ENSPACE_TO_QUAD: DoubleEnspaceToQuadRule {
        key: Base / "double-enspace-to-quad",
        level: Authoring,
        summary: "Rewrite two adjacent enspaces to one exact-width quad.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::ENSPACE],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::ENSPACE],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::QUAD],
        },
        apply(_rule, cx, node_id) {
            if !is_math_sibling(cx, node_id) {
                return Ok(RuleEffect::Skipped);
            }
            let Some(command) = cx.match_command(node_id, &base::cmd::ENSPACE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\enspace")?;

            if cx
                .ast
                .prev_sibling(node_id)
                .is_some_and(|previous| is_argument_free_enspace(cx, previous))
            {
                return Ok(RuleEffect::Skipped);
            }

            let Some(next) = cx.ast.next_sibling(node_id) else {
                return Ok(RuleEffect::Skipped);
            };
            if !is_argument_free_enspace(cx, next) {
                return Ok(RuleEffect::Skipped);
            }

            cx.ast.replace_node(node_id, bare_command_node(base::cmd::QUAD.name));
            cx.ast.remove_node(next);
            Ok(RuleEffect::Applied)
        }
    }
}

fn is_argument_free_enspace(
    cx: &crate::rewrite::rule_context::RuleContext<'_>,
    node_id: crate::ast::NodeId,
) -> bool {
    cx.match_command(node_id, &base::cmd::ENSPACE)
        .is_some_and(|command| command.args.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::{BuildConfig, Profile, TransformConfig, TransformContext};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DOUBLE_ENSPACE_TO_QUAD,
        level: Authoring,
        examples: [
        {
            label: double_enspace,
            packages: ["base"],
            input: r"A\enspace\enspace B",
            expected: r"A\quad B",
        },
        {
            label: text_double_enspace_preserved,
            packages: ["base", "textmacros"],
            input: r"\text{A\enspace\enspace B}",
            expected: r"\text{A\enspace\enspace B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: DOUBLE_ENSPACE_TO_QUAD,
        level: Authoring,
        examples: [
        {
            label: greedy_triple_leaves_trailing_enspace,
            packages: ["base"],
            input: r"A\enspace\enspace\enspace B",
            expected: r"A\quad\enspace B",
        },
        {
            label: singleton_is_preserved,
            packages: ["base"],
            input: r"A\enspace B",
            expected: r"A\enspace B",
        },
        {
            label: explicit_group_boundary_is_preserved,
            packages: ["base"],
            input: r"A\enspace{\enspace}B",
            expected: r"A\enspace{\enspace}B",
        },
        {
            label: merges_inside_script_group,
            packages: ["base"],
            input: r"x^{\enspace\enspace}",
            expected: r"x^{\quad}",
        },
        {
            label: argument_slots_do_not_share_siblings,
            packages: ["base"],
            input: r"\frac{\enspace}{\enspace}",
            expected: r"\frac{\enspace}{\enspace}",
        },
        ]
    }

    #[test]
    fn composes_left_to_right_with_small_spacer_merge() {
        assert_authoring_transform(
            r"A\,\,\,\,\,\,\,\,\,B",
            r"A\quad\enspace B",
        );
        assert_authoring_transform(
            r"A\,\,\,\,\,\,\,\,\,\,\,\,B",
            r"A\qquad B",
        );
    }

    fn assert_authoring_transform(input: &str, expected: &str) {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let transform_context = TransformContext::from_build_config(
            BuildConfig::profile(Profile::Authoring),
            &parse_ctx,
        )
        .expect("authoring transform context should build");
        let config = TransformConfig {
            rewrite_enabled: true,
            lower_attributes_enabled: false,
            finalize_ast: crate::FinalizeAstConfig::DISABLED,
            flatten_groups: crate::FlattenGroupsConfig::DISABLED,
            max_iterations: 100,
        };
        let parse_config = texform_core::parse::ParseConfig::STRICT;
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &parse_config);

        transform_context
            .run_with(&mut ast, &parse_ctx, &config)
            .expect("authoring transform should succeed");

        let actual = crate::serialize::serialize(&ast);
        let expected_ast = crate::parse_to_ast_for_test(&parse_ctx, expected, &parse_config);
        assert_eq!(actual, crate::serialize::serialize(&expected_ast));
    }
}
