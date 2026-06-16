//! Expand buildrel syntax to an explicit relation-level stacked operator form.
//!
//! ```yaml
//! proposal: buildrel-expand
//! triggers:
//!   - cmd:buildrel
//! consumes:
//!   eliminates: cmd:buildrel
//!   touches: cmd:over
//! produces:
//!   - cmd:mathrel
//!   - cmd:mathop
//!   - cmd:limits
//! rewrite_patterns:
//!   - {from: '\buildrel #1 \over #2', to: '\mathrel{\mathop{#2}\limits^{#1}}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::stacked_operator_command;
use crate::ast::{ContentMode, GroupKind, Node, NodeId, Slot};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BUILDREL_EXPAND: BuildrelExpandRule {
        key: Base / "buildrel-expand",
        level: Expand,
        summary: "Expand buildrel syntax to an explicit relation-level stacked operator form.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::BUILDREL],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BUILDREL],
            touches: cmd_targets![&base::cmd::OVER],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::MATHREL, &base::cmd::MATHOP, &base::cmd::LIMITS],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::BUILDREL) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 1, r"\buildrel")?;
            let above_head =
                cx.for_rule(Self::KEY).mandatory_math_content(&command.args[0], r"\buildrel", "above")?;

            let Some(buildrel_link) = cx.ast.parent(node_id) else {
                return Ok(RuleEffect::Skipped);
            };

            let mut above_parts = above_parts_from_head(cx, above_head);
            let (buildrel_index, left_group) = match buildrel_link.slot {
                Slot::GroupChild(index) => (index, buildrel_link.parent),
                Slot::Argument(0) => {
                    return expand_direct_frac_wrapped_buildrel_parts(
                        cx,
                        buildrel_link.parent,
                        above_parts,
                    );
                }
                Slot::ScriptBase => {
                    let scripted = buildrel_link.parent;
                    let Some(parts) = above_parts_from_scripted_buildrel(cx, above_head, scripted) else {
                        return Ok(RuleEffect::Skipped);
                    };
                    above_parts = parts;

                    let Some(scripted_link) = cx.ast.parent(scripted) else {
                        return Ok(RuleEffect::Skipped);
                    };
                    match scripted_link.slot {
                        Slot::GroupChild(index) => (index, scripted_link.parent),
                        Slot::Argument(0) => {
                            return expand_direct_frac_wrapped_buildrel_parts(
                                cx,
                                scripted_link.parent,
                                above_parts,
                            );
                        }
                        _ => return Ok(RuleEffect::Skipped),
                    }
                }
                _ => return Ok(RuleEffect::Skipped),
            };
            let Some(over_link) = cx.ast.parent(left_group) else {
                return Ok(RuleEffect::Skipped);
            };
            if over_link.slot != Slot::InfixLeft {
                return expand_frac_wrapped_buildrel(cx, buildrel_index, left_group, above_parts);
            }
            let over_id = over_link.parent;
            let Some(infix) = cx.match_infix(over_id, &base::cmd::OVER) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(infix.args, r"\over")?;
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
            let replacement = cx.ast.new_node(replacement);

            cx.ast
                .replace_with_math_sequence(over_id, before, replacement, after);
            Ok(RuleEffect::Applied)
        }
    }
}

fn expand_direct_frac_wrapped_buildrel_parts(
    cx: &mut crate::rewrite::rule_context::RuleContext<'_>,
    frac_id: NodeId,
    above_parts: Vec<NodeId>,
) -> Result<RuleEffect, crate::rewrite::RuleError> {
    let Some(frac) = cx.match_command(frac_id, &base::cmd::FRAC) else {
        return Ok(RuleEffect::Skipped);
    };
    cx.for_rule(BuildrelExpandRule::KEY)
        .expect_arg_len(frac.args, 2, r"\frac")?;
    let right =
        cx.for_rule(BuildrelExpandRule::KEY)
            .mandatory_math_content(&frac.args[1], r"\frac", "denominator")?;

    expand_frac_wrapped_parts(cx, frac_id, Vec::new(), above_parts, right)
}

fn expand_frac_wrapped_buildrel(
    cx: &mut crate::rewrite::rule_context::RuleContext<'_>,
    buildrel_index: usize,
    left_group: NodeId,
    mut above_parts: Vec<NodeId>,
) -> Result<RuleEffect, crate::rewrite::RuleError> {
    let Some(frac_link) = cx.ast.parent(left_group) else {
        return Ok(RuleEffect::Skipped);
    };
    if frac_link.slot != Slot::Argument(0) {
        return Ok(RuleEffect::Skipped);
    }
    let frac_id = frac_link.parent;
    let Some(frac) = cx.match_command(frac_id, &base::cmd::FRAC) else {
        return Ok(RuleEffect::Skipped);
    };
    cx.for_rule(BuildrelExpandRule::KEY)
        .expect_arg_len(frac.args, 2, r"\frac")?;
    let right =
        cx.for_rule(BuildrelExpandRule::KEY)
            .mandatory_math_content(&frac.args[1], r"\frac", "denominator")?;

    let left_children = match cx.ast.node(left_group) {
        Node::Group {
            children,
            mode: ContentMode::Math,
            ..
        } => children.clone(),
        _ => return Ok(RuleEffect::Skipped),
    };

    let mut before = Vec::new();
    for &child in &left_children[..buildrel_index] {
        before.push(cx.ast.clone_subtree(child));
    }

    for &child in &left_children[buildrel_index + 1..] {
        above_parts.push(cx.ast.clone_subtree(child));
    }

    expand_frac_wrapped_parts(cx, frac_id, before, above_parts, right)
}

fn expand_frac_wrapped_parts(
    cx: &mut crate::rewrite::rule_context::RuleContext<'_>,
    frac_id: NodeId,
    before: Vec<NodeId>,
    above_parts: Vec<NodeId>,
    right: NodeId,
) -> Result<RuleEffect, crate::rewrite::RuleError> {
    let above = math_content_from_parts(cx, above_parts);

    let (operator_source, after_sources) = split_operator_and_after(cx, right);
    let operator = cx.ast.clone_subtree(operator_source);
    let after = after_sources
        .into_iter()
        .map(|child| cx.ast.clone_subtree(child))
        .collect();
    let replacement = stacked_operator_command(cx, &base::cmd::MATHREL, operator, above);
    let replacement = cx.ast.new_node(replacement);

    cx.ast
        .replace_with_math_sequence(frac_id, before, replacement, after);
    Ok(RuleEffect::Applied)
}

fn math_content_from_parts(cx: &mut crate::rewrite::rule_context::RuleContext<'_>, mut parts: Vec<NodeId>) -> NodeId {
    if parts.len() == 1 {
        parts.pop().expect("single-part vector should have one item")
    } else {
        cx.ast.implicit_math_group(parts)
    }
}

fn above_parts_from_head(
    cx: &mut crate::rewrite::rule_context::RuleContext<'_>,
    above_head: NodeId,
) -> Vec<NodeId> {
    let mut parts = Vec::new();
    cx.ast.append_cloned_math_content(&mut parts, above_head);
    parts
}

fn above_parts_from_scripted_buildrel(
    cx: &mut crate::rewrite::rule_context::RuleContext<'_>,
    above_head: NodeId,
    scripted: NodeId,
) -> Option<Vec<NodeId>> {
    let (subscript, superscript) = match cx.ast.node(scripted) {
        Node::Scripted {
            subscript,
            superscript,
            ..
        } => (*subscript, *superscript),
        _ => return None,
    };

    let base_parts = above_parts_from_head(cx, above_head);
    let base = math_content_from_parts(cx, base_parts);
    if subscript.is_none() && superscript.is_none() {
        return Some(vec![base]);
    }

    let subscript = subscript.map(|node| cx.ast.clone_subtree(node));
    let superscript = superscript.map(|node| cx.ast.clone_subtree(node));
    let scripted_above = cx.ast.new_node(Node::Scripted {
        base,
        subscript,
        superscript,
    });
    Some(vec![scripted_above])
}

fn split_operator_and_after(
    cx: &crate::rewrite::rule_context::RuleContext<'_>,
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
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BUILDREL_EXPAND,
        level: Expand,
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
        level: Expand,
        examples: [
        {
            label: braced_above_keeps_following_relation_operand,
            packages: ["base"],
            input: r"P \buildrel {n+1} \over \equiv Q",
            expected: r"P \mathrel{\mathop{\equiv}\limits^{n+1}} Q",
        },
        {
            label: repeated_buildrel_keeps_chain_order,
            packages: ["base"],
            input: r"\cdots\to K\buildrel f\over\longrightarrow K\buildrel f\over\longrightarrow K",
            expected: r"\cdots\to K\mathrel{\mathop{\longrightarrow}\limits^{f}} K\mathrel{\mathop{\longrightarrow}\limits^{f}} K",
        },
        {
            label: frac_wrapped_buildrel_relation,
            packages: ["base"],
            input: r"\frac{\buildrel Q}{\longrightarrow}",
            expected: r"\mathrel{\mathop{\longrightarrow}\limits^{Q}}",
        },
        {
            label: frac_wrapped_buildrel_keeps_above_subscript,
            packages: ["base"],
            input: r"\frac{\buildrel T_\alpha}{\longrightarrow}",
            expected: r"\mathrel{\mathop{\longrightarrow}\limits^{T_\alpha}}",
        },
        {
            label: over_buildrel_keeps_above_superscript,
            packages: ["base"],
            input: r"P \buildrel T^4 \over \rightarrow Q",
            expected: r"P \mathrel{\mathop{\rightarrow}\limits^{T^4}} Q",
        },
        {
            label: frac_wrapped_buildrel_keeps_surrounding_operands,
            packages: ["base"],
            input: r"\frac{P \buildrel{R\to\infty}}{= -E}",
            expected: r"P \mathrel{\mathop{=}\limits^{R\to\infty}} -E",
        },
        ]
    }

    #[test]
    fn faithful_profile_uses_buildrel_rule_before_over_to_frac() {
        // Faithful enables both Standard and Expand rules. This locks in the contract
        // that buildrel-expand gets this TeX shape instead of over-to-frac.
        let parse_ctx = crate::parse::ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"A_n \buildrel n\to\infty \over = B_n", &texform_core::parse::ParseConfig::STRICT);

        let context = crate::TransformContext::from_build_config(
            crate::BuildConfig::profile(crate::Profile::Faithful),
            &parse_ctx,
        )
        .expect("transform context should build");
        let report = context
            .run(&mut ast, &parse_ctx)
            .expect("transform should succeed");
        let actual = crate::serialize::serialize(&ast);
        let expected_ast = crate::parse_to_ast_for_test(&parse_ctx, r"A_n \mathrel{\mathop{=}\limits^{n\to\infty}} B_n", &texform_core::parse::ParseConfig::STRICT);
        let expected = crate::serialize::serialize(&expected_ast);

        assert_eq!(actual, expected);
        assert!(!actual.contains(r"\frac"));
        let buildrel_stat = report
            .rewrite
            .rules
            .iter()
            .find(|stat| stat.key.to_string() == "base/buildrel-expand")
            .expect("buildrel-expand should be attempted");
        assert_eq!(buildrel_stat.applied_count, 1);
        assert_eq!(buildrel_stat.skipped_count, 0);
    }
}
