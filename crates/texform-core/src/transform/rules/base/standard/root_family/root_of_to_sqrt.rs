//! Rewrite legacy root-of syntax to bracketed sqrt notation.
//!
//! ```yaml
//! proposal: root-of-to-sqrt
//! triggers:
//!   - cmd:root
//! consumes:
//!   eliminates: cmd:root
//!   touches: cmd:of
//! produces: cmd:sqrt
//! rewrite_patterns:
//!   - {from: '\root #1 \of #2', to: '\sqrt[#1]{#2}'}
//! ```

use texform_specs::builtin::base;

use crate::ast::{Argument, ArgumentKind, ArgumentValue, ContentMode, GroupKind, Node, NodeId, Slot};
use crate::transform::helpers::{mandatory_content, prefix_command, required_math_content};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleKey, RuleProduces};
use crate::transform::rule_context::RuleContext;
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static ROOT_OF_TO_SQRT: RootOfToSqrtRule {
        key: Base / "root-of-to-sqrt",
        class: Standard,
        summary: "Rewrite legacy root-of syntax to bracketed sqrt notation.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::ROOT],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ROOT],
            touches: cmd_targets![&base::cmd::OF],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::SQRT],
        },
        apply(rule, cx, node_id) {
            rewrite_root_of(rule.meta().key, cx, node_id)
        }
    }
}

fn rewrite_root_of(
    rule_key: RuleKey,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
) -> Result<RuleEffect, crate::transform::engine::TransformError> {
    let Some(root) = cx.match_command(node_id, &base::cmd::ROOT) else {
        return Ok(RuleEffect::Skipped);
    };
    cx.expect_arg_len(rule_key, root.args, 1, "\\root")?;
    let degree_head = required_math_content(rule_key, cx, &root.args[0], "\\root", "degree")?;

    let Some(parent_link) = cx.ast.parent(node_id) else {
        return Err(cx.invalid_shape(rule_key, "\\root should be attached to a parent"));
    };
    let Slot::GroupChild(root_index) = parent_link.slot else {
        return Err(cx.invalid_shape(
            rule_key,
            "\\root should appear as math-list content",
        ));
    };

    let siblings = cx.ast.children(parent_link.parent).to_vec();
    let Some(of_index) = siblings
        .iter()
        .enumerate()
        .skip(root_index + 1)
        .find_map(|(index, &sibling)| cx.match_command(sibling, &base::cmd::OF).map(|_| index))
    else {
        return Err(cx.invalid_shape(
            rule_key,
            "\\root should be followed by \\of",
        ));
    };
    let of = cx
        .match_command(siblings[of_index], &base::cmd::OF)
        .expect("\\of index should still refer to an \\of command");
    cx.expect_no_args(rule_key, of.args, "\\of")?;

    if siblings.get(of_index + 1).is_none() {
        return Err(cx.invalid_shape(
            rule_key,
            "\\of should be followed by a radicand",
        ));
    }

    cx.ast.replace_node(node_id, Node::Text(String::new()));

    let parent_id = parent_link.parent;
    let mut degree_tail = Vec::new();
    for _ in (root_index + 1)..of_index {
        let child = cx.ast.children(parent_id)[root_index + 1];
        degree_tail.push(cx.ast.detach(child));
    }

    let of_node = cx.ast.children(parent_id)[root_index + 1];
    let detached_of = cx.ast.detach(of_node);
    cx.ast.remove_detached(detached_of);

    let radicand = cx.ast.children(parent_id)[root_index + 1];
    let radicand = cx.ast.detach(radicand);

    let degree = if degree_tail.is_empty() {
        degree_head
    } else {
        let mut degree_children = Vec::with_capacity(degree_tail.len() + 1);
        degree_children.push(degree_head);
        degree_children.extend(degree_tail);
        cx.ast.new_node(Node::Group {
            children: degree_children,
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        })
    };

    cx.ast.replace_node(
        node_id,
        prefix_command(
            &base::cmd::SQRT,
            vec![
                Some(Argument {
                    kind: ArgumentKind::Optional,
                    value: ArgumentValue::MathContent(degree),
                }),
                mandatory_content(radicand, ContentMode::Math),
            ],
        ),
    );

    Ok(RuleEffect::Applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentKind, ArgumentValue};
    use crate::parse::ParseContext;
    use crate::transform::TransformRule as _;
    use crate::transform::transform_examples;
    use crate::transform::{TransformProfile, transform_ast};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ROOT_OF_TO_SQRT,
        class: Standard,
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
        class: Standard,
        examples: [
        {
            label: braced_degree,
            packages: ["base"],
            input: r"\root {1+2} \of x",
            expected: r"\sqrt[1+2]{x}",
        },
        {
            label: bare_radicand_keeps_following_siblings,
            packages: ["base"],
            input: r"a+\root n \of y+z",
            expected: r"a+\sqrt[n]{y}+z",
        },
        ]
    }

    #[test]
    fn rewrites_root_of_into_sqrt_with_optional_degree() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let transform_ctx = TransformProfile::AUTHORING
            .builder()
            .only(ROOT_OF_TO_SQRT.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\root 1+2 \of {x+y}", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("root-of-to-sqrt transform should succeed");

        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].count, 1);
        assert_eq!(output.applied[0].key.to_string(), "base/root-of-to-sqrt");

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
