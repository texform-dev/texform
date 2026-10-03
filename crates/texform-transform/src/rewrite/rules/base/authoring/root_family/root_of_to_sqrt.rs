//! Rewrite legacy root-of syntax to bracketed sqrt notation.
//!
//! ```yaml
//! proposal: root-of-to-sqrt
//! triggers:
//!   - cmd:root
//! consumes:
//!   eliminates: cmd:root
//! produces: cmd:sqrt
//! rewrite_patterns:
//!   - {from: '\root #1 \of #2', to: '\sqrt[#1]{#2}'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter, NodeId};
use crate::rewrite::helpers::{mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleKey, RuleProduces};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static ROOT_OF_TO_SQRT: RootOfToSqrtRule {
        key: Base / "root-of-to-sqrt",
        level: Authoring,
        summary: "Rewrite legacy root-of syntax to bracketed sqrt notation.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::ROOT],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ROOT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::SQRT],
        },
        apply(rule, cx, node_id) {
            rewrite_root_of(Self::KEY, cx, node_id)
        }
    }
}

fn rewrite_root_of(
    rule_key: RuleKey,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
) -> Result<RuleEffect, crate::rewrite::RuleError> {
    let Some(root) = cx.match_command(node_id, &base::cmd::ROOT) else {
        return Ok(RuleEffect::Skipped);
    };
    cx.for_rule(rule_key).expect_arg_len(root.args, 2, "\\root")?;
    let degree = match &root.args[0] {
        Some(Argument {
            kind:
                ArgumentKind::Until {
                    close: Delimiter::Control(name),
                },
            value: ArgumentValue::MathContent(degree),
            ..
        }) if name == "of" => *degree,
        _ => return Err(cx.for_rule(rule_key).invalid_shape("\\root degree should end at \\of")),
    };
    let radicand = cx.for_rule(rule_key).mandatory_math_content(&root.args[1], "\\root", "radicand")?;

    cx.ast.replace_node(
        node_id,
        prefix_command_node(
            &base::cmd::SQRT,
            vec![
                Some(Argument::from_value(
                    ArgumentKind::Optional,
                    ArgumentValue::MathContent(degree),
                )),
                mandatory_content_slot(radicand, ContentMode::Math),
            ],
        ),
    );

    Ok(RuleEffect::Applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentKind, ArgumentValue, Node};
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RuleLevel};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ROOT_OF_TO_SQRT,
        level: Authoring,
        examples: [
        {
            label: compound_root,
            packages: ["base"],
            input: r"\root 1+2 \of {x+y}",
            expected: r"\sqrt[1+2]{x+y}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: ROOT_OF_TO_SQRT,
        level: Authoring,
        examples: [
        {
            label: braced_degree,
            packages: ["base"],
            input: r"\root {1+2} \of x",
            expected: r"\sqrt[{1+2}]{x}",
        },
        {
            label: bare_radicand_keeps_following_siblings,
            packages: ["base"],
            input: r"a+\root n \of y+z",
            expected: r"a+\sqrt[n]{y}+z",
        },
        {
            label: single_group_degree_loses_one_brace_layer,
            packages: ["base"],
            input: r"\root{1+2}\of x",
            expected: r"\sqrt[1+2]{x}",
        },
        {
            label: unbraced_scripted_degree,
            packages: ["base"],
            input: r"\root n_i\of{x}",
            expected: r"\sqrt[n_i]{x}",
        },
        {
            label: braced_terminator_stays_in_degree,
            packages: ["base"],
            input: r"\root{a\of b}\of x",
            expected: r"\sqrt[a\of b]{x}",
        },
        {
            label: empty_degree_keeps_empty_optional,
            packages: ["base"],
            input: r"\root\of{x}",
            expected: r"\sqrt[]{x}",
        },
        {
            label: scripts_after_radicand_bind_to_root,
            packages: ["base"],
            input: r"\root 3\of{x}^2",
            expected: r"\sqrt[3]{x}^2",
        },
        ]
    }

    #[test]
    fn rewrites_root_of_into_sqrt_with_optional_degree() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"\root 1+2 \of {x+y}", &texform_core::parse::ParseConfig::STRICT);

        let output =
            run_one_rule_for_test(&mut ast, &parse_ctx, &ROOT_OF_TO_SQRT, RuleLevel::Authoring)
            .expect("root-of-to-sqrt transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        assert_eq!(output.rewrite.rules[0].applied_count, 1);
        assert_eq!(output.rewrite.rules[0].key.to_string(), "base/root-of-to-sqrt");

        let children = ast.children(ast.root());
        assert_eq!(children.len(), 1);

        match ast.node(children[0]) {
            Node::Command { name, args, .. } => {
                assert_eq!(name, "sqrt");
                assert_eq!(args.len(), 2);

                let degree = args[0].as_ref().expect("sqrt degree should exist");
                assert_eq!(degree.kind, ArgumentKind::Optional);
                let ArgumentValue::MathContent(degree_id) = degree.value else {
                    panic!("expected math content degree, got {:?}", degree.value);
                };
                assert_eq!(
                    ast.children(degree_id)
                        .iter()
                        .map(|&child| ast.node(child))
                        .collect::<Vec<_>>(),
                    vec![&Node::Char('1'), &Node::Char('+'), &Node::Char('2')]
                );

                let radicand = args[1].as_ref().expect("sqrt radicand should exist");
                assert_eq!(radicand.kind, ArgumentKind::Mandatory);
                let ArgumentValue::MathContent(radicand_id) = radicand.value else {
                    panic!("expected math content radicand, got {:?}", radicand.value);
                };
                assert_eq!(
                    ast.children(radicand_id)
                        .iter()
                        .map(|&child| ast.node(child))
                        .collect::<Vec<_>>(),
                    vec![&Node::Char('x'), &Node::Char('+'), &Node::Char('y')]
                );
            }
            other => panic!("expected sqrt command after transform, got {:?}", other),
        }
    }
}
