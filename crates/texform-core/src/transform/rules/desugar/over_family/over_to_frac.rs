//! Rewrite the TeX primitive infix `\over` to the LaTeX `\frac` command.
//!
//! In plain TeX, `a \over b` produces a fraction, but it uses an infix
//! notation that is discouraged in modern LaTeX. This rule normalizes it
//! to the prefix form `\frac{a}{b}`, which is the canonical representation
//! used by the rest of the pipeline.

use texform_specs::builtin::{ams, base};

use crate::ast::ContentMode;
use crate::transform::helpers::{mandatory_content, prefix_command};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, cmd_triggers, define_rule};

define_rule! {
    /// Rewrites the infix `\over` primitive into the prefix `\frac{…}{…}` form.
    pub static OVER_TO_FRAC: OverToFracRule {
        key: Desugar / "over-to-frac",
        summary: "Rewrite infix \\over into prefix \\frac",
        phase: Normalize,
        safety: Semantic,
        triggers: cmd_triggers![&base::cmd::OVER],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::OVER],
            requires: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC, &ams::cmd::FRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::OVER) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, infix.args, "\\over")?;

            // Reuse the existing operand subtrees as the two mandatory frac args.
            cx.ast.replace_node(
                node_id,
                prefix_command(
                    &base::cmd::FRAC,
                    vec![
                        mandatory_content(infix.left, ContentMode::Math),
                        mandatory_content(infix.right, ContentMode::Math),
                    ],
                ),
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{ArgumentKind, ArgumentValue, Node};
    use crate::context::ParseContext;
    use crate::transform::{RuleAvailability, TransformProfile};

    #[test]
    fn rewrites_infix_over_into_frac_command() {
        let ctx = ParseContext::from_packages(&["base"]);
        let output = ctx
            .parse_and_transform(r"a \over b", true, &TransformProfile::default())
            .expect("over-to-frac transform should succeed");

        assert_eq!(output.transform_report.iterations, 2);
        assert_eq!(output.transform_report.applied.len(), 1);
        assert_eq!(output.transform_report.applied[0].count, 1);
        assert_eq!(
            output.transform_report.applied[0].key.to_string(),
            "desugar/over-to-frac"
        );

        let root = output.ast.root();
        let children = output.ast.children(root);
        assert_eq!(children.len(), 1);

        match output.ast.node(children[0]) {
            Node::Command { name, args } => {
                assert_eq!(name, "frac");
                assert_eq!(args.len(), 2);

                let left = args[0].as_ref().expect("frac lhs should exist");
                assert_eq!(left.kind, ArgumentKind::Mandatory);
                let left_id = match left.value {
                    ArgumentValue::MathContent(id) => id,
                    ref other => panic!("expected lhs content arg, got {:?}", other),
                };
                assert_eq!(output.ast.node(left_id), &Node::Char('a'));

                let right = args[1].as_ref().expect("frac rhs should exist");
                assert_eq!(right.kind, ArgumentKind::Mandatory);
                let right_id = match right.value {
                    ArgumentValue::MathContent(id) => id,
                    ref other => panic!("expected rhs content arg, got {:?}", other),
                };
                assert_eq!(output.ast.node(right_id), &Node::Char('b'));
            }
            other => panic!("expected frac command after transform, got {:?}", other),
        }
    }

    #[test]
    fn reports_rule_as_available_for_base_profile() {
        let ctx = ParseContext::from_packages(&["base"]);
        let statuses = ctx
            .transform_rule_statuses(&TransformProfile::default())
            .expect("profile compilation should succeed");

        let status = statuses
            .iter()
            .find(|status| status.key.to_string() == "desugar/over-to-frac")
            .expect("over-to-frac status should exist");
        assert!(matches!(status.availability, RuleAvailability::Available));
    }
}
