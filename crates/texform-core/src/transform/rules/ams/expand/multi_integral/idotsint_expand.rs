//! Expand idotsint to explicit repeated integral surfaces.
//!
//! ```yaml
//! proposal: idotsint-expand
//! consumes:
//!   eliminates: cmd:idotsint
//!   touches: null
//! produces:
//!   - cmd:!
//!   - cmd:mathop
//!   - cmd:,
//!   - cmd:limits
//! rewrite_patterns:
//!   - {from: \idotsint, to: \int\cdots\int}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;
use texform_specs::specs::BuiltinCharacterRecord;

use crate::ast::{ContentMode, Node, NodeId, Slot};
use crate::transform::helpers::{
    implicit_math_group, mandatory_content, prefix_command, replace_with_math_sequence,
};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand idotsint to explicit repeated integral surfaces.
    pub static IDOTSINT_EXPAND: IdotsintExpandRule {
        key: Ams / "idotsint-expand",
        class: Expand,
        summary: "Expand idotsint to explicit repeated integral surfaces.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Ams],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&ams::cmd::IDOTSINT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![
                &base::cmd::_EXCLAMATION,
                &base::cmd::MATHOP,
                &base::cmd::_COMMA,
                &base::cmd::LIMITS,
            ],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &ams::cmd::IDOTSINT) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, command.args, r"\idotsint")?;

            if replace_idotsint_followed_by_scripted_limits(cx, node_id)? {
                return Ok(RuleEffect::Applied);
            }
            if replace_idotsint_as_script_base(cx, node_id) {
                return Ok(RuleEffect::Applied);
            }

            replace_plain_idotsint(cx, node_id);
            Ok(RuleEffect::Applied)
        }
    }
}

fn replace_plain_idotsint(cx: &mut crate::transform::rule_context::RuleContext<'_>, node_id: NodeId) {
    replace_with_math_nodes(
        cx,
        node_id,
        vec![integral_command(), cdots_command(), integral_command()],
    );
}

fn replace_idotsint_as_script_base(
    cx: &mut crate::transform::rule_context::RuleContext<'_>,
    node_id: NodeId,
) -> bool {
    if cx.ast.slot(node_id) != Some(Slot::ScriptBase) {
        return false;
    }
    let Some(parent) = cx.ast.parent_id(node_id) else {
        return false;
    };
    let (subscript, superscript) = match cx.ast.node(parent) {
        Node::Scripted {
            base,
            subscript,
            superscript,
        } if *base == node_id => (*subscript, *superscript),
        _ => return false,
    };

    let (subscript, superscript) = clone_script_attachments(cx, subscript, superscript);
    let scripted_integral = Node::Scripted {
        base: cx.ast.new_node(integral_command()),
        subscript,
        superscript,
    };

    replace_with_math_nodes(
        cx,
        parent,
        vec![integral_command(), cdots_command(), scripted_integral],
    );
    true
}

fn replace_idotsint_followed_by_scripted_limits(
    cx: &mut crate::transform::rule_context::RuleContext<'_>,
    node_id: NodeId,
) -> Result<bool, crate::transform::engine::TransformError> {
    let Some(next) = cx.ast.next_sibling(node_id) else {
        return Ok(false);
    };
    let (limits_base, subscript, superscript) = match cx.ast.node(next) {
        Node::Scripted {
            base,
            subscript,
            superscript,
        } => (*base, *subscript, *superscript),
        _ => return Ok(false),
    };
    let Some(limits_command) = cx.match_command(limits_base, &base::cmd::LIMITS) else {
        return Ok(false);
    };
    if !limits_command.args.is_empty() {
        return Ok(false);
    }

    let (subscript, superscript) = clone_script_attachments(cx, subscript, superscript);
    let limits_with_scripts = Node::Scripted {
        base: cx
            .ast
            .new_node(prefix_command(&base::cmd::LIMITS, Vec::new())),
        subscript,
        superscript,
    };
    let mathop_body = explicit_multi_integral_body(cx);
    let mathop = prefix_command(
        &base::cmd::MATHOP,
        vec![mandatory_content(mathop_body, ContentMode::Math)],
    );
    replace_with_math_nodes(
        cx,
        node_id,
        vec![
            negative_thin_space_command(),
            negative_thin_space_command(),
            mathop,
            limits_with_scripts,
        ],
    );
    // Consume the original following \limits node after cloning its scripts.
    cx.ast.remove_node(next);
    Ok(true)
}

fn explicit_multi_integral_body(cx: &mut crate::transform::rule_context::RuleContext<'_>) -> NodeId {
    let first_space = cx.ast.new_node(thin_space_command());
    let second_space = cx.ast.new_node(thin_space_command());
    let first_int = cx.ast.new_node(integral_command());
    let cdots = cx.ast.new_node(cdots_command());
    let second_int = cx.ast.new_node(integral_command());
    implicit_math_group(
        cx,
        vec![first_space, second_space, first_int, cdots, second_int],
    )
}

fn clone_script_attachments(
    cx: &mut crate::transform::rule_context::RuleContext<'_>,
    subscript: Option<NodeId>,
    superscript: Option<NodeId>,
) -> (Option<NodeId>, Option<NodeId>) {
    (
        subscript.map(|id| cx.ast.clone_subtree(id)),
        superscript.map(|id| cx.ast.clone_subtree(id)),
    )
}

fn replace_with_math_nodes(
    cx: &mut crate::transform::rule_context::RuleContext<'_>,
    node_id: NodeId,
    nodes: Vec<Node>,
) {
    let mut nodes = nodes.into_iter();
    let replacement = nodes
        .next()
        .expect("idotsint expansions always emit at least one node");
    let after = nodes.map(|node| cx.ast.new_node(node)).collect();
    replace_with_math_sequence(cx, node_id, Vec::new(), replacement, after);
}

fn character_command(record: &'static BuiltinCharacterRecord) -> Node {
    // TeX control-sequence character records such as \int remain command
    // nodes in the AST.
    Node::Command {
        name: record.name.to_string(),
        args: Vec::new(),
        known: true,
    }
}

fn negative_thin_space_command() -> Node {
    prefix_command(&base::cmd::_EXCLAMATION, Vec::new())
}

fn thin_space_command() -> Node {
    prefix_command(&base::cmd::_COMMA, Vec::new())
}

fn integral_command() -> Node {
    character_command(&base::chars::INT)
}

fn cdots_command() -> Node {
    character_command(&base::chars::CDOTS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: IDOTSINT_EXPAND,
        class: Expand,
        examples: [
        {
            label: plain_multi_integral_domain,
            packages: ["base", "ams"],
            input: r"\idotsint_{[0,1]^4} f(x_1,\dots,x_4)\,dx_1\cdots dx_4",
            expected: r"\int\cdots\int_{[0,1]^4} f(x_1,\dots,x_4)\,dx_1\cdots dx_4",
        },
        {
            label: explicit_limits_multi_integral,
            packages: ["base", "ams"],
            input: r"\idotsint\limits_{D_n}^{\infty} e^{-\sum_{j=1}^{m} t_j}\,dt_1\cdots dt_m",
            expected: r"\!\!\mathop{\,\,\int\cdots\int}\limits_{D_n}^{\infty} e^{-\sum_{j=1}^{m} t_j}\,dt_1\cdots dt_m",
        },
        ]
    }
    // END: Generated examples

}
