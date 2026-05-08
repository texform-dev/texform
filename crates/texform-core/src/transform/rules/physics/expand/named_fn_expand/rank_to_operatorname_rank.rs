//! Rewrite rank to an explicit operatorname form instead of a package-local alias.
//!
//! ```yaml
//! proposal: rank-to-operatorname-rank
//! consumes:
//!   eliminates: cmd:rank
//!   touches: null
//! produces: cmd:operatorname
//! rewrite_patterns:
//!   - {label: rank, from: \rank, to: '\operatorname{rank}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::physics;

use crate::ast::{ContentMode, GroupKind, Node};
use crate::transform::helpers::{mandatory_content, prefix_command, star};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite rank to an explicit operatorname form instead of a package-local alias.
    pub static RANK_TO_OPERATORNAME_RANK: RankToOperatornameRankRule {
        key: Physics / "rank-to-operatorname-rank",
        tier: Expand,
        summary: "Rewrite rank to an explicit operatorname form instead of a package-local alias.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::RANK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::OPERATORNAME],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::RANK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, command.args, r"\rank")?;

            let rank_children = "rank"
                .chars()
                .map(|ch| cx.ast.new_node(Node::Char(ch)))
                .collect();
            let rank_text = cx.ast.new_node(Node::Group {
                children: rank_children,
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            });
            cx.ast.replace_node(
                node_id,
                prefix_command(
                    &ams::cmd::OPERATORNAME,
                    vec![
                        star(false),
                        mandatory_content(rank_text, ContentMode::Math),
                    ],
                ),
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentKind, ArgumentValue};
    use crate::parse::ParseContext;
    use crate::transform::{TransformProfile, TransformRule as _, transform_ast};
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: RANK_TO_OPERATORNAME_RANK,
        tier: Expand,
        examples: [
        {
            label: rank_bare_operator,
            packages: ["base", "physics", "ams"],
            input: r"\rank A\le n",
            expected: r"\operatorname{rank} A\le n",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn output_matches_operatorname_argument_shape() {
        let parse_ctx = ParseContext::from_packages(&["base", "physics", "ams"]);
        let transform_ctx = TransformProfile::CORPUS
            .builder()
            .only(RANK_TO_OPERATORNAME_RANK.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\rank A", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("rank-to-operatorname-rank transform should succeed");

        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].count, 1);

        let children = ast.children(ast.root());
        let Node::Command { name, args, known } = ast.node(children[0]) else {
            panic!("expected operatorname command");
        };
        assert_eq!(name, ams::cmd::OPERATORNAME.name);
        assert!(*known);
        assert_eq!(args.len(), 2);

        let Some(star_arg) = &args[0] else {
            panic!("operatorname should carry an explicit false star slot");
        };
        assert_eq!(star_arg.kind, ArgumentKind::Star);
        assert_eq!(star_arg.value, ArgumentValue::Boolean(false));

        let Some(rank_arg) = &args[1] else {
            panic!("operatorname should carry a rank argument");
        };
        assert_eq!(rank_arg.kind, ArgumentKind::Mandatory);
        let ArgumentValue::MathContent(rank_group) = rank_arg.value else {
            panic!("rank argument should be math content");
        };
        let Node::Group { children, mode, .. } = ast.node(rank_group) else {
            panic!("rank argument should be a group");
        };
        assert_eq!(*mode, ContentMode::Math);
        assert_eq!(children.len(), 4);
        for (child, expected) in children.iter().copied().zip(['r', 'a', 'n', 'k']) {
            assert_eq!(ast.node(child), &Node::Char(expected));
        }
    }
}
