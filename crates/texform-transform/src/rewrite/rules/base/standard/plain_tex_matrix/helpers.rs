use texform_knowledge::builtin::{ams, base};
use texform_knowledge::specs::{BuiltinCommandRecord, BuiltinEnvironmentRecord};

use crate::ast::{ArgumentSlot, Node, NodeId};
use crate::rewrite::RuleError;
use crate::rewrite::helpers::{linebreak_command_node, star_slot};
use crate::rewrite::rule::{RuleEffect, RuleKey};
use crate::rewrite::rule_context::RuleContext;

pub(super) fn rewrite_cr_body_to_environment(
    rule: RuleKey,
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    source: &'static BuiltinCommandRecord,
    target: &'static BuiltinEnvironmentRecord,
    env_args: Vec<ArgumentSlot>,
) -> Result<RuleEffect, RuleError> {
    let Some(command) = cx.match_command(node_id, source) else {
        return Ok(RuleEffect::Skipped);
    };
    let subject = format!(r"\{}", source.name);
    cx.for_rule(rule)
        .expect_arg_len(command.args, 1, &subject)?;
    let body = mandatory_math_body(rule, cx, &command.args[0], source.name)?;
    let children = cr_body_children(cx, body);

    replace_with_environment(cx, node_id, target, env_args, children);
    Ok(RuleEffect::Applied)
}

pub(super) fn mandatory_math_body(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    command_name: &str,
) -> Result<NodeId, RuleError> {
    cx.for_rule(rule).mandatory_math_content(slot, &format!(r"\{command_name}"), "body")
}

fn cr_body_children(cx: &mut RuleContext<'_>, body: NodeId) -> Vec<NodeId> {
    let rows = cr_rows(cx, body);
    let row_count = rows.len();
    let mut children = Vec::new();

    for (index, row) in rows.into_iter().enumerate() {
        children.extend(row);
        if index + 1 < row_count {
            children.push(cx.ast.new_node(linebreak_command()));
        }
    }

    children
}

pub(super) fn cr_rows(cx: &mut RuleContext<'_>, body: NodeId) -> Vec<Vec<NodeId>> {
    let source_children = match cx.ast.node(body) {
        Node::Group { children, .. } => children.clone(),
        _ => vec![body],
    };
    let mut rows = vec![Vec::new()];

    for child in source_children {
        if is_cr_command(cx, child) {
            rows.push(Vec::new());
        } else {
            let cloned = cx.ast.clone_subtree(child);
            rows.last_mut()
                .expect("rows should always contain the current row")
                .push(cloned);
        }
    }

    rows
}

fn is_cr_command(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    matches!(
        cx.ast.node(node_id),
        Node::Command { name, args, .. } if name == base::cmd::CR.name && args.is_empty()
    )
}

pub(super) fn replace_with_environment(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    target: &'static BuiltinEnvironmentRecord,
    args: Vec<ArgumentSlot>,
    children: Vec<NodeId>,
) {
    let body = cx.ast.implicit_math_group(children);
    cx.ast.replace_node_drop_detached_children(node_id,
        Node::Environment {
            name: target.name.to_string(),
            args,
            known: true,
            body,
        },
    );
}

pub(super) fn linebreak_command() -> Node {
    linebreak_command_node()
}

pub(super) fn tag_command(tag: NodeId) -> Node {
    Node::Command {
        name: ams::cmd::TAG.name.to_string(),
        args: vec![
            star_slot(false),
            crate::rewrite::helpers::mandatory_content_slot(tag, crate::ast::ContentMode::Text),
        ],
        known: true,
    }
}
