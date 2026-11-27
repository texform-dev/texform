//! Drop the italic correction \/ in math mode, keeping an empty group where it carries scripts or separates a single-atom body.
//!
//! ```yaml
//! proposal: italic-correction-drop
//! triggers:
//!   - cmd:/
//! consumes:
//!   eliminates: null
//!   touches: cmd:/
//! produces: null
//! rewrite_patterns:
//!   - {label: bare, from: '#1\/#2', to: '#1#2'}
//!   - {label: scripted, from: '#1\/_{#2}', to: '#1{}_{#2}'}
//!   - {label: single-atom-body, from: '{#1\/}', to: '{#1{}}'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{Ast, ContentMode, GroupKind, Node, NodeId, ParentLink, Slot};
use crate::rewrite::helpers::{content_mode_at, remove_node_preserving_slot};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static ITALIC_CORRECTION_DROP: ItalicCorrectionDropRule {
        key: Base / "italic-correction-drop",
        level: Authoring,
        summary: "Drop the italic correction \\/ in math mode, keeping an empty group where it carries scripts or separates a single-atom body.",
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::_SLASH],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::_SLASH],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::_SLASH) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\/")?;
            // In text mode \/ is a real italic correction after the preceding glyph.
            if content_mode_at(cx.ast, node_id) != Some(ContentMode::Math) {
                return Ok(RuleEffect::Skipped);
            }

            match cx.ast.parent(node_id) {
                Some(ParentLink {
                    parent,
                    slot: Slot::GroupChild(_),
                }) if is_two_node_brace_body(cx.ast, parent) => {
                    cx.ast.replace_node(
                        node_id,
                        Node::Group {
                            children: Vec::new(),
                            kind: GroupKind::Implicit,
                            mode: ContentMode::Math,
                        },
                    );
                }
                // A script base keeps an empty group so the scripts do not move
                // onto the preceding atom; other single-child slots stay filled.
                _ => remove_node_preserving_slot(cx.ast, node_id),
            }
            Ok(RuleEffect::Applied)
        }
    }
}

/// Reports whether `group` is a brace group or argument body with exactly two
/// nodes. Removing the kern from it would leave a single-atom body, which TeX
/// lays out differently (a single-character `\mathop` body is centered on the
/// axis and an accent uses the character's skew), so the kern becomes an empty
/// group that keeps the body multi-atom without adding width. Delimited and
/// inline-math groups and environment bodies have no single-atom layout.
fn is_two_node_brace_body(ast: &Ast, group: NodeId) -> bool {
    matches!(
        ast.node(group),
        Node::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            children,
            ..
        } if children.len() == 2
    ) && ast.slot(group) != Some(Slot::EnvBody)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ITALIC_CORRECTION_DROP,
        level: Authoring,
        examples: [
        {
            label: bare,
            packages: ["base"],
            input: r"f\/x",
            expected: r"fx",
        },
        {
            label: scripted,
            packages: ["base"],
            input: r"\sum\/_{i} a",
            expected: r"\sum{}_{i} a",
        },
        {
            label: single_atom_body,
            packages: ["base"],
            input: r"\mathop{E\/}[X]",
            expected: r"\mathop{E{}}[X]",
        },
        {
            label: single_atom_accent_body,
            packages: ["base"],
            input: r"\hat{f\/}",
            expected: r"\hat{f{}}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: ITALIC_CORRECTION_DROP,
        level: Authoring,
        examples: [
        {
            label: drops_kern_that_blocks_an_argument,
            packages: ["base", "physics"],
            input: r"\var\/{x} + \sin\/[2]x",
            expected: r"{\var}{x} + \sin{}[2]x",
        },
        {
            label: drops_kern_at_environment_body_start,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{matrix*}\/[r]x\end{matrix*}",
            expected: r"\begin{matrix*}{}[r]x\end{matrix*}",
        },
        {
            label: drops_kern_before_binary_operator,
            packages: ["base"],
            input: r"(\/-x)",
            expected: r"(-x)",
        },
        {
            label: drops_kern_from_multi_atom_group,
            packages: ["base"],
            input: r"\hat{xy\/}",
            expected: r"\hat{xy}",
        },
        {
            label: keeps_single_atom_script_base_group,
            packages: ["base"],
            input: r"{f\/}^2",
            expected: r"{f{}}^2",
        },
        {
            label: repeated_kerns_leave_one_empty_group,
            packages: ["base"],
            input: r"\mathop{E\/\/}",
            expected: r"\mathop{E{}}",
        },
        {
            label: leading_kern_in_accent_body,
            packages: ["base"],
            input: r"\hat{\/f}",
            expected: r"\hat{{}f}",
        },
        {
            label: keeps_single_atom_sequence_brace_group,
            packages: ["base"],
            input: r"a{f\/}b",
            expected: r"a{f{}}b",
        },
        {
            label: drops_kern_from_environment_body,
            packages: ["base", "ams"],
            input: r"\begin{matrix}f\/\end{matrix}",
            expected: r"\begin{matrix}f\end{matrix}",
        },
        {
            label: drops_kern_from_delimited_group,
            packages: ["base"],
            input: r"\left(f\/\right)",
            expected: r"\left(f\right)",
        },
        {
            label: drops_kern_from_inline_math,
            packages: ["base"],
            input: r"\text{$f\/$}",
            expected: r"\text{$f$}",
        },
        {
            label: drops_root_level_trailing_kern,
            packages: ["base"],
            input: r"f\/",
            expected: r"f",
        },
        {
            label: sole_group_child_leaves_the_group_empty,
            packages: ["base"],
            input: r"{\/}x",
            expected: r"{}x",
        },
        {
            label: superscript_base,
            packages: ["base"],
            input: r"f\/^2",
            expected: r"f{}^2",
        },
        {
            label: brace_less_argument_keeps_an_empty_slot,
            packages: ["base"],
            input: r"\hat\/ x^\/",
            expected: r"\hat{} x^{}",
        },
        {
            label: inline_math_inside_text,
            packages: ["base"],
            input: r"\text{a$f\/x$}",
            expected: r"\text{a$fx$}",
        },
        {
            label: preserves_text_mode_italic_correction,
            packages: ["base"],
            input: r"\text{f\/ x}\text\/",
            expected: r"\text{f\/ x}\text\/",
        },
        ]
    }

    #[test]
    fn authoring_profile_keeps_structural_empty_groups_through_flatten_groups() {
        use crate::parse::{ParseConfig, ParseContext};
        use crate::{BuildConfig, Profile, TransformContext};

        let parse_ctx = ParseContext::from_packages(&["base"]);
        let engine = TransformContext::from_build_config(
            BuildConfig::profile(Profile::Authoring),
            &parse_ctx,
        )
        .unwrap();
        for (input, expected) in [
            (r"\sum\/_{i} a", r"\sum{}_{i} a"),
            (r"\mathop{E\/}[X]", r"\mathop{E{}}[X]"),
            (r"\hat{f\/}", r"\hat{f{}}"),
            (r"{f\/}^2", r"{f{}}^2"),
            (r"f\/x", r"fx"),
        ] {
            let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
            engine.run(&mut ast, &parse_ctx).unwrap();
            ast.assert_invariants();
            let expected =
                crate::parse_to_ast_for_test(&parse_ctx, expected, &ParseConfig::STRICT);
            assert_eq!(
                crate::serialize::serialize(&ast),
                crate::serialize::serialize(&expected),
                "{input}"
            );
        }
    }

    #[test]
    fn authoring_profile_drops_kern_at_boundary_hazards_and_round_trips() {
        use crate::parse::{ParseConfig, ParseContext};
        use crate::rewrite::RewriteRule as _;
        use crate::{BuildConfig, Profile, TransformContext, round_trip};

        // Full Authoring profile: boundary separation wraps `\var` and strict
        // FlattenGroups then unwraps `{x}` to reach a fixed point.
        let physics_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        let physics_engine = TransformContext::from_build_config(
            BuildConfig::profile(Profile::Authoring),
            &physics_ctx,
        )
        .unwrap();
        for (input, expected) in [
            (r"\var\/{x}", r"{ \var } x"),
            (r"\sin\/[2]x", r"\sin { } [ 2 ] x"),
            (
                r"\begin{aligned}\/[a]x\end{aligned}",
                r"\begin {aligned} { } [ a ] x \end {aligned}",
            ),
        ] {
            let mut ast = crate::parse_to_ast_for_test(&physics_ctx, input, &ParseConfig::STRICT);
            physics_engine.run(&mut ast, &physics_ctx).unwrap();
            ast.assert_invariants();
            assert_eq!(crate::serialize::serialize(&ast), expected, "{input}");
            let failures = round_trip::check_transformed(&physics_ctx, &physics_engine, &ast);
            assert!(failures.is_empty(), "{input}: {failures:?}");
        }

        // Isolate this rule under the full Authoring phase pipeline so later
        // `matrix*` rewrites do not hide the environment-body separation check.
        let mathtools_ctx = ParseContext::from_packages(&["base", "ams", "mathtools"]);
        let mathtools_engine = TransformContext::from_build_config(
            BuildConfig::profile(Profile::Authoring)
                .only_rule_for_tests(ITALIC_CORRECTION_DROP.meta().key),
            &mathtools_ctx,
        )
        .unwrap();
        let input = r"\begin{matrix*}\/[r]x\end{matrix*}";
        let mut ast = crate::parse_to_ast_for_test(&mathtools_ctx, input, &ParseConfig::STRICT);
        mathtools_engine.run(&mut ast, &mathtools_ctx).unwrap();
        ast.assert_invariants();
        assert_eq!(
            crate::serialize::serialize(&ast),
            r"\begin {matrix*} { } [ r ] x \end {matrix*}"
        );
        let failures = round_trip::check_transformed(&mathtools_ctx, &mathtools_engine, &ast);
        assert!(failures.is_empty(), "{input}: {failures:?}");
    }
}
