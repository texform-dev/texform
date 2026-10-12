//! Detects where a known command would read a following sibling as an omitted
//! trailing argument after serialization and reparsing.
//!
//! A parsed tree never has such an adjacency: the parser would have attached
//! the token as an argument. Phases that remove a separator between a command
//! and a following token must check it, as in `\sin{}[2]` → `\sin[2]` when the
//! `physics` `\sin` takes `o`.

use crate::ast::{ArgumentKind, ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, Slot};
use crate::knowledge::{ActiveCommandRecord, ArgForm, DelimiterToken};

/// Whether the nearest node of `before`, walked backwards from a boundary, is
/// a known command that would read the first token of `after` as an omitted
/// trailing argument. Whitespace text between them counts as a leading space.
///
/// `lookup` resolves the active record of a command name in `mode`, the mode of
/// the sequence holding both sides.
pub(crate) fn boundary_captures<'a, 'r>(
    ast: &Ast,
    lookup: impl Fn(&str, ContentMode) -> Option<&'r ActiveCommandRecord>,
    mode: ContentMode,
    before: impl Iterator<Item = &'a NodeId>,
    after: impl Iterator<Item = &'a NodeId>,
) -> bool {
    let mut after_space = false;
    let mut before = before.copied().skip_while(|&node| {
        let space = is_whitespace_text(ast, node);
        after_space |= space;
        space
    });
    // A command carrying scripts has already stopped reading arguments.
    let Some((name, args)) = before.next().and_then(|node| match ast.node(node) {
        Node::Command {
            name,
            args,
            known: true,
        }
        | Node::Declarative { name, args } => Some((name, args)),
        _ => None,
    }) else {
        return false;
    };
    let Some(next) = after.copied().find(|&node| {
        let space = is_whitespace_text(ast, node);
        after_space |= space;
        !space
    }) else {
        return false;
    };
    if let Node::Text(text) = ast.node(next) {
        after_space |= text.starts_with(char::is_whitespace);
    }
    let Some(record) = lookup(name, mode) else {
        return false;
    };
    let spec = record.argspec.args;
    if spec.len() != args.len() {
        return false;
    }
    // Arguments are read in order, so only slots after the last supplied one
    // can still take a following token. An absent star is stored as `false`
    // rather than as an empty slot.
    let start = args
        .iter()
        .rposition(|slot| {
            slot.as_ref().is_some_and(|argument| {
                argument.kind != ArgumentKind::Star
                    || argument.value != ArgumentValue::Boolean(false)
            })
        })
        .map_or(0, |last| last + 1);
    for slot in &spec[start..] {
        if slot.required {
            return false;
        }
        if after_space && slot.no_leading_space {
            continue;
        }
        if form_opens_with(ast, &slot.form, next) {
            return true;
        }
    }
    false
}

/// Separates `node` from a preceding command that would read it as an
/// omitted trailing argument. Returns whether it changed the tree.
///
/// Use this after a rewrite places `node` where a separator used to be. An
/// empty group goes before `node`, as in `\\{}[x]`, unless the command would
/// read that group too; then the command itself is wrapped, as in
/// `{\exp}(x)`. FlattenGroups keeps either group.
pub(crate) fn separate_from_preceding_command<'r>(
    ast: &mut Ast,
    lookup: impl Fn(&str, ContentMode) -> Option<&'r ActiveCommandRecord> + Copy,
    node: NodeId,
) -> bool {
    let Some(Slot::GroupChild(index)) = ast.slot(node) else {
        return false;
    };
    let Some(parent) = ast.parent_id(node) else {
        return false;
    };
    let mode = match ast.node(parent) {
        Node::Root { mode, .. } | Node::Group { mode, .. } => *mode,
        _ => return false,
    };
    let siblings = ast.children(parent);
    let (before, after) = siblings.split_at(index);
    if !boundary_captures(ast, lookup, mode, before.iter().rev(), after.iter()) {
        return false;
    }

    let Some(command) = before
        .iter()
        .rev()
        .copied()
        .find(|&sibling| !is_whitespace_text(ast, sibling))
    else {
        return false;
    };
    let Some(Slot::GroupChild(command_index)) = ast.slot(command) else {
        return false;
    };

    let separator = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Explicit,
        mode,
    });
    let siblings = ast.children(parent);
    if !boundary_captures(
        ast,
        lookup,
        mode,
        siblings[..index].iter().rev(),
        std::iter::once(&separator),
    ) {
        ast.insert_child(parent, index, separator);
        return true;
    }
    ast.remove_detached(separator);
    let command = ast.detach_children_range(parent, command_index..command_index + 1);
    let group = ast.new_node(Node::Group {
        children: command,
        kind: GroupKind::Explicit,
        mode,
    });
    ast.insert_child(parent, command_index, group);
    true
}

fn is_whitespace_text(ast: &Ast, node: NodeId) -> bool {
    matches!(ast.node(node), Node::Text(text) if text.trim().is_empty())
}

/// Whether `node` begins with a token that opens an argument of `form`.
fn form_opens_with(ast: &Ast, form: &ArgForm, node: NodeId) -> bool {
    match form {
        ArgForm::Standard => starts_with_char(ast, node, '['),
        ArgForm::Star => starts_with_char(ast, node, '*'),
        ArgForm::Group => starts_with_brace_group(ast, node),
        ArgForm::Delimited { open, .. } => starts_with_token(ast, node, open),
        ArgForm::Paired { pairs } => pairs
            .iter()
            .any(|(open, _)| starts_with_token(ast, node, open)),
        ArgForm::Until { .. } => false,
    }
}

fn starts_with_token(ast: &Ast, node: NodeId, token: &DelimiterToken) -> bool {
    match token {
        // A brace delimiter is read as a brace group, which the AST keeps as
        // a group node rather than as characters.
        DelimiterToken::Char('{') => starts_with_brace_group(ast, node),
        DelimiterToken::Char(c) => starts_with_char(ast, node, *c),
        DelimiterToken::ControlSeq(control) => matches!(
            ast.node(first_token(ast, node)),
            Node::Command { name, .. } if name == control.as_ref()
        ),
    }
}

fn starts_with_brace_group(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(first_token(ast, node)),
        Node::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            ..
        }
    )
}

fn starts_with_char(ast: &Ast, node: NodeId, c: char) -> bool {
    match ast.node(first_token(ast, node)) {
        Node::Char(value) => *value == c,
        Node::Text(text) => text.trim_start().starts_with(c),
        _ => false,
    }
}

/// The node that serializes first: scripts follow their base.
fn first_token(ast: &Ast, node: NodeId) -> NodeId {
    match ast.node(node) {
        Node::Scripted { base, .. } => first_token(ast, *base),
        _ => node,
    }
}
