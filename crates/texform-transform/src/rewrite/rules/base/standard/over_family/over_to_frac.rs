//! Rewrite infix over to an explicit frac command.
//!
//! ```yaml
//! proposal: over-to-frac
//! triggers:
//!   - cmd:over
//! consumes:
//!   eliminates: cmd:over
//!   touches: null
//! produces: cmd:frac
//! rewrite_patterns:
//!   - {from: '#1 \over #2', to: '\frac{#1}{#2}'}
//! ```

use texform_specs::builtin::base;

use super::helpers::replace_infix_with_command;
use crate::ast::ContentMode;
use crate::rewrite::helpers::infix_prefix_args;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static OVER_TO_FRAC: OverToFracRule {
        key: Base / "over-to-frac",
        class: Standard,
        summary: "Rewrite infix over to an explicit frac command.",
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::OVER],
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
            cx.for_rule(Self::KEY).expect_no_args(infix.args, "\\over")?;
            // \buildrel uses TeX's \buildrel <above> \over <operator> shape; leave
            // that infix form for buildrel-expand instead of turning it into \frac.
            if cx.ast.subtree_contains_command(infix.left, base::cmd::BUILDREL.name) {
                return Ok(RuleEffect::Skipped);
            }
            replace_infix_with_command(
                cx,
                node_id,
                &base::cmd::FRAC,
                infix_prefix_args(infix.left, infix.right, ContentMode::Math),
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentKind, ArgumentValue, Node};
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RuleClass};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: OVER_TO_FRAC,
        class: Standard,
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
        let mut ast = parse_ctx
            .parse_to_ast(r"a \over b", &texform_core::parse::ParseConfig::STRICT)
            .expect("parse should succeed");

        let output =
            run_one_rule_for_test(&mut ast, &parse_ctx, &OVER_TO_FRAC, RuleClass::Standard)
            .expect("over-to-frac transform should succeed");

        assert_eq!(output.rewrite.iterations, 2);
        assert_eq!(output.rewrite.applied.len(), 1);
        assert_eq!(output.rewrite.applied[0].count, 1);
        assert_eq!(output.rewrite.applied[0].key.to_string(), "base/over-to-frac");

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
