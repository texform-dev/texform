//! FlattenGroups removes structurally redundant explicit and implicit groups.

use crate::ast::{ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, ParentLink, Slot};

/// Per-run switches for the FlattenGroups phase: the master gate plus one
/// preserve guard per situation where flattening would be unsafe. Use the
/// `STRICT` / `STRUCTURAL_ONLY` presets rather than setting fields by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlattenGroupsConfig {
    /// Run the phase when `true`; skip it entirely when `false`.
    pub enabled: bool,
    /// Semantic guard. Keep groups whose subtree contains a declarative command
    /// (for example `\cal` or `\bf`) to avoid leaking declarative scope into
    /// following siblings.
    pub preserve_group_containing_declarative_command: bool,
    /// Semantic guard. Keep groups occupying a `ScriptBase` slot to avoid
    /// changing which atom subscripts or superscripts attach to.
    pub preserve_group_in_script_base_slot: bool,
    /// Semantic guard. Keep all groups inside an environment body to preserve
    /// cell boundaries and intra-cell spacing.
    pub preserve_group_inside_env_body: bool,
    /// Semantic guard. Keep a `GroupChild` whose subtree contains an
    /// `\over`-style infix to preserve the infix scope.
    pub preserve_group_containing_infix: bool,
    /// Spacing guard. Keep a `GroupChild` when its preceding sibling or its
    /// first child is command-like.
    pub preserve_group_adjacent_to_command_like: bool,
    /// Spacing guard. Keep a risky group directly used as an argument of a
    /// command to preserve one spacing boundary without preserving redundant
    /// nesting.
    pub preserve_group_as_argument_of_command: bool,
    /// Spacing guard (sub-flag). Recurse through `Scripted` bases when
    /// classifying "command-like" for the adjacency check above.
    pub preserve_group_after_scripted_command_like: bool,
    /// Spacing guard. Keep empty `GroupChild`s (`{}`) to preserve spacing and
    /// kerning effects.
    pub preserve_empty_group: bool,
    /// Spacing guard. Keep singleton groups containing only one math
    /// atom-spacing character.
    pub preserve_group_with_lone_atom_spacing_char: bool,
    /// Spacing guard. Keep multi-child `GroupChild`s whose first child is a
    /// math atom-spacing character.
    pub preserve_group_starting_with_atom_spacing_char: bool,
    /// Spacing guard. Keep a `GroupChild` whose subtree contains a
    /// `\left...\right` delimited group.
    pub preserve_group_containing_delimited_pair: bool,
}

impl FlattenGroupsConfig {
    /// All preserve guards on.
    pub const STRICT: Self = Self {
        enabled: true,
        preserve_group_containing_declarative_command: true,
        preserve_group_in_script_base_slot: true,
        preserve_group_inside_env_body: true,
        preserve_group_containing_infix: true,
        preserve_group_adjacent_to_command_like: true,
        preserve_group_as_argument_of_command: true,
        preserve_group_after_scripted_command_like: true,
        preserve_empty_group: true,
        preserve_group_with_lone_atom_spacing_char: true,
        preserve_group_starting_with_atom_spacing_char: true,
        preserve_group_containing_delimited_pair: true,
    };
    /// Only Semantic guards on. All Spacing guards off.
    pub const STRUCTURAL_ONLY: Self = Self {
        enabled: true,
        preserve_group_containing_declarative_command: true,
        preserve_group_in_script_base_slot: true,
        preserve_group_inside_env_body: true,
        preserve_group_containing_infix: true,
        preserve_group_adjacent_to_command_like: false,
        preserve_group_as_argument_of_command: false,
        preserve_group_after_scripted_command_like: false,
        preserve_empty_group: false,
        preserve_group_with_lone_atom_spacing_char: false,
        preserve_group_starting_with_atom_spacing_char: false,
        preserve_group_containing_delimited_pair: false,
    };
    pub const ENABLED: Self = Self::STRICT;
    pub const DISABLED: Self = Self {
        enabled: false,
        ..Self::STRICT
    };
    pub const DEFAULTS: Self = Self::STRICT;
}

/// What the FlattenGroups phase did: how many groups it flattened, and how
/// often each preserve guard blocked a flattening.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlattenGroupsReport {
    /// Counts of the flattenings actually performed, by action kind.
    pub actions: FlattenGroupsActionCounts,
    /// Per-guard counts of flattenings that were prevented. Counters are
    /// short-circuit: when several guards match the same group, only the first
    /// one in evaluation order is incremented.
    pub guards: FlattenGroupsGuardCounts,
}

/// How many groups FlattenGroups removed, split by the action taken.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlattenGroupsActionCounts {
    /// Empty `GroupChild` (`{}`) dropped.
    pub removed_empty: usize,
    /// Single-child `GroupChild` replaced by its child.
    pub replaced_single_child: usize,
    /// Multi-child `GroupChild` spliced into its parent's child sequence.
    pub inlined_multi_child: usize,
    /// Single-child group in an `Argument` / `Script*` / `Infix*` slot
    /// unwrapped in place.
    pub unwrapped_slot: usize,
}

/// How often each preserve guard prevented a group from being flattened.
///
/// Each field corresponds to the same-named flag on [`FlattenGroupsConfig`]
/// and counts the structural situation that flag protects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlattenGroupsGuardCounts {
    /// Group kept because its subtree holds a declarative command, so
    /// flattening would leak declarative scope into following siblings.
    pub preserve_group_containing_declarative_command: usize,
    /// Group kept because it occupies a `ScriptBase` slot, so flattening would
    /// change which atom a sub/superscript attaches to.
    pub preserve_group_in_script_base_slot: usize,
    /// Group kept because it sits inside an environment body, where flattening
    /// would blur cell boundaries or intra-cell spacing.
    pub preserve_group_inside_env_body: usize,
    /// Group kept because its subtree holds an `\over`-style infix, so
    /// flattening would change the infix scope.
    pub preserve_group_containing_infix: usize,
    /// Group kept because its preceding sibling or first child is command-like,
    /// where flattening would change atom spacing.
    pub preserve_group_adjacent_to_command_like: usize,
    /// Group kept because it is a risky singleton used directly as a command
    /// argument, preserving one spacing boundary.
    pub preserve_group_as_argument_of_command: usize,
    /// Adjacency check above matched only after recursing through a `Scripted`
    /// base; counted in addition to `preserve_group_adjacent_to_command_like`.
    pub preserve_group_after_scripted_command_like: usize,
    /// Empty group kept for its spacing / kerning effect.
    pub preserve_empty_group: usize,
    /// Singleton group kept because it holds a single math atom-spacing
    /// character.
    pub preserve_group_with_lone_atom_spacing_char: usize,
    /// Multi-child group kept because its first child is a math atom-spacing
    /// character.
    pub preserve_group_starting_with_atom_spacing_char: usize,
    /// Group kept because its subtree holds a `\left...\right` delimited pair.
    pub preserve_group_containing_delimited_pair: usize,
}

pub fn run(ast: &mut Ast, config: &FlattenGroupsConfig, report: &mut FlattenGroupsReport) {
    if !config.enabled {
        return;
    }

    visit(ast, ast.root(), false, config, report);
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
    config: &FlattenGroupsConfig,
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
            let child_flags = visit(
                ast,
                child,
                in_env_body || slot == Slot::EnvBody,
                config,
                report,
            );
            flags.has_declarative |= child_flags.has_declarative;
            flags.has_infix |= child_flags.has_infix;
            flags.has_delimited |= child_flags.has_delimited;
        }
    }

    if ast.contains(node) {
        try_unwrap(ast, node, flags, in_env_body, config, report);
    }

    flags
}

fn try_unwrap(
    ast: &mut Ast,
    node: NodeId,
    flags: SubtreeFlags,
    in_env_body: bool,
    config: &FlattenGroupsConfig,
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
    if config.preserve_group_containing_declarative_command && flags.has_declarative {
        report.guards.preserve_group_containing_declarative_command += 1;
        return;
    }
    if config.preserve_group_inside_env_body && in_env_body {
        report.guards.preserve_group_inside_env_body += 1;
        return;
    }

    let Some(link) = ast.parent(node) else {
        return;
    };
    if !slot_can_unwrap(link.slot, child_count) {
        return;
    }
    if matches!(link.slot, Slot::GroupChild(_))
        && config.preserve_group_containing_infix
        && flags.has_infix
    {
        report.guards.preserve_group_containing_infix += 1;
        return;
    }
    if matches!(link.slot, Slot::GroupChild(_))
        && config.preserve_group_containing_delimited_pair
        && flags.has_delimited
    {
        report.guards.preserve_group_containing_delimited_pair += 1;
        return;
    }
    if let Slot::GroupChild(index) = link.slot
        && config.preserve_group_adjacent_to_command_like
    {
        let command_contact = group_child_touches_command(
            ast,
            node,
            link.parent,
            index,
            config.preserve_group_after_scripted_command_like,
        );
        if command_contact.touches_command {
            report.guards.preserve_group_adjacent_to_command_like += 1;
            if command_contact.used_scripted_base {
                report.guards.preserve_group_after_scripted_command_like += 1;
            }
            return;
        }
    }
    let children = ast.children(node);
    let first_is_atom = children
        .first()
        .is_some_and(|child| is_atom_spacing_char(ast, *child));
    if matches!(link.slot, Slot::GroupChild(_)) {
        if config.preserve_empty_group && child_count == 0 {
            report.guards.preserve_empty_group += 1;
            return;
        }
        if config.preserve_group_with_lone_atom_spacing_char && child_count == 1 && first_is_atom {
            report.guards.preserve_group_with_lone_atom_spacing_char += 1;
            return;
        }
        if config.preserve_group_starting_with_atom_spacing_char && child_count > 1 && first_is_atom
        {
            report.guards.preserve_group_starting_with_atom_spacing_char += 1;
            return;
        }
    }
    if matches!(link.slot, Slot::ScriptBase)
        && config.preserve_group_with_lone_atom_spacing_char
        && child_count == 1
        && first_is_atom
    {
        report.guards.preserve_group_with_lone_atom_spacing_char += 1;
        return;
    }
    if matches!(link.slot, Slot::Argument(_))
        && config.preserve_group_as_argument_of_command
        && group_as_argument_of_command_needs_boundary(ast, node)
    {
        report.guards.preserve_group_as_argument_of_command += 1;
        return;
    }

    let Some(parent_mode) = context_mode(ast, link) else {
        return;
    };
    if mode != parent_mode {
        return;
    }

    if matches!(link.slot, Slot::ScriptBase)
        && config.preserve_group_in_script_base_slot
        && !is_atomic_base(ast, ast.children(node)[0])
    {
        report.guards.preserve_group_in_script_base_slot += 1;
        return;
    }

    match link.slot {
        Slot::GroupChild(index) => unwrap_group_child(ast, node, link.parent, index, report),
        Slot::Argument(_)
        | Slot::ScriptBase
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => redirect_single_child_slot(ast, node, report),
        Slot::EnvBody => {}
    }
}

fn slot_can_unwrap(slot: Slot, child_count: usize) -> bool {
    match slot {
        Slot::GroupChild(_) => true,
        Slot::Argument(_)
        | Slot::ScriptBase
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => child_count == 1,
        Slot::EnvBody => false,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CommandContact {
    touches_command: bool,
    used_scripted_base: bool,
}

fn group_child_touches_command(
    ast: &Ast,
    node: NodeId,
    parent: NodeId,
    index: usize,
    include_scripted: bool,
) -> CommandContact {
    let previous = index
        .checked_sub(1)
        .and_then(|previous| ast.children(parent).get(previous).copied());
    let first_child = ast.children(node).first().copied();

    command_contact_for_node(ast, previous, include_scripted).merge(command_contact_for_node(
        ast,
        first_child,
        include_scripted,
    ))
}

impl CommandContact {
    fn merge(self, other: Self) -> Self {
        Self {
            touches_command: self.touches_command || other.touches_command,
            used_scripted_base: self.used_scripted_base || other.used_scripted_base,
        }
    }
}

fn command_contact_for_node(
    ast: &Ast,
    node: Option<NodeId>,
    include_scripted: bool,
) -> CommandContact {
    let Some(node) = node else {
        return CommandContact::default();
    };
    if is_command_like(ast, node, false) {
        return CommandContact {
            touches_command: true,
            used_scripted_base: false,
        };
    }
    if include_scripted && is_command_like(ast, node, true) {
        return CommandContact {
            touches_command: true,
            used_scripted_base: true,
        };
    }
    CommandContact::default()
}

fn is_atom_spacing_char(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(node),
        Node::Char(
            '=' | '<' | '>' | '+' | '-' | ',' | ':' | ';' | '.' | '/' | '*' | '!' | '?' | '|' | '·'
        )
    )
}

fn is_command_like(ast: &Ast, node: NodeId, include_scripted: bool) -> bool {
    match ast.node(node) {
        Node::Command { .. } | Node::Declarative { .. } => true,
        Node::Scripted { base, .. } if include_scripted => is_command_like(ast, *base, true),
        _ => false,
    }
}

fn is_atomic_base(ast: &Ast, node: NodeId) -> bool {
    match ast.node(node) {
        Node::Char(_) | Node::Prime { .. } => true,
        Node::Command { name, args, .. } => {
            args.iter().all(Option::is_none)
                && !subtree_has_scripted(ast, node)
                && !is_script_placement_sensitive_command(name)
        }
        _ => false,
    }
}

fn group_as_argument_of_command_needs_boundary(ast: &Ast, node: NodeId) -> bool {
    let children = ast.children(node);
    if children.len() != 1 {
        return false;
    }
    subtree_has_command_like(ast, children[0])
}

fn subtree_has_command_like(ast: &Ast, node: NodeId) -> bool {
    if is_command_like(ast, node, false) {
        return true;
    }
    ast.edges(node)
        .into_iter()
        .any(|(child, _)| subtree_has_command_like(ast, child))
}

fn is_script_placement_sensitive_command(name: &str) -> bool {
    matches!(
        name,
        "arccos"
            | "arcsin"
            | "arctan"
            | "arg"
            | "bigcap"
            | "bigcup"
            | "bigodot"
            | "bigoplus"
            | "bigotimes"
            | "bigsqcup"
            | "bigtriangledown"
            | "bigtriangleup"
            | "biguplus"
            | "bigvee"
            | "bigwedge"
            | "cos"
            | "cosh"
            | "cot"
            | "coth"
            | "csc"
            | "deg"
            | "det"
            | "dim"
            | "exp"
            | "gcd"
            | "hom"
            | "inf"
            | "int"
            | "ker"
            | "lg"
            | "lim"
            | "liminf"
            | "limsup"
            | "ln"
            | "log"
            | "max"
            | "min"
            | "operatorname"
            | "Pr"
            | "prod"
            | "sec"
            | "sin"
            | "sinh"
            | "sup"
            | "sum"
            | "tan"
            | "tanh"
    )
}

fn subtree_has_scripted(ast: &Ast, node: NodeId) -> bool {
    if matches!(ast.node(node), Node::Scripted { .. }) {
        return true;
    }
    ast.edges(node)
        .into_iter()
        .any(|(child, _)| subtree_has_scripted(ast, child))
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
        ArgumentValue::MathContent(_) | ArgumentValue::OperatorNameContent(_) => {
            Some(ContentMode::Math)
        }
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
        0 => report.actions.removed_empty += 1,
        1 => report.actions.replaced_single_child += 1,
        _ => report.actions.inlined_multi_child += 1,
    }
}

fn redirect_single_child_slot(ast: &mut Ast, node: NodeId, report: &mut FlattenGroupsReport) {
    let mut children = ast.detach_children_range(node, 0..1);
    let child = children
        .pop()
        .expect("single-child slot unwrap requires one child");
    ast.replace_content_child(node, child);
    ast.remove_detached(node);
    report.actions.unwrapped_slot += 1;
}
