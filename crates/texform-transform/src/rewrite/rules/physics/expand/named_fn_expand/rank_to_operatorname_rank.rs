//! Rewrite rank to an explicit operatorname form instead of a package-local alias.
//!
//! ```yaml
//! proposal: rank-to-operatorname-rank
//! triggers:
//!   - cmd:rank
//! consumes:
//!   eliminates: cmd:rank
//!   touches: null
//! produces: cmd:operatorname
//! rewrite_patterns:
//!   - {from: \rank, to: '\operatorname{rank}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::physics;

use crate::ast::{ContentMode, GroupKind, Node};
use crate::rewrite::helpers::{mandatory_content_slot, prefix_command_node, star_slot};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static RANK_TO_OPERATORNAME_RANK: RankToOperatornameRankRule {
        key: Physics / "rank-to-operatorname-rank",
        level: Expand,
        summary: "Rewrite rank to an explicit operatorname form instead of a package-local alias.",
        fidelity: Full,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::RANK],
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
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\rank")?;

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
                prefix_command_node(
                    &ams::cmd::OPERATORNAME,
                    vec![
                        star_slot(false),
                        mandatory_content_slot(rank_text, ContentMode::Math),
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
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, NormalizationLevel};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: RANK_TO_OPERATORNAME_RANK,
        level: Expand,
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
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"\rank A", &texform_core::parse::ParseConfig::STRICT);

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &RANK_TO_OPERATORNAME_RANK,
            NormalizationLevel::Expand,
        )
            .expect("rank-to-operatorname-rank transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        assert_eq!(output.rewrite.rules[0].applied_count, 1);

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
