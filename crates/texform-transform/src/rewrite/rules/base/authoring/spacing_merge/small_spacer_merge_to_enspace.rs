//! Merge exact half-em small-spacer chunks into enspace in one pass.
//!
//! ```yaml
//! proposal: small-spacer-merge-to-enspace
//! triggers:
//!   - 'cmd:,'
//!   - 'cmd::'
//!   - cmd:;
//! consumes:
//!   eliminates: null
//!   touches: ['cmd:,', 'cmd::', cmd:;]
//! produces: cmd:enspace
//! rewrite_patterns:
//!   - {label: thin-triple, from: '\,\,\,', to: \enspace}
//!   - {label: colon-semicolon, from: \:\;, to: \enspace}
//!   - {label: semicolon-colon, from: '\;\:', to: \enspace}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::is_math_sibling;
use crate::ast::{Node, NodeId};
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static SMALL_SPACER_MERGE_TO_ENSPACE: SmallSpacerMergeToEnspaceRule {
        key: Base / "small-spacer-merge-to-enspace",
        level: Authoring,
        summary: "Merge exact half-em small-spacer chunks into enspace in one pass.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::_COMMA, &base::cmd::_COLON, &base::cmd::_SEMICOLON],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::_COMMA, &base::cmd::_COLON, &base::cmd::_SEMICOLON],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::ENSPACE],
        },
        apply(_rule, cx, node_id) {
            if !is_math_sibling(cx, node_id) {
                return Ok(RuleEffect::Skipped);
            }
            let Some(first) = small_spacer_kind(cx, node_id, false) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(cx.ast.arg_slots(node_id), first.subject())?;

            if cx
                .ast
                .prev_sibling(node_id)
                .and_then(|previous| small_spacer_kind(cx, previous, true))
                .is_some()
            {
                return Ok(RuleEffect::Skipped);
            }

            let mut run = Vec::new();
            let mut current = Some(node_id);
            while let Some(candidate) = current {
                let Some(kind) = small_spacer_kind(cx, candidate, true) else {
                    break;
                };
                run.push((candidate, kind));
                current = cx.ast.next_sibling(candidate);
            }

            let mut applied = false;
            let mut index = 0;
            while index < run.len() {
                let chunk_len = matching_chunk_len(&run[index..]);
                if chunk_len == 0 {
                    index += 1;
                    continue;
                }

                cx.ast.replace_node(
                    run[index].0,
                    bare_command_node(base::cmd::ENSPACE.name),
                );
                for &(consumed, _) in &run[index + 1..index + chunk_len] {
                    cx.ast.remove_node(consumed);
                }
                applied = true;
                index += chunk_len;
            }

            Ok(if applied {
                RuleEffect::Applied
            } else {
                RuleEffect::Skipped
            })
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SmallSpacerKind {
    Thin,
    Medium,
    Thick,
}

impl SmallSpacerKind {
    fn subject(self) -> &'static str {
        match self {
            Self::Thin => r"\,",
            Self::Medium => r"\:",
            Self::Thick => r"\;",
        }
    }
}

fn small_spacer_kind(
    cx: &RuleContext<'_>,
    node_id: NodeId,
    require_no_args: bool,
) -> Option<SmallSpacerKind> {
    let Node::Command { name, args, .. } = cx.ast.node(node_id) else {
        return None;
    };
    if require_no_args && !args.is_empty() {
        return None;
    }

    if name == base::cmd::_COMMA.name {
        Some(SmallSpacerKind::Thin)
    } else if name == base::cmd::_COLON.name {
        Some(SmallSpacerKind::Medium)
    } else if name == base::cmd::_SEMICOLON.name {
        Some(SmallSpacerKind::Thick)
    } else {
        None
    }
}

fn matching_chunk_len(run: &[(NodeId, SmallSpacerKind)]) -> usize {
    match run {
        [
            (_, SmallSpacerKind::Thin),
            (_, SmallSpacerKind::Thin),
            (_, SmallSpacerKind::Thin),
            ..
        ] => 3,
        [(_, SmallSpacerKind::Medium), (_, SmallSpacerKind::Thick), ..]
        | [(_, SmallSpacerKind::Thick), (_, SmallSpacerKind::Medium), ..] => 2,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RuleLevel};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SMALL_SPACER_MERGE_TO_ENSPACE,
        level: Authoring,
        examples: [
        {
            label: thin_triple,
            packages: ["base"],
            input: r"A\,\,\,B",
            expected: r"A\enspace B",
        },
        {
            label: colon_semicolon,
            packages: ["base"],
            input: r"A\:\;B",
            expected: r"A\enspace B",
        },
        {
            label: semicolon_colon,
            packages: ["base"],
            input: r"A\;\:B",
            expected: r"A\enspace B",
        },
        {
            label: thin_multiple_one_pass,
            packages: ["base"],
            input: r"A\,\,\,\,\,\,\,B",
            expected: r"A\enspace\enspace\,B",
        },
        {
            label: mixed_pair_multiple_one_pass,
            packages: ["base"],
            input: r"A\:\;\;\:B",
            expected: r"A\enspace\enspace B",
        },
        {
            label: text_run_preserved,
            packages: ["base", "textmacros"],
            input: r"\text{A\,\,\,B}",
            expected: r"\text{A\,\,\,B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: SMALL_SPACER_MERGE_TO_ENSPACE,
        level: Authoring,
        examples: [
        {
            label: incomplete_chunks_are_preserved,
            packages: ["base"],
            input: r"A\,\,x\:\:y\;\;B",
            expected: r"A\,\,x\:\:y\;\;B",
        },
        {
            label: unmatched_prefix_before_chunk_is_preserved,
            packages: ["base"],
            input: r"A\,\:\;B",
            expected: r"A\,\enspace B",
        },
        {
            label: explicit_group_boundary_breaks_thin_chunk,
            packages: ["base"],
            input: r"A\,\,{\,}B",
            expected: r"A\,\,{\,}B",
        },
        {
            label: merges_inside_script_group,
            packages: ["base"],
            input: r"x^{\,\,\,}",
            expected: r"x^{\enspace}",
        },
        {
            label: argument_slots_do_not_form_chunks,
            packages: ["base"],
            input: r"\frac{\,\,}{\,}",
            expected: r"\frac{\,\,}{\,}",
        },
        {
            label: negative_space_breaks_thin_chunk,
            packages: ["base"],
            input: r"A\,\!\,\,B",
            expected: r"A\,\!\,\,B",
        },
        ]
    }

    #[test]
    fn rewrites_all_chunks_in_one_application() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"A\,\,\,\,\,\,\,B",
            &texform_core::parse::ParseConfig::STRICT,
        );

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &SMALL_SPACER_MERGE_TO_ENSPACE,
            RuleLevel::Authoring,
        )
        .expect("small-spacer merge should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        assert_eq!(output.rewrite.rules[0].applied_count, 1);
    }
}
