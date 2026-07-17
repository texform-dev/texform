//! Drop one directly nested duplicate mathrel wrapper.
//!
//! ```yaml
//! proposal: duplicate-mathrel-wrapper-drop
//! triggers:
//!   - cmd:mathrel
//! consumes:
//!   eliminates: null
//!   touches: cmd:mathrel
//! produces: null
//! rewrite_patterns:
//!   - {from: '\mathrel{\mathrel{#1}}', to: '\mathrel{#1}'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{Ast, ContentMode, GroupKind, Node, NodeId};
use crate::rewrite::helpers::{mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static DUPLICATE_MATHREL_WRAPPER_DROP: DuplicateMathrelWrapperDropRule {
        key: Base / "duplicate-mathrel-wrapper-drop",
        level: Authoring,
        summary: "Drop one directly nested duplicate mathrel wrapper.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::MATHREL],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::MATHREL],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(_rule, cx, node_id) {
            let inner_content = {
                let Some(outer) = cx.match_command(node_id, &base::cmd::MATHREL) else {
                    return Ok(RuleEffect::Skipped);
                };
                let subject = outer.subject();
                let scoped = cx.for_rule(Self::KEY);
                scoped.expect_arg_len(outer.args, 1, &subject)?;
                let outer_content =
                    scoped.mandatory_math_content(&outer.args[0], &subject, "content")?;

                let Some(inner_id) = direct_math_content_node(cx.ast, outer_content) else {
                    return Ok(RuleEffect::Skipped);
                };
                let Some(inner) = cx.match_command(inner_id, &base::cmd::MATHREL) else {
                    return Ok(RuleEffect::Skipped);
                };
                let inner_subject = inner.subject();
                scoped.expect_arg_len(inner.args, 1, &inner_subject)?;
                scoped.mandatory_math_content(&inner.args[0], &inner_subject, "content")?
            };

            let content = cx.ast.clone_subtree(inner_content);
            cx.ast.replace_node_drop_detached_children(
                node_id,
                prefix_command_node(
                    &base::cmd::MATHREL,
                    vec![mandatory_content_slot(content, ContentMode::Math)],
                ),
            );
            Ok(RuleEffect::Applied)
        }
    }
}

fn direct_math_content_node(ast: &Ast, node_id: NodeId) -> Option<NodeId> {
    match ast.node(node_id) {
        Node::Group {
            children,
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        } => {
            let [child] = children.as_slice() else {
                return None;
            };
            Some(*child)
        }
        Node::Group { .. } => None,
        _ => Some(node_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DUPLICATE_MATHREL_WRAPPER_DROP,
        level: Authoring,
        examples: [
        {
            label: direct_duplicate,
            packages: ["base"],
            input: r"a\mathrel{\mathrel{=}}b",
            expected: r"a\mathrel{=}b",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: DUPLICATE_MATHREL_WRAPPER_DROP,
        level: Authoring,
        examples: [
        {
            label: fixed_point_collapses_deeper_duplicates,
            packages: ["base"],
            input: r"a\mathrel{\mathrel{\mathrel{=}}}b",
            expected: r"a\mathrel{=}b",
        },
        {
            label: preserves_multiple_inner_content_nodes,
            packages: ["base"],
            input: r"a\mathrel{\mathrel{=x}}b",
            expected: r"a\mathrel{=x}b",
        },
        {
            label: preserves_empty_inner_content,
            packages: ["base"],
            input: r"a\mathrel{\mathrel{}}b",
            expected: r"a\mathrel{}b",
        },
        {
            label: singleton_wrapper_is_preserved,
            packages: ["base"],
            input: r"a\mathrel{=}b",
            expected: r"a\mathrel{=}b",
        },
        {
            label: mixed_wrapper_is_preserved,
            packages: ["base"],
            input: r"a\mathrel{\mathbin{=}}b",
            expected: r"a\mathrel{\mathbin{=}}b",
        },
        {
            label: outer_sibling_is_preserved,
            packages: ["base"],
            input: r"a\mathrel{x\mathrel{=}}b",
            expected: r"a\mathrel{x\mathrel{=}}b",
        },
        {
            label: explicit_group_boundary_is_preserved,
            packages: ["base"],
            input: r"a\mathrel{{\mathrel{=}}}b",
            expected: r"a\mathrel{{\mathrel{=}}}b",
        },
        {
            label: scripted_inner_wrapper_is_preserved,
            packages: ["base"],
            input: r"a\mathrel{\mathrel{=}^{x}}b",
            expected: r"a\mathrel{\mathrel{=}^{x}}b",
        },
        ]
    }
}
