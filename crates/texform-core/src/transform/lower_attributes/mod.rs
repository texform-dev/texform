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

use generated::{CommandRef, DeclarativeEntry, ModeTarget, PrefixEntry};

// ---------------------------------------------------------------------------
// Public diagnostic surface
// ---------------------------------------------------------------------------

/// Per-phase statistics. Aggregated into [`crate::transform::TransformReport`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LowerAttributesReport {
    /// How many times each declarative or prefix command was removed from the AST.
    pub consumed: HashMap<&'static str, usize>,
    /// Of the consumed declaratives above, how many were redundant repeats of the active
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
    /// Prefix wrappers whose effect was fully carried by inherited or inner state.
    pub absorbed_prefixes: HashMap<&'static str, usize>,
}

/// One of the attribute axes recognised by the phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attr {
    MathFont,
    MathSize,
    MathStyle,
    TextFamily,
    TextSeries,
    TextShape,
    TextSize,
}

/// Canonical value carried by an attribute slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttrValue {
    MathFont(MathFontValue),
    Size(SizeValue),
    Style(StyleValue),
    TextFamily(TextFamily),
    TextSeries(TextSeries),
    TextShape(TextShape),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MathFontValue(pub &'static str);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextFamily {
    Roman,
    SansSerif,
    Typewriter,
    Calligraphic,
    Italic,
    Oldstyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextSeries {
    Medium,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextShape {
    Upright,
    Italic,
    Slanted,
    SmallCaps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AttributeSet {
    attr: Attr,
    value: AttrValue,
}

// ---------------------------------------------------------------------------
// Per-container attribute snapshot
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AttributeState {
    math_font: Option<AttrValue>,
    math_size: Option<AttrValue>,
    math_style: Option<AttrValue>,
    text_family: Option<AttrValue>,
    text_series: Option<AttrValue>,
    text_shape: Option<AttrValue>,
    text_size: Option<AttrValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pair {
    state: AttributeState,
    node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CollectResult {
    pairs: Vec<Pair>,
    final_state: AttributeState,
}

impl AttributeState {
    fn get(self, attr: Attr) -> Option<AttrValue> {
        match attr {
            Attr::MathFont => self.math_font,
            Attr::MathSize => self.math_size,
            Attr::MathStyle => self.math_style,
            Attr::TextFamily => self.text_family,
            Attr::TextSeries => self.text_series,
            Attr::TextShape => self.text_shape,
            Attr::TextSize => self.text_size,
        }
    }

    #[allow(dead_code)]
    fn with(mut self, set: AttributeSet) -> Self {
        self.set(set);
        self
    }

    /// Sets the selected slot. Returns true when the previous value differed,
    /// which means the call should be recorded as a changepoint by the caller.
    fn set(&mut self, set: AttributeSet) -> bool {
        let slot = self.slot_mut(set.attr);
        if *slot == Some(set.value) {
            return false;
        }
        *slot = Some(set.value);
        true
    }

    fn diff_axes(self, inherited: Self, mode: ContentMode) -> Vec<Attr> {
        Self::axis_order(mode)
            .iter()
            .copied()
            .filter(|attr| self.get(*attr) != inherited.get(*attr))
            .collect()
    }

    fn with_mode_reset(mut self, mode: ContentMode) -> Self {
        for &attr in Self::axis_order(mode) {
            *self.slot_mut(attr) = None;
        }
        self
    }

    fn slot_mut(&mut self, attr: Attr) -> &mut Option<AttrValue> {
        match attr {
            Attr::MathFont => &mut self.math_font,
            Attr::MathSize => &mut self.math_size,
            Attr::MathStyle => &mut self.math_style,
            Attr::TextFamily => &mut self.text_family,
            Attr::TextSeries => &mut self.text_series,
            Attr::TextShape => &mut self.text_shape,
            Attr::TextSize => &mut self.text_size,
        }
    }

    fn axis_order(mode: ContentMode) -> &'static [Attr] {
        match mode {
            ContentMode::Math => &[Attr::MathFont, Attr::MathSize, Attr::MathStyle],
            ContentMode::Text => &[
                Attr::TextShape,
                Attr::TextSeries,
                Attr::TextFamily,
                Attr::TextSize,
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(crate) fn run(ast: &mut Ast, report: &mut LowerAttributesReport) {
    canonicalize_subtree(
        ast,
        ast.root(),
        AttributeState::default(),
        ContentMode::Math,
        report,
    );
}

// ---------------------------------------------------------------------------
// Traversal
// ---------------------------------------------------------------------------

fn canonicalize_subtree(
    ast: &mut Ast,
    node_id: NodeId,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) {
    let container_mode = match ast.node(node_id) {
        Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
        _ => None,
    };

    if let Some(container_mode) = container_mode {
        process_container(ast, node_id, inherited, container_mode, report);
    } else {
        canonicalize_content_slots(ast, node_id, inherited, mode, report);
    }
}

fn canonicalize_content_slots(
    ast: &mut Ast,
    parent: NodeId,
    inherited: AttributeState,
    parent_mode: ContentMode,
    report: &mut LowerAttributesReport,
) {
    let edges = ast.edges(parent);
    for (child, slot) in edges {
        let Some(child_mode) = content_slot_mode(ast, parent, slot) else {
            continue;
        };
        let child_inherited = inherited_for_child_mode(inherited, parent_mode, child_mode);
        let placeholder = empty_implicit_group(ast, child_mode);
        ast.replace_content_child(child, placeholder);

        let collected =
            collect_single_detached_node(ast, child, child_inherited, child_mode, report);
        let rebuilt = segment_and_emit(ast, collected.pairs, child_inherited, child_mode, report);
        let replacement = single_content_replacement(ast, rebuilt, child_mode);
        ast.replace_content_child(placeholder, replacement);
        ast.remove_detached(placeholder);
    }
}

fn content_slot_mode(ast: &Ast, parent: NodeId, slot: Slot) -> Option<ContentMode> {
    match slot {
        Slot::Argument(index) => argument_content_mode(ast, parent, index),
        Slot::EnvBody | Slot::ScriptBase | Slot::ScriptSub | Slot::ScriptSup => {
            Some(ContentMode::Math)
        }
        _ => None,
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

fn inherited_for_child_mode(
    inherited: AttributeState,
    parent_mode: ContentMode,
    child_mode: ContentMode,
) -> AttributeState {
    if child_mode == parent_mode {
        inherited
    } else {
        inherited.with_mode_reset(child_mode)
    }
}

fn single_content_replacement(ast: &mut Ast, mut nodes: Vec<NodeId>, mode: ContentMode) -> NodeId {
    if nodes.len() == 1 {
        return nodes.pop().expect("single content node should exist");
    }

    ast.new_node(Node::Group {
        children: nodes,
        kind: GroupKind::Implicit,
        mode,
    })
}

// ---------------------------------------------------------------------------
// Container rebuild
// ---------------------------------------------------------------------------

fn process_container(
    ast: &mut Ast,
    container: NodeId,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) {
    let len = ast.children(container).len();
    if len == 0 {
        return;
    }

    let detached = ast.detach_children_range(container, 0..len);
    let collected = collect_detached_children(ast, detached, inherited, mode, report);
    record_trailing_empty_segment(
        &collected.pairs,
        collected.final_state,
        inherited,
        mode,
        report,
    );
    let rebuilt = segment_and_emit(ast, collected.pairs, inherited, mode, report);
    let removed = ast.replace_children(container, rebuilt);
    debug_assert!(removed.is_empty());
}

fn collect_detached_children(
    ast: &mut Ast,
    children: Vec<NodeId>,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> CollectResult {
    let mut pairs = Vec::new();
    let mut state = inherited;

    for child in children {
        collect_detached_child(ast, child, &mut state, mode, report, &mut pairs);
    }

    CollectResult {
        pairs,
        final_state: state,
    }
}

fn collect_detached_child(
    ast: &mut Ast,
    child: NodeId,
    state: &mut AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
    pairs: &mut Vec<Pair>,
) {
    if let Some(entry) = lookup_declarative_at(ast, child, mode) {
        consume_declarative(ast, child, state, entry, report);
        return;
    }

    if let Some(entry) = lookup_prefix_at(ast, child, mode)
        && mandatory_content_child(ast, child).is_some()
    {
        let previous = *state;
        let body_pairs = collect_prefix_body(ast, child, previous, entry, mode, report);
        if prefix_is_fully_absorbed(previous, entry.set, &body_pairs) {
            *report.absorbed_prefixes.entry(entry.key).or_default() += 1;
        }
        pairs.extend(body_pairs);
        ast.remove_detached(child);
        return;
    }

    if is_explicit_group(ast, child) {
        pairs.extend(collect_explicit_group(ast, child, *state, mode, report));
        return;
    }

    canonicalize_subtree(ast, child, *state, mode, report);
    pairs.push(Pair {
        state: *state,
        node: child,
    });
}

fn consume_declarative(
    ast: &mut Ast,
    node: NodeId,
    state: &mut AttributeState,
    entry: &'static DeclarativeEntry,
    report: &mut LowerAttributesReport,
) {
    *report.consumed.entry(entry.key).or_default() += 1;
    if !state.set(entry.set) {
        *report.collapsed.entry(entry.key).or_default() += 1;
    }
    ast.remove_detached(node);
}

fn collect_prefix_body(
    ast: &mut Ast,
    prefix: NodeId,
    previous: AttributeState,
    entry: &'static PrefixEntry,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> Vec<Pair> {
    *report.consumed.entry(entry.key).or_default() += 1;
    let body_state = previous.with(entry.set);
    let body = mandatory_content_child(ast, prefix).expect("registered prefix should have a body");

    match ast.node(body) {
        Node::Group {
            kind: GroupKind::Implicit,
            ..
        } => {
            let len = ast.children(body).len();
            let detached = ast.detach_children_range(body, 0..len);
            detach_body_from_prefix(ast, body, mode);
            ast.remove_detached(body);
            let collected = collect_detached_children(ast, detached, body_state, mode, report);
            record_trailing_empty_segment(
                &collected.pairs,
                collected.final_state,
                body_state,
                mode,
                report,
            );
            collected.pairs
        }
        Node::Group {
            kind: GroupKind::Explicit,
            ..
        } => {
            detach_body_from_prefix(ast, body, mode);
            collect_explicit_group(ast, body, body_state, mode, report)
        }
        _ => {
            detach_body_from_prefix(ast, body, mode);
            collect_single_detached_node(ast, body, body_state, mode, report).pairs
        }
    }
}

fn detach_body_from_prefix(ast: &mut Ast, body: NodeId, mode: ContentMode) {
    let placeholder = empty_implicit_group(ast, mode);
    ast.replace_content_child(body, placeholder);
}

fn collect_explicit_group(
    ast: &mut Ast,
    group: NodeId,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> Vec<Pair> {
    if !has_direct_declarative_marker(ast, group, mode) {
        canonicalize_subtree(ast, group, inherited, mode, report);
        return vec![Pair {
            state: inherited,
            node: group,
        }];
    }

    let len = ast.children(group).len();
    let detached = ast.detach_children_range(group, 0..len);
    let inner = collect_detached_children(ast, detached, inherited, mode, report);
    record_trailing_empty_segment(&inner.pairs, inner.final_state, inherited, mode, report);

    if !inner.pairs.is_empty() && inner.pairs.iter().any(|pair| pair.state != inherited) {
        ast.remove_detached(group);
        return inner.pairs;
    }

    let nodes = inner.pairs.into_iter().map(|pair| pair.node).collect();
    let removed = ast.replace_children(group, nodes);
    debug_assert!(removed.is_empty());
    vec![Pair {
        state: inherited,
        node: group,
    }]
}

fn collect_single_detached_node(
    ast: &mut Ast,
    node: NodeId,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> CollectResult {
    let mut pairs = Vec::new();
    let mut state = inherited;
    collect_detached_child(ast, node, &mut state, mode, report, &mut pairs);
    record_trailing_empty_segment(&pairs, state, inherited, mode, report);
    CollectResult {
        pairs,
        final_state: state,
    }
}

fn record_trailing_empty_segment(
    pairs: &[Pair],
    final_state: AttributeState,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) {
    let segment_state = pairs.last().map_or(inherited, |pair| pair.state);
    if !final_state.diff_axes(segment_state, mode).is_empty() {
        report.eliminated_empty_segments += 1;
    }
}

fn has_direct_declarative_marker(ast: &Ast, group: NodeId, mode: ContentMode) -> bool {
    ast.children(group)
        .iter()
        .any(|child| lookup_declarative_at(ast, *child, mode).is_some())
}

fn is_explicit_group(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(node),
        Node::Group {
            kind: GroupKind::Explicit,
            ..
        }
    )
}

fn mandatory_content_child(ast: &Ast, node: NodeId) -> Option<NodeId> {
    let mut found = None;
    for argument in ast.arg_slots(node).iter().flatten() {
        if !matches!(argument.kind, crate::ast::ArgumentKind::Mandatory) {
            continue;
        }
        let child = match argument.value {
            ArgumentValue::MathContent(child) | ArgumentValue::TextContent(child) => child,
            _ => continue,
        };
        if found.replace(child).is_some() {
            return None;
        }
    }
    found
}

fn empty_implicit_group(ast: &mut Ast, mode: ContentMode) -> NodeId {
    ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode,
    })
}

fn prefix_is_fully_absorbed(
    previous: AttributeState,
    set: AttributeSet,
    body_pairs: &[Pair],
) -> bool {
    let previous_value = previous.get(set.attr);
    if previous_value == Some(set.value) {
        return true;
    }

    !body_pairs.is_empty()
        && body_pairs
            .iter()
            .all(|pair| pair.state.get(set.attr) != Some(set.value))
}

fn segment_and_emit(
    ast: &mut Ast,
    pairs: Vec<Pair>,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> Vec<NodeId> {
    let mut rebuilt = Vec::new();
    let mut iter = pairs.into_iter().peekable();

    while let Some(first) = iter.next() {
        let segment_state = first.state;
        let mut segment = vec![first.node];

        while let Some(next) = iter.peek() {
            if next.state != segment_state {
                break;
            }
            segment.push(iter.next().expect("peeked segment pair should exist").node);
        }

        rebuilt.extend(wrap_with_canonical(
            ast,
            segment,
            segment_state,
            inherited,
            mode,
            report,
        ));
    }

    rebuilt
}

/// Wrap a non-empty segment body with the active attributes in mode-specific
/// order: attributes with a `prefix` target wrap the body into an implicit
/// group + prefix command (innermost first), while attributes without a prefix
/// prepend the corresponding declarative.
fn wrap_with_canonical(
    ast: &mut Ast,
    mut children: Vec<NodeId>,
    state: AttributeState,
    inherited: AttributeState,
    mode: ContentMode,
    report: &mut LowerAttributesReport,
) -> Vec<NodeId> {
    debug_assert!(
        !children.is_empty(),
        "segment_and_emit must not call wrap_with_canonical with an empty segment"
    );

    for attr in emit_axis_order(state, inherited, mode) {
        let Some(value) = state.get(attr) else {
            continue;
        };
        let Some(target) = lookup_target(attr, value, mode) else {
            continue;
        };

        if let Some(prefix) = target.prefix {
            let group = ast.new_node(Node::Group {
                children,
                kind: GroupKind::Implicit,
                mode,
            });
            let command = ast.new_node(Node::Command {
                name: prefix.name.to_string(),
                args: vec![mandatory_content_slot(group, mode)],
                known: true,
            });
            *report.wrapped.entry((attr, value, mode)).or_default() += 1;
            children = vec![command];
        } else {
            children.insert(0, new_declarative_node(ast, target.declarative));
            *report.reinserted.entry((attr, value, mode)).or_default() += 1;
        }
    }

    children
}

fn emit_axis_order(
    state: AttributeState,
    inherited: AttributeState,
    mode: ContentMode,
) -> Vec<Attr> {
    let mut axes = state.diff_axes(inherited, mode);
    if matches!(mode, ContentMode::Math) && axes == [Attr::MathFont, Attr::MathSize] {
        axes.swap(0, 1);
    }
    axes
}

// ---------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------

fn lookup_declarative_at(
    ast: &Ast,
    node_id: NodeId,
    mode: ContentMode,
) -> Option<&'static DeclarativeEntry> {
    let Node::Declarative { name, args } = ast.node(node_id) else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    lookup_declarative(mode, name)
}

fn lookup_declarative(mode: ContentMode, name: &str) -> Option<&'static DeclarativeEntry> {
    generated::DECLARATIVES
        .iter()
        .find(|entry| entry.allowed_mode == mode && entry.name == name)
}

fn lookup_prefix_at(ast: &Ast, node_id: NodeId, mode: ContentMode) -> Option<&'static PrefixEntry> {
    let Node::Command { name, .. } = ast.node(node_id) else {
        return None;
    };
    generated::PREFIXES
        .iter()
        .find(|entry| entry.allowed_mode == mode && entry.name == name)
}

fn lookup_target(attr: Attr, value: AttrValue, mode: ContentMode) -> Option<&'static ModeTarget> {
    generated::ATTRIBUTE_TARGETS
        .iter()
        .find(|entry| entry.attr == attr && entry.value == value)
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
    use super::{
        Attr, AttrValue, AttributeSet, AttributeState, MathFontValue, SizeValue, TextFamily,
        TextSeries, TextShape,
    };
    use crate::ast::ContentMode;

    #[test]
    fn attribute_state_set_returns_false_on_repeat() {
        let mut state = AttributeState::default();
        let bold = AttributeSet {
            attr: Attr::MathFont,
            value: AttrValue::MathFont(MathFontValue("VARIANT.BOLD")),
        };

        assert!(state.set(bold));
        assert!(!state.set(bold));
        assert_eq!(state.get(Attr::MathFont), Some(bold.value));
    }

    #[test]
    fn attribute_state_diff_axes_uses_mode_specific_order() {
        let inherited = AttributeState::default();
        let state = AttributeState::default()
            .with(AttributeSet {
                attr: Attr::MathStyle,
                value: AttrValue::Style(super::StyleValue {
                    letter: "D",
                    display: true,
                    level: 0,
                }),
            })
            .with(AttributeSet {
                attr: Attr::MathFont,
                value: AttrValue::MathFont(MathFontValue("VARIANT.BOLD")),
            })
            .with(AttributeSet {
                attr: Attr::TextFamily,
                value: AttrValue::TextFamily(TextFamily::Roman),
            })
            .with(AttributeSet {
                attr: Attr::TextShape,
                value: AttrValue::TextShape(TextShape::Italic),
            })
            .with(AttributeSet {
                attr: Attr::TextSeries,
                value: AttrValue::TextSeries(TextSeries::Bold),
            });

        assert_eq!(
            state.diff_axes(inherited, ContentMode::Math),
            vec![Attr::MathFont, Attr::MathStyle]
        );
        assert_eq!(
            state.diff_axes(inherited, ContentMode::Text),
            vec![Attr::TextShape, Attr::TextSeries, Attr::TextFamily]
        );
    }

    #[test]
    fn attribute_state_mode_reset_preserves_other_mode() {
        let state = AttributeState::default()
            .with(AttributeSet {
                attr: Attr::MathSize,
                value: AttrValue::Size(SizeValue(120)),
            })
            .with(AttributeSet {
                attr: Attr::TextSize,
                value: AttrValue::Size(SizeValue(85)),
            })
            .with(AttributeSet {
                attr: Attr::TextShape,
                value: AttrValue::TextShape(TextShape::Italic),
            });

        let math_reset = state.with_mode_reset(ContentMode::Math);
        assert_eq!(math_reset.get(Attr::MathSize), None);
        assert_eq!(
            math_reset.get(Attr::TextSize),
            Some(AttrValue::Size(SizeValue(85)))
        );
        assert_eq!(
            math_reset.get(Attr::TextShape),
            Some(AttrValue::TextShape(TextShape::Italic))
        );

        let text_reset = state.with_mode_reset(ContentMode::Text);
        assert_eq!(
            text_reset.get(Attr::MathSize),
            Some(AttrValue::Size(SizeValue(120)))
        );
        assert_eq!(text_reset.get(Attr::TextSize), None);
        assert_eq!(text_reset.get(Attr::TextShape), None);
    }
}
