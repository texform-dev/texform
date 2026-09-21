//! FlattenGroups removes structurally redundant explicit and implicit groups.

use serde::{Deserialize, Deserializer};

use crate::ast::{ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, ParentLink, Slot};

/// Public per-run switches for FlattenGroups: whether the phase runs, and
/// whether rendered-spacing groups are kept.
///
/// Structural guards always stay on for this public config. Fine-grained
/// per-guard control is not part of the stable API.
/// `preserve_rendered_spacing` does not control serializer source spacing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlattenGroupsConfig {
    /// Run the phase when `true`; skip it entirely when `false`.
    pub enabled: bool,
    /// Keep groups whose only public-facing effect is rendered math spacing.
    ///
    /// This does not control serializer source whitespace (`SerializeOptions`
    /// `*_spacing` fields). Structural guards stay on even when this is `false`.
    pub preserve_rendered_spacing: bool,
}

impl FlattenGroupsConfig {
    /// Phase on with rendered-spacing protection: every internal guard is on.
    pub const STRICT: Self = Self {
        enabled: true,
        preserve_rendered_spacing: true,
    };
    /// Phase on without rendered-spacing protection: only structural guards stay on.
    pub const STRUCTURAL_ONLY: Self = Self {
        enabled: true,
        preserve_rendered_spacing: false,
    };
    /// Phase on with every preserve guard: alias of [`Self::STRICT`].
    pub const ENABLED: Self = Self::STRICT;
    /// Phase off. `preserve_rendered_spacing` is copied from [`Self::STRICT`] but unused while disabled.
    pub const DISABLED: Self = Self {
        enabled: false,
        ..Self::STRICT
    };
    /// Historical default: same as [`Self::STRICT`].
    pub const DEFAULTS: Self = Self::STRICT;
}

/// Complete FlattenGroups protection set for one run.
///
/// This type is an unstable research/internal surface. Field names, layout, and
/// the run-with-guards entry may change without notice. Doc comments describe
/// the actual trigger, not every overlapping case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlattenGroupsGuards {
    /// Keep a group when its subtree contains a declarative command such as
    /// `\cal` or `\bf`, so flattening would leak that scope into following siblings.
    pub declarative_scope: bool,
    /// Keep a group occupying a `ScriptBase` slot when its child is not an
    /// atomic base, so flattening would change which atom a script attaches to.
    pub script_base: bool,
    /// Keep a group inside an environment body, except a lone `Prime` in a
    /// superscript slot, so cell boundaries and intra-cell spacing stay intact.
    pub env_body: bool,
    /// Keep a `GroupChild` whose subtree contains an `\over`-style infix, so
    /// flattening would change the infix scope.
    pub infix_scope: bool,
    /// Keep a `GroupChild` when its preceding sibling or first child is
    /// command-like (`group_child_touches_command` / `CommandContact`).
    pub command_contact: bool,
    /// Refines [`Self::command_contact`]: also treat a `Scripted` node as
    /// command-like when that classification walks through its base. Has no
    /// independent effect when `command_contact` is `false`.
    pub command_like_includes_scripted_base: bool,
    /// Keep a group in an `Argument` slot that has exactly one child whose
    /// subtree contains a command-like node, preserving one spacing boundary.
    pub command_argument: bool,
    /// Keep an empty `GroupChild` (`{}`) for its spacing / kerning effect.
    pub empty_group: bool,
    /// Keep a singleton group whose only child is one math atom-spacing
    /// character (`= < > + - , : ; . / * ! ? | ·`).
    pub lone_atom_spacing_char: bool,
    /// Keep a multi-child `GroupChild` whose first child is a math atom-spacing
    /// character.
    pub leading_atom_spacing_char: bool,
    /// Keep a `GroupChild` whose subtree contains a `\left...\right` delimited pair.
    pub delimited_pair: bool,
}

impl FlattenGroupsGuards {
    /// Expand a public FlattenGroups strategy into the complete guard set.
    ///
    /// Unstable research/internal helper. The four structural guards are always
    /// `true`. The seven spacing-related switches follow
    /// [`FlattenGroupsConfig::preserve_rendered_spacing`].
    pub const fn from_config(config: FlattenGroupsConfig) -> Self {
        Self {
            declarative_scope: true,
            script_base: true,
            env_body: true,
            infix_scope: true,
            command_contact: config.preserve_rendered_spacing,
            command_like_includes_scripted_base: config.preserve_rendered_spacing,
            command_argument: config.preserve_rendered_spacing,
            empty_group: config.preserve_rendered_spacing,
            lone_atom_spacing_char: config.preserve_rendered_spacing,
            leading_atom_spacing_char: config.preserve_rendered_spacing,
            delimited_pair: config.preserve_rendered_spacing,
        }
    }

    /// Apply a sparse research overlay. Omitted fields keep the expanded values.
    pub fn apply_overlay(&mut self, overlay: FlattenGroupsGuardsOverlay) {
        if let Some(value) = overlay.declarative_scope {
            self.declarative_scope = value;
        }
        if let Some(value) = overlay.script_base {
            self.script_base = value;
        }
        if let Some(value) = overlay.env_body {
            self.env_body = value;
        }
        if let Some(value) = overlay.infix_scope {
            self.infix_scope = value;
        }
        if let Some(value) = overlay.command_contact {
            self.command_contact = value;
        }
        if let Some(value) = overlay.command_like_includes_scripted_base {
            self.command_like_includes_scripted_base = value;
        }
        if let Some(value) = overlay.command_argument {
            self.command_argument = value;
        }
        if let Some(value) = overlay.empty_group {
            self.empty_group = value;
        }
        if let Some(value) = overlay.lone_atom_spacing_char {
            self.lone_atom_spacing_char = value;
        }
        if let Some(value) = overlay.leading_atom_spacing_char {
            self.leading_atom_spacing_char = value;
        }
        if let Some(value) = overlay.delimited_pair {
            self.delimited_pair = value;
        }
    }
}

/// Sparse overlay over [`FlattenGroupsGuards`].
///
/// Unstable research/internal input. Present keys must be booleans; JSON `null`
/// and implicit conversions are rejected. Omitted keys keep the expanded value.
/// This type is not a public compatibility surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields, expecting = "an object")]
pub struct FlattenGroupsGuardsOverlay {
    #[serde(deserialize_with = "require_present_bool")]
    pub declarative_scope: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub script_base: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub env_body: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub infix_scope: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub command_contact: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub command_like_includes_scripted_base: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub command_argument: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub empty_group: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub lone_atom_spacing_char: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub leading_atom_spacing_char: Option<bool>,
    #[serde(deserialize_with = "require_present_bool")]
    pub delimited_pair: Option<bool>,
}

fn require_present_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    bool::deserialize(deserializer).map(Some)
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
/// Counter names keep the historical `preserve_*` report contract. They map
/// one-to-one onto [`FlattenGroupsGuards`] fields and still count the first
/// matching situation in evaluation order.
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

pub fn run(ast: &mut Ast, guards: &FlattenGroupsGuards, report: &mut FlattenGroupsReport) {
    visit(ast, ast.root(), false, guards, report);
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
    guards: &FlattenGroupsGuards,
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
                guards,
                report,
            );
            flags.has_declarative |= child_flags.has_declarative;
            flags.has_infix |= child_flags.has_infix;
            flags.has_delimited |= child_flags.has_delimited;
        }
    }

    if ast.contains(node) {
        try_unwrap(ast, node, flags, in_env_body, guards, report);
    }

    flags
}

fn try_unwrap(
    ast: &mut Ast,
    node: NodeId,
    flags: SubtreeFlags,
    in_env_body: bool,
    guards: &FlattenGroupsGuards,
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
    if guards.declarative_scope && flags.has_declarative {
        report.guards.preserve_group_containing_declarative_command += 1;
        return;
    }
    let Some(link) = ast.parent(node) else {
        return;
    };
    if guards.env_body && in_env_body && !is_lone_prime_superscript_group(ast, node, link) {
        report.guards.preserve_group_inside_env_body += 1;
        return;
    }
    if !slot_can_unwrap(link.slot, child_count) {
        return;
    }
    if matches!(link.slot, Slot::GroupChild(_)) && guards.infix_scope && flags.has_infix {
        report.guards.preserve_group_containing_infix += 1;
        return;
    }
    if matches!(link.slot, Slot::GroupChild(_)) && guards.delimited_pair && flags.has_delimited {
        report.guards.preserve_group_containing_delimited_pair += 1;
        return;
    }
    if let Slot::GroupChild(index) = link.slot
        && guards.command_contact
    {
        let command_contact = group_child_touches_command(
            ast,
            node,
            link.parent,
            index,
            guards.command_like_includes_scripted_base,
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
        if guards.empty_group && child_count == 0 {
            report.guards.preserve_empty_group += 1;
            return;
        }
        if guards.lone_atom_spacing_char && child_count == 1 && first_is_atom {
            report.guards.preserve_group_with_lone_atom_spacing_char += 1;
            return;
        }
        if guards.leading_atom_spacing_char && child_count > 1 && first_is_atom {
            report.guards.preserve_group_starting_with_atom_spacing_char += 1;
            return;
        }
    }
    if matches!(link.slot, Slot::ScriptBase)
        && guards.lone_atom_spacing_char
        && child_count == 1
        && first_is_atom
    {
        report.guards.preserve_group_with_lone_atom_spacing_char += 1;
        return;
    }
    if matches!(link.slot, Slot::Argument(_))
        && guards.command_argument
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
        && guards.script_base
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

fn is_lone_prime_superscript_group(ast: &Ast, node: NodeId, link: ParentLink) -> bool {
    // This group is script syntax, not cell structure. Keeping it would emit
    // `^{'}`, even though the active prime character already creates a superscript.
    if !matches!(link.slot, Slot::ScriptSup) {
        return false;
    }
    let [child] = ast.children(node) else {
        return false;
    };
    matches!(ast.node(*child), Node::Prime { .. })
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
