//! Expand buildrel syntax to an explicit relation-class stacked operator form.
//!
//! ```yaml
//! proposal: buildrel-expand
//! consumes:
//!   eliminates: cmd:buildrel
//!   touches: null
//! produces:
//!   - cmd:mathrel
//!   - cmd:mathop
//!   - cmd:limits
//! rewrite_patterns:
//!   - {from: '\buildrel #1 \over #2', to: '\mathrel{\mathop{#2}\limits^{#1}}'}
//! ```

use texform_specs::builtin::base;

use super::helpers::stacked_operator_command;
use crate::ast::{ContentMode, GroupKind, Node, NodeId, Slot};
use crate::transform::helpers::{
    append_cloned_math_content, implicit_math_group, replace_with_math_sequence,
    required_math_content,
};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand buildrel syntax to an explicit relation-class stacked operator form.
    pub static BUILDREL_EXPAND: BuildrelExpandRule {
        key: Base / "buildrel-expand",
        class: Expand,
        summary: "Expand buildrel syntax to an explicit relation-class stacked operator form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BUILDREL],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::MATHREL, &base::cmd::MATHOP, &base::cmd::LIMITS],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::BUILDREL) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_arg_len(rule.meta().key, command.args, 1, r"\buildrel")?;
            let above_head =
                required_math_content(rule.meta().key, cx, &command.args[0], r"\buildrel", "above")?;

            let Some(buildrel_link) = cx.ast.parent(node_id) else {
                return Ok(RuleEffect::Skipped);
            };
            let Slot::GroupChild(buildrel_index) = buildrel_link.slot else {
                return Ok(RuleEffect::Skipped);
            };
            let left_group = buildrel_link.parent;
            let Some(over_link) = cx.ast.parent(left_group) else {
                return Ok(RuleEffect::Skipped);
            };
            if over_link.slot != Slot::InfixLeft {
                return Ok(RuleEffect::Skipped);
            }
            let over_id = over_link.parent;
            let Some(infix) = cx.match_infix(over_id, &base::cmd::OVER) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, infix.args, r"\over")?;
            // This rule owns \buildrel ... \over ... before over-to-frac can treat
            // the same \over as a generic fraction infix.
            let right = infix.right;

            let left_children = match cx.ast.node(left_group) {
                Node::Group {
                    children,
                    kind: GroupKind::Implicit,
                    mode: ContentMode::Math,
                } => children.clone(),
                _ => return Ok(RuleEffect::Skipped),
            };

            let mut before = Vec::new();
            for &child in &left_children[..buildrel_index] {
                before.push(cx.ast.clone_subtree(child));
            }

            let mut above_parts = Vec::new();
            append_cloned_math_content(cx, &mut above_parts, above_head);
            for &child in &left_children[buildrel_index + 1..] {
                above_parts.push(cx.ast.clone_subtree(child));
            }
            let above = math_content_from_parts(cx, above_parts);

            let (operator_source, after_sources) = split_operator_and_after(cx, right);
            let operator = cx.ast.clone_subtree(operator_source);
            let after = after_sources
                .into_iter()
                .map(|child| cx.ast.clone_subtree(child))
                .collect();
            let replacement = stacked_operator_command(cx, &base::cmd::MATHREL, operator, above);

            replace_with_math_sequence(cx, over_id, before, replacement, after);
            Ok(RuleEffect::Applied)
        }
    }
}

fn math_content_from_parts(cx: &mut crate::transform::rule_context::RuleContext<'_>, mut parts: Vec<NodeId>) -> NodeId {
    if parts.len() == 1 {
        parts.pop().expect("single-part vector should have one item")
    } else {
        implicit_math_group(cx, parts)
    }
}

fn split_operator_and_after(
    cx: &crate::transform::rule_context::RuleContext<'_>,
    right: NodeId,
) -> (NodeId, Vec<NodeId>) {
    match cx.ast.node(right) {
        Node::Group {
            children,
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        } if !children.is_empty() => (children[0], children[1..].to_vec()),
        _ => (right, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;
    use crate::transform::{TransformProfile, transform_ast};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BUILDREL_EXPAND,
        class: Expand,
        examples: [
        {
            label: asymptotic_relation_stack,
            packages: ["base"],
            input: r"A_n \buildrel n\to\infty \over = B_n",
            expected: r"A_n \mathrel{\mathop{=}\limits^{n\to\infty}} B_n",
        },
        {
            label: arrow_relation_stack,
            packages: ["base"],
            input: r"X \buildrel \phi \over \longrightarrow Y",
            expected: r"X \mathrel{\mathop{\longrightarrow}\limits^{\phi}} Y",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: BUILDREL_EXPAND,
        class: Expand,
        examples: [
        {
            label: braced_above_keeps_following_relation_operand,
            packages: ["base"],
            input: r"P \buildrel {n+1} \over \equiv Q",
            expected: r"P \mathrel{\mathop{\equiv}\limits^{n+1}} Q",
        },
        ]
    }

    #[test]
    fn corpus_profile_uses_buildrel_rule_before_over_to_frac() {
        // CORPUS enables both Base and Expand rules. This locks in the contract
        // that buildrel-expand gets this TeX shape instead of over-to-frac.
        let parse_ctx = crate::parse::ParseContext::from_packages(&["base"]);
        let transform_ctx = TransformProfile::CORPUS
            .builder()
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"A_n \buildrel n\to\infty \over = B_n", true)
            .expect("parse input should succeed");

        transform_ast(&mut ast, &parse_ctx, &transform_ctx).expect("transform should succeed");
        let actual = crate::serialize::serialize(&ast);
        let expected_ast = parse_ctx
            .parse_to_ast(r"A_n \mathrel{\mathop{=}\limits^{n\to\infty}} B_n", true)
            .expect("parse expected should succeed");
        let expected = crate::serialize::serialize(&expected_ast);

        assert_eq!(actual, expected);
        assert!(!actual.contains(r"\frac"));
    }
}
