//! Rewrite infix over to an explicit frac command.
//!
//! ```yaml
//! proposal: over-to-frac
//! consumes:
//!   eliminates: cmd:over
//!   touches: null
//! produces: cmd:frac
//! rewrite_patterns:
//!   - {label: over, from: '#1 \over #2', to: '\frac{#1}{#2}'}
//! ```

use texform_specs::builtin::base;

use crate::ast::{ContentMode, GroupKind, Node};
use crate::transform::helpers::{mandatory_content, prefix_command};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite infix over to an explicit frac command.
    pub static OVER_TO_FRAC: OverToFracRule {
        key: Base / "over-to-frac",
        tier: Base,
        summary: "Rewrite infix over to an explicit frac command.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::OVER],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC],
        },
        apply(rule, cx, node_id) {
            let Some(infix) = cx.match_infix(node_id, &base::cmd::OVER) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, infix.args, "\\over")?;
            // \buildrel uses TeX's \buildrel <above> \over <operator> shape; leave
            // that infix form for buildrel-expand instead of turning it into \frac.
            if contains_command(cx, infix.left, base::cmd::BUILDREL.name) {
                return Ok(RuleEffect::Skipped);
            }
            let unwrap_parent_id = cx.ast.parent_id(node_id).and_then(|parent_id| {
                match cx.ast.node(parent_id) {
                    Node::Group {
                        children,
                        kind: GroupKind::Explicit,
                        ..
                    } if children.as_slice() == [node_id] => Some(parent_id),
                    _ => None,
                }
            });
            let frac_args = vec![
                mandatory_content(infix.left, ContentMode::Math),
                mandatory_content(infix.right, ContentMode::Math),
            ];

            // Reuse the existing operand subtrees as the two mandatory frac args.
            if let Some(parent_id) = unwrap_parent_id {
                cx.ast.replace_node(node_id, Node::Text(String::new()));
                cx.ast.replace_node(
                    parent_id,
                    Node::Command {
                        name: base::cmd::FRAC.name.to_string(),
                        args: frac_args,
                        known: true,
                    },
                );
                cx.ast.remove_detached(node_id);
            } else {
                cx.ast
                    .replace_node(node_id, prefix_command(&base::cmd::FRAC, frac_args));
            }
            Ok(RuleEffect::Applied)
        }
    }
}

fn contains_command(
    cx: &crate::transform::rule_context::RuleContext<'_>,
    node_id: crate::ast::NodeId,
    command_name: &str,
) -> bool {
    cx.ast.find_all(node_id, |node| {
        matches!(node, Node::Command { name, .. } if name == command_name)
    }).len() > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentKind, ArgumentValue};
    use crate::parse::ParseContext;
    use crate::transform::transform_examples;
    use crate::transform::{TransformProfile, transform_ast};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: OVER_TO_FRAC,
        tier: Base,
        examples: [
        {
            label: stacked_over_fraction,
            packages: ["base"],
            input: r"(x_1+\cdots+x_m)^2 \over \sum_{j=1}^{n}(y_j^2+1)",
            expected: r"\frac{(x_1+\cdots+x_m)^2}{\sum_{j=1}^{n}(y_j^2+1)}",
        },
        {
            label: braced_over_in_expression,
            packages: ["base"],
            input: r"1+{a+b \over c+d}",
            expected: r"1+\frac{a+b}{c+d}",
        },
        ]
    }
    // END: Generated examples

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
