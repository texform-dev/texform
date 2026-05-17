//! FlattenGroups removes structurally redundant explicit and implicit groups.

use crate::ast::{ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, ParentLink, Slot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlattenGroupsConfig {
    pub enabled: bool,
}

impl FlattenGroupsConfig {
    pub const ENABLED: Self = Self { enabled: true };
    pub const DISABLED: Self = Self { enabled: false };
    pub const DEFAULTS: Self = Self::ENABLED;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlattenGroupsReport {
    pub removed_empty: usize,
    pub replaced_single_child: usize,
    pub spliced: usize,
    pub redirected_slot: usize,
}

pub fn run(ast: &mut Ast, config: &FlattenGroupsConfig, report: &mut FlattenGroupsReport) {
    if !config.enabled {
        return;
    }

    visit(ast, ast.root(), false, report);
}

#[derive(Clone, Copy, Debug, Default)]
struct SubtreeFlags {
    has_declarative: bool,
    has_infix: bool,
    has_delimited: bool,
}

fn visit(
    ast: &mut Ast,
    node: NodeId,
    in_env_body: bool,
    report: &mut FlattenGroupsReport,
) -> SubtreeFlags {
    let edges = ast.edges(node);
    let mut flags = SubtreeFlags {
        has_declarative: matches!(ast.node(node), Node::Declarative { .. }),
        has_infix: matches!(ast.node(node), Node::Infix { .. }),
        has_delimited: matches!(
            ast.node(node),
            Node::Group {
                kind: GroupKind::Delimited { .. },
                ..
            }
        ),
    };
    for (child, slot) in edges {
        if ast.contains(child) {
            let child_flags = visit(ast, child, in_env_body || slot == Slot::EnvBody, report);
            flags.has_declarative |= child_flags.has_declarative;
            flags.has_infix |= child_flags.has_infix;
            flags.has_delimited |= child_flags.has_delimited;
        }
    }

    if ast.contains(node) {
        try_unwrap(ast, node, flags, in_env_body, report);
    }

    flags
}

fn try_unwrap(
    ast: &mut Ast,
    node: NodeId,
    flags: SubtreeFlags,
    in_env_body: bool,
    report: &mut FlattenGroupsReport,
) {
    let (kind, mode, child_count) = match ast.node(node) {
        Node::Group {
            kind,
            mode,
            children,
        } => (kind.clone(), *mode, children.len()),
        _ => return,
    };
    if !matches!(kind, GroupKind::Explicit | GroupKind::Implicit) {
        return;
    }
    if flags.has_declarative {
        return;
    }
    if in_env_body {
        return;
    }

    let Some(link) = ast.parent(node) else {
        return;
    };
    if !slot_can_unwrap(link.slot, child_count) {
        return;
    }
    if matches!(link.slot, Slot::GroupChild(_)) && flags.has_infix {
        return;
    }
    if matches!(link.slot, Slot::GroupChild(_)) && flags.has_delimited {
        return;
    }
    if let Slot::GroupChild(index) = link.slot
        && group_child_touches_command(ast, node, link.parent, index)
    {
        return;
    }
    if matches!(link.slot, Slot::GroupChild(_)) && group_child_preserves_atom_spacing(ast, node) {
        return;
    }

    let Some(parent_mode) = context_mode(ast, link) else {
        return;
    };
    if mode != parent_mode {
        return;
    }

    match link.slot {
        Slot::GroupChild(index) => unwrap_group_child(ast, node, link.parent, index, report),
        Slot::Argument(_)
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => redirect_single_child_slot(ast, node, report),
        Slot::ScriptBase | Slot::EnvBody => {}
    }
}

fn slot_can_unwrap(slot: Slot, child_count: usize) -> bool {
    match slot {
        Slot::GroupChild(_) => true,
        Slot::Argument(_)
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => child_count == 1,
        Slot::ScriptBase => false,
        Slot::EnvBody => false,
    }
}

fn group_child_touches_command(ast: &Ast, node: NodeId, parent: NodeId, index: usize) -> bool {
    let previous_is_command = index
        .checked_sub(1)
        .and_then(|previous| ast.children(parent).get(previous).copied())
        .is_some_and(|previous| is_command_like(ast, previous));
    let first_child_is_command = ast
        .children(node)
        .first()
        .copied()
        .is_some_and(|child| is_command_like(ast, child));

    previous_is_command || first_child_is_command
}

fn group_child_preserves_atom_spacing(ast: &Ast, node: NodeId) -> bool {
    let children = ast.children(node);
    if children.is_empty() {
        return true;
    }
    children
        .first()
        .is_some_and(|child| is_atom_spacing_char(ast, *child))
}

fn is_atom_spacing_char(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(node),
        Node::Char(
            '=' | '<' | '>' | '+' | '-' | ',' | ':' | ';' | '.' | '/' | '*' | '!' | '?' | '|'
        )
    )
}

fn is_command_like(ast: &Ast, node: NodeId) -> bool {
    match ast.node(node) {
        Node::Command { .. } | Node::Declarative { .. } => true,
        Node::Scripted { base, .. } => is_command_like(ast, *base),
        _ => false,
    }
}

fn context_mode(ast: &Ast, link: ParentLink) -> Option<ContentMode> {
    match link.slot {
        Slot::GroupChild(_) => match ast.node(link.parent) {
            Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
            _ => None,
        },
        Slot::Argument(index) => argument_slot_mode(ast, link.parent, index),
        Slot::ScriptBase
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => Some(ContentMode::Math),
        Slot::EnvBody => None,
    }
}

fn argument_slot_mode(ast: &Ast, parent: NodeId, index: usize) -> Option<ContentMode> {
    let argument = ast.arg_slots(parent).get(index)?.as_ref()?;
    match argument.value {
        ArgumentValue::MathContent(_) => Some(ContentMode::Math),
        ArgumentValue::TextContent(_) => Some(ContentMode::Text),
        _ => None,
    }
}

fn unwrap_group_child(
    ast: &mut Ast,
    node: NodeId,
    parent: NodeId,
    index: usize,
    report: &mut FlattenGroupsReport,
) {
    let child_count = ast.children(node).len();
    let children = ast.detach_children_range(node, 0..child_count);
    let mut parent_children = ast.children(parent).to_vec();
    assert_eq!(
        parent_children.get(index),
        Some(&node),
        "group child index must match parent link"
    );

    parent_children.splice(index..index + 1, children);
    ast.replace_children(parent, parent_children);
    ast.remove_detached(node);

    match child_count {
        0 => report.removed_empty += 1,
        1 => report.replaced_single_child += 1,
        _ => report.spliced += 1,
    }
}

fn redirect_single_child_slot(ast: &mut Ast, node: NodeId, report: &mut FlattenGroupsReport) {
    let mut children = ast.detach_children_range(node, 0..1);
    let child = children
        .pop()
        .expect("single-child slot unwrap requires one child");
    ast.replace_content_child(node, child);
    ast.remove_detached(node);
    report.redirected_slot += 1;
}
