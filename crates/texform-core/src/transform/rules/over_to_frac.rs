//! Rewrite the TeX primitive infix `\over` to the LaTeX `\frac` command.
//!
//! In plain TeX, `a \over b` produces a fraction, but it uses an infix
//! notation that is discouraged in modern LaTeX. This rule normalizes it
//! to the prefix form `\frac{a}{b}`, which is the canonical representation
//! used by the rest of the pipeline.

use texform_specs::builtin::{ams, base};

use crate::ast::NodeId;
use crate::transform::context::TransformContext;
use crate::transform::engine::TransformError;
use crate::transform::helpers::{mandatory_content, prefix_command};
use crate::transform::rule::{
    RuleConsumes, RuleEffect, RuleGroup, RuleKey, RuleMeta, RulePhase, RuleProduces, RuleSafety,
    RuleTarget, RuleTrigger, TransformRule,
};

/// The singleton instance of the rule, registered in the global rule registry.
pub static OVER_TO_FRAC: OverToFracRule = OverToFracRule;

/// Rewrites the infix `\over` primitive into the prefix `\frac{…}{…}` form.
pub struct OverToFracRule;

impl TransformRule for OverToFracRule {
    fn meta(&self) -> &'static RuleMeta {
        // Metadata is defined as a function-local static so it stays colocated
        // with the rule implementation rather than drifting to a separate site.
        static META: RuleMeta = RuleMeta {
            key: RuleKey {
                group: RuleGroup::Structural,
                name: "over-to-frac",
            },
            summary: "Rewrite infix \\over into prefix \\frac",
            phase: RulePhase::Normalize,
            safety: RuleSafety::Semantic,
            triggers: &[RuleTrigger::Command(&base::cmd::OVER)],
            consumes: RuleConsumes {
                eliminates: &[RuleTarget::Command(&base::cmd::OVER)],
                requires: &[],
            },
            produces: RuleProduces {
                targets: &[
                    RuleTarget::Command(&base::cmd::FRAC),
                    RuleTarget::Command(&ams::cmd::FRAC),
                ],
            },
        };
        &META
    }

    fn apply(
        &self,
        cx: &mut TransformContext<'_>,
        node_id: NodeId,
    ) -> Result<RuleEffect, TransformError> {
        let Some(infix) = cx.match_infix(node_id, &base::cmd::OVER) else {
            return Ok(RuleEffect::Skipped);
        };
        cx.expect_no_args(self.meta().key, infix.args, "\\over")?;

        // Reuse the existing operand subtrees as the two mandatory frac args.
        cx.ast.replace_node(
            node_id,
            prefix_command(
                &base::cmd::FRAC,
                vec![
                    mandatory_content(infix.left),
                    mandatory_content(infix.right),
                ],
            ),
        );
        Ok(RuleEffect::Applied)
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
            "structural/over-to-frac"
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
                    ArgumentValue::Content(id) => id,
                    ref other => panic!("expected lhs content arg, got {:?}", other),
                };
                assert_eq!(output.ast.node(left_id), &Node::Char('a'));

                let right = args[1].as_ref().expect("frac rhs should exist");
                assert_eq!(right.kind, ArgumentKind::Mandatory);
                let right_id = match right.value {
                    ArgumentValue::Content(id) => id,
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

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].key.to_string(), "structural/over-to-frac");
        assert!(matches!(
            statuses[0].availability,
            RuleAvailability::Available
        ));
    }
}
