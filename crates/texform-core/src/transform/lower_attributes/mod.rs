//! Lower attribute-scope commands to explicit prefix / declarative form.
//!
//! The phase rewrites every Root / Group / Environment container so that
//! registered declaratives such as `\bf`, `\large`, or `\displaystyle` are
//! either replaced by their prefix-command equivalent (e.g. `\mathbf{...}`)
//! or by a single repositioned declarative at the start of the segment they
//! affect. The set of recognised declaratives, the per-mode prefix targets
//! and the canonical attribute values are loaded from `data.yaml` and
//! generated into `generated.rs` by `build.rs` (see `codegen.rs`).

use std::collections::HashMap;

use crate::ast::{ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, Slot};
use crate::transform::helpers::mandatory_content_slot;

mod generated;

use generated::{CommandRef, DeclarativeEntry, ModeTarget};

// ---------------------------------------------------------------------------
// Public diagnostic surface
// ---------------------------------------------------------------------------

/// Per-phase statistics. Aggregated into [`crate::transform::TransformReport`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LowerAttributesReport {
    /// How many times each declarative command was removed from the AST.
    pub dropped: HashMap<&'static str, usize>,
    /// Of the drops above, how many were redundant repeats of the active
    /// value and therefore did not produce a changepoint.
    pub collapsed: HashMap<&'static str, usize>,
    /// Segments wrapped into a prefix command, keyed by attribute / value / mode.
    pub wrapped: HashMap<(Attr, AttrValue, ContentMode), usize>,
    /// Declaratives prepended to a segment because the attribute has no prefix
    /// equivalent (e.g. `\large`, `\displaystyle`).
    pub reinserted: HashMap<(Attr, AttrValue, ContentMode), usize>,
    /// Trailing changepoints whose segment ended up empty (e.g. `{x \bf}` or
    /// `\sqrt{\bf}`).
    pub eliminated_empty_segments: usize,
}

/// One of the three attribute axes recognised by the phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attr {
    Font,
    Size,
    Style,
}

/// Canonical value carried by an attribute slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttrValue {
    Font(&'static str),
    Size(SizeValue),
    Style(StyleValue),
}

impl AttrValue {
    const fn attr(self) -> Attr {
        match self {
            Self::Font(_) => Attr::Font,
            Self::Size(_) => Attr::Size,
            Self::Style(_) => Attr::Style,
        }
    }
}

/// Stable scaled-integer representation of a size factor (value × 100 rounded).
/// Avoids using a bare `f64` as a `Eq + Hash` key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SizeValue(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StyleValue {
    pub letter: &'static str,
    pub display: bool,
    pub level: u8,
}

// ---------------------------------------------------------------------------
// Per-container attribute snapshot
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AttributeState {
    font: Option<AttrValue>,
    size: Option<AttrValue>,
    style: Option<AttrValue>,
}

impl AttributeState {
    fn get(&self, attr: Attr) -> Option<AttrValue> {
        match attr {
            Attr::Font => self.font,
            Attr::Size => self.size,
            Attr::Style => self.style,
        }
    }

    /// Sets the slot indicated by `value.attr()`. Returns true when the
    /// previous value differed, which means the call should be recorded as
    /// a changepoint by the caller.
    fn set(&mut self, value: AttrValue) -> bool {
        let slot = match value.attr() {
            Attr::Font => &mut self.font,
            Attr::Size => &mut self.size,
            Attr::Style => &mut self.style,
        };
        if *slot == Some(value) {
            return false;
        }
        *slot = Some(value);
        true
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(crate) fn run(ast: &mut Ast, report: &mut LowerAttributesReport) {
    visit(ast, ast.root(), report);
}

// ---------------------------------------------------------------------------
// Traversal
// ---------------------------------------------------------------------------

fn visit(ast: &mut Ast, node_id: NodeId, report: &mut LowerAttributesReport) {
    if !ast.contains(node_id) {
        return;
    }

    let edges = ast.edges(node_id);
    for (child, _) in edges {
        visit(ast, child, report);
    }

    if !ast.contains(node_id) {
        return;
    }

    let container_mode = match ast.node(node_id) {
        Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
        _ => None,
    };
    if let Some(mode) = container_mode {
        process_container(ast, node_id, mode, report);
        return;
    }

    process_single_content_children(ast, node_id, report);
}

/// Lower registered declaratives that appear *directly* under a non-group
/// content slot (mandatory arg, env body, or script base/sub/sup). Such a
/// slot would normally carry a Group, but the parser folds single-element
/// content slots into the element itself. The declarative is dropped and the
/// slot becomes an empty implicit group.
fn process_single_content_children(
    ast: &mut Ast,
    parent: NodeId,
    report: &mut LowerAttributesReport,
) {
    let edges = ast.edges(parent);
    for (child, slot) in edges {
        let mode = match slot {
            Slot::Argument(index) => argument_content_mode(ast, parent, index),
            Slot::EnvBody | Slot::ScriptBase | Slot::ScriptSub | Slot::ScriptSup => {
                Some(ContentMode::Math)
            }
            _ => None,
        };
        let Some(mode) = mode else {
            continue;
        };
        if matches!(ast.node(child), Node::Group { .. }) {
            continue;
        }
        let Some((_, name)) = lookup_declarative_at(ast, child, mode) else {
            continue;
        };

        let empty = ast.new_node(Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode,
        });
        ast.replace_content_child(child, empty);
        ast.remove_detached(child);
        *report.dropped.entry(name).or_default() += 1;
        report.eliminated_empty_segments += 1;
    }
}

fn argument_content_mode(ast: &Ast, parent: NodeId, index: usize) -> Option<ContentMode> {
    ast.arg_slots(parent)
        .get(index)
        .and_then(Option::as_ref)
        .and_then(|argument| match argument.value {
            ArgumentValue::MathContent(_) => Some(ContentMode::Math),
            ArgumentValue::TextContent(_) => Some(ContentMode::Text),
            _ => None,
        })
}

// ---------------------------------------------------------------------------
// Container rebuild
// ---------------------------------------------------------------------------

/// Rewrite all registered declaratives within one Root / Group container.
///
/// 1. Classify the children: `retained` (everything else) and `dropped`
///    (recognised declaratives). Record changepoints as `(index_in_retained,
///    new_value)` pairs.
/// 2. Detach every child of the container in one shot, then free the dropped
///    declarative subtrees.
/// 3. Walk `retained` segment by segment: at each changepoint absorb the new
///    value into the active state, then `apply_state` either wraps the segment
///    body with a prefix command or prepends a declarative.
fn process_container(
    ast: &mut Ast,
    parent: NodeId,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) {
    let original = ast.children(parent).to_vec();
    if original.is_empty() {
        return;
    }

    let mut tracker = AttributeState::default();
    let mut retained: Vec<NodeId> = Vec::new();
    let mut dropped: Vec<NodeId> = Vec::new();
    let mut changepoints: Vec<(usize, AttrValue)> = Vec::new();

    for child in &original {
        match lookup_declarative_at(ast, *child, mode) {
            Some((value, name)) => {
                *report.dropped.entry(name).or_default() += 1;
                if tracker.set(value) {
                    changepoints.push((retained.len(), value));
                } else {
                    *report.collapsed.entry(name).or_default() += 1;
                }
                dropped.push(*child);
            }
            None => retained.push(*child),
        }
    }

    if dropped.is_empty() {
        return;
    }

    let detached = ast.detach_children_range(parent, 0..original.len());
    debug_assert_eq!(detached, original);
    for node in dropped {
        ast.remove_detached(node);
    }

    let rebuilt = rebuild_segments(ast, &retained, &changepoints, mode, report);

    let removed = ast.replace_children(parent, rebuilt);
    debug_assert!(removed.is_empty());
}

fn rebuild_segments(
    ast: &mut Ast,
    retained: &[NodeId],
    changepoints: &[(usize, AttrValue)],
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> Vec<NodeId> {
    let total = retained.len();
    let mut rebuilt: Vec<NodeId> = Vec::new();
    let mut active = AttributeState::default();
    let mut cursor = 0;
    let mut cp_idx = 0;

    while cursor < total {
        while cp_idx < changepoints.len() && changepoints[cp_idx].0 == cursor {
            active.set(changepoints[cp_idx].1);
            cp_idx += 1;
        }
        let next = changepoints
            .get(cp_idx)
            .map_or(total, |(position, _)| *position);
        let segment = retained[cursor..next].to_vec();
        rebuilt.extend(apply_state(ast, segment, active, mode, report));
        cursor = next;
    }

    // Any leftover changepoint sits at `total`; they collectively form a
    // single trailing empty segment that contributes one elimination
    // regardless of how many same-position changepoints overlap there.
    if cp_idx < changepoints.len() {
        debug_assert!(changepoints[cp_idx..].iter().all(|(pos, _)| *pos == total));
        report.eliminated_empty_segments += 1;
    }

    rebuilt
}

/// Wrap a non-empty segment body with the active attributes in fixed
/// `[font, size, style]` order: attributes with a `prefix` target wrap the
/// body into an implicit group + prefix command (innermost first), while
/// attributes without a prefix prepend the corresponding declarative.
fn apply_state(
    ast: &mut Ast,
    mut children: Vec<NodeId>,
    state: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> Vec<NodeId> {
    debug_assert!(
        !children.is_empty(),
        "process_container must not call apply_state with an empty segment"
    );

    for attr in [Attr::Font, Attr::Size, Attr::Style] {
        let Some(value) = state.get(attr) else {
            continue;
        };
        let Some(target) = lookup_target(value, mode) else {
            continue;
        };

        if let Some(prefix) = target.prefix {
            let group = ast.new_node(Node::Group {
                children,
                kind: GroupKind::Implicit,
                mode,
            });
            // safe: build.rs guarantees every prefix in data.yaml is a real
            // builtin command, so `known: true` always holds.
            let command = ast.new_node(Node::Command {
                name: prefix.name.to_string(),
                args: vec![mandatory_content_slot(group, mode)],
                known: true,
            });
            *report
                .wrapped
                .entry((value.attr(), value, mode))
                .or_default() += 1;
            children = vec![command];
        } else {
            children.insert(0, new_declarative_node(ast, target.declarative));
            *report
                .reinserted
                .entry((value.attr(), value, mode))
                .or_default() += 1;
        }
    }

    children
}

// ---------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------

fn lookup_declarative_at(
    ast: &Ast,
    node_id: NodeId,
    mode: ContentMode,
) -> Option<(AttrValue, &'static str)> {
    let Node::Declarative { name, args } = ast.node(node_id) else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    lookup_declarative(mode, name).map(|entry| (entry.set, entry.name))
}

fn lookup_declarative(mode: ContentMode, name: &str) -> Option<&'static DeclarativeEntry> {
    generated::DECLARATIVES
        .iter()
        .find(|entry| entry.allowed_mode == mode && entry.name == name)
}

fn lookup_target(value: AttrValue, mode: ContentMode) -> Option<&'static ModeTarget> {
    let targets = match value {
        AttrValue::Font(_) => generated::ATTRIBUTE_TARGETS_FONT,
        AttrValue::Size(_) => generated::ATTRIBUTE_TARGETS_SIZE,
        AttrValue::Style(_) => generated::ATTRIBUTE_TARGETS_STYLE,
    };
    targets
        .iter()
        .find(|entry| entry.value == value)
        .and_then(|entry| match mode {
            ContentMode::Math => entry.math.as_ref(),
            ContentMode::Text => entry.text.as_ref(),
        })
}

fn new_declarative_node(ast: &mut Ast, command: CommandRef) -> NodeId {
    ast.new_node(Node::Declarative {
        name: command.name.to_string(),
        args: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Attr, AttrValue, AttributeState};

    #[test]
    fn attribute_state_set_returns_false_on_repeat() {
        let mut state = AttributeState::default();
        let bold = AttrValue::Font("VARIANT.BOLD");

        assert!(state.set(bold));
        assert!(!state.set(bold));
        assert_eq!(state.get(Attr::Font), Some(bold));
    }
}
