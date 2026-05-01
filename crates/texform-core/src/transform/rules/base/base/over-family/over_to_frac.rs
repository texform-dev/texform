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
        key: Base / "over-to-frac",
        tier: Base,
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
    use crate::parse::ParseContext;
    use crate::transform::{transform_ast, TransformProfile};

    #[test]
    fn rewrites_infix_over_into_frac_command() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let transform_ctx = TransformProfile::AUTHORING
            .builder()
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"a \over b", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("over-to-frac transform should succeed");

        assert_eq!(output.iterations, 2);
        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].count, 1);
        assert_eq!(output.applied[0].key.to_string(), "base/over-to-frac");

        let root = ast.root();
        let children = ast.children(root);
        assert_eq!(children.len(), 1);

        match ast.node(children[0]) {
            Node::Command { name, args, .. } => {
                assert_eq!(name, "frac");
                assert_eq!(args.len(), 2);

                let left = args[0].as_ref().expect("frac lhs should exist");
                assert_eq!(left.kind, ArgumentKind::Mandatory);
                let left_id = match left.value {
                    ArgumentValue::MathContent(id) => id,
                    ref other => panic!("expected lhs content arg, got {:?}", other),
                };
                assert_eq!(ast.node(left_id), &Node::Char('a'));

                let right = args[1].as_ref().expect("frac rhs should exist");
                assert_eq!(right.kind, ArgumentKind::Mandatory);
                let right_id = match right.value {
                    ArgumentValue::MathContent(id) => id,
                    ref other => panic!("expected rhs content arg, got {:?}", other),
                };
                assert_eq!(ast.node(right_id), &Node::Char('b'));
            }
            other => panic!("expected frac command after transform, got {:?}", other),
        }
    }
}
