use texform_knowledge::builtin::{ams, base};
use texform_knowledge::specs::{BuiltinCommandRecord, BuiltinEnvironmentRecord};

use crate::ast::{ArgumentSlot, ArgumentValue, Node, NodeId};
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
    let mut children = Vec::new();

    for row in alignment_rows(cx, body) {
        children.extend(row.children);
        children.extend(row.terminator);
    }

    children
}

/// A row of a plain-TeX alignment body and the `\\` that ends it, if any.
pub(super) struct AlignmentRow {
    pub children: Vec<NodeId>,
    pub terminator: Option<NodeId>,
}

/// Splits a plain-TeX alignment body at known `\cr`, `\\`, and `\newline` row
/// separators. `\cr` and `\newline` become `\\`, while `\\` keeps its `*` and
/// spacing. A final separator only ends the last row, so it is dropped unless
/// it carries `*` or spacing.
pub(super) fn alignment_rows(cx: &mut RuleContext<'_>, body: NodeId) -> Vec<AlignmentRow> {
    let source_children = match cx.ast.node(body) {
        Node::Group { children, .. } => children.clone(),
        _ => vec![body],
    };
    let mut rows = Vec::new();
    let mut children = Vec::new();

    for child in source_children {
        let terminator = match cx.ast.node(child) {
            Node::Command { name, known: true, .. } if name == base::cmd::_BACKSLASH.name => {
                cx.ast.clone_subtree(child)
            }
            Node::Command { name, args, known: true }
                if args.is_empty()
                    && (name == base::cmd::CR.name || name == base::cmd::NEWLINE.name) =>
            {
                cx.ast.new_node(linebreak_command())
            }
            _ => {
                children.push(cx.ast.clone_subtree(child));
                continue;
            }
        };
        rows.push(AlignmentRow {
            children: std::mem::take(&mut children),
            terminator: Some(terminator),
        });
    }

    match rows.last_mut() {
        Some(last) if children.is_empty() => {
            if let Some(terminator) = last.terminator.take_if(|t| is_default_linebreak(cx, *t)) {
                cx.ast.remove_detached(terminator);
            }
        }
        _ => rows.push(AlignmentRow { children, terminator: None }),
    }

    rows
}

fn is_default_linebreak(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    let Node::Command { args, .. } = cx.ast.node(node_id) else {
        return false;
    };
    args.iter().all(|slot| {
        slot.as_ref()
            .is_none_or(|arg| arg.value == ArgumentValue::Boolean(false))
    })
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

pub(super) fn tag_command(tag: NodeId, star: bool) -> Node {
    Node::Command {
        name: ams::cmd::TAG.name.to_string(),
        args: vec![
            star_slot(star),
            crate::rewrite::helpers::mandatory_content_slot(tag, crate::ast::ContentMode::Text),
        ],
        known: true,
    }
}
