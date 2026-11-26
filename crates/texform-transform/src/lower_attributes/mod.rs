//! Lower attribute-scope commands to explicit prefix / declarative form.
//!
//! Registered declaratives and prefix wrappers come from `data.yaml`. An
//! attribute whose canonical target has a prefix is carried by that wrapper
//! (`{\bf x}` becomes `\mathbf{x}`). An attribute that still has only a
//! declarative command stays inside a real local group, so it cannot affect
//! siblings outside the original group or prefix argument. Following siblings
//! resume the attribute state from before that scope. The phase does not
//! invent `\textstyle`, `\displaystyle`, or `\normalsize` to close a scope.
//!
//! Nested math arguments, subscripts, superscripts, infix operands, and
//! environment bodies can change style implicitly. This phase does not derive
//! that style, so a recorded parent style is not proof that an explicit style
//! declaration inside those slots is redundant. A script base stays in the
//! surrounding style; subscripts and superscripts inherit `math_font` but reset
//! `math_style` and `math_size`. Environment bodies reset all attributes for
//! their mode. Command arguments are fail-closed: only commands verified to
//! inherit attributes identically in both MathJax 4 and XeTeX are whitelisted
//! to propagate `math_font` (math mode) or text attributes (text mode); all
//! other commands reset the child mode's attributes so inner wrappers are never
//! dropped as redundant. FlattenGroups' declarative-scope guard keeps the
//! groups retained here; a later pass normalizes inside them and does not add
//! another wrapper.
//!
//! In a math argument or environment body, `&`, `\\`, `\newline`, and `\cr`
//! end a declarative scope, because TeX typesets each alignment cell as its own
//! group: `\rm a & b` affects only `a`. Separators inside a nested brace group
//! and at the top level keep the running state.

use std::collections::HashMap;
use std::sync::OnceLock;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::argument_capture::{environment_captures_in, sequence_boundary_hazard};
use crate::ast::{Argument, ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, Slot};
use crate::parse::ParseContext;
use crate::report::ReportRecorder;
use crate::rewrite::helpers::mandatory_content_slot;
use texform_knowledge::builtin::base;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/lower_attributes_generated.rs"));
}

use generated::{CommandRef, DeclarativeEntry, ModeTarget, PrefixEntry};

// ---------------------------------------------------------------------------
// Public diagnostic surface
// ---------------------------------------------------------------------------

/// Statistics accumulated across every LowerAttributes invocation in one
/// transform run.
///
/// Counters aggregate all invocations across the scheduling rounds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LowerAttributesReport {
    /// Per-attribute counters for consumed inputs and emitted canonical forms.
    pub attributes: HashMap<AttributeSet, AttributeStat>,
    /// Trailing changepoints whose segment ended up empty (e.g. `{x \bf}` or
    /// `\sqrt{\bf}`).
    pub eliminated_empty_segments: usize,
}

/// Per-attribute breakdown of forms consumed, found redundant, and emitted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttributeStat {
    /// Declarative and prefix forms removed from the AST.
    pub consumed: AttributeFormCounts,
    /// Consumed forms whose attribute effect did not produce emitted output.
    ///
    /// For prefixes this includes both already-active outer prefixes and
    /// prefixes whose body overrides or otherwise carries the effect.
    pub redundant: AttributeFormCounts,
    /// Canonical declarative and prefix forms emitted for the attribute.
    pub emitted: AttributeFormCounts,
}

/// Declarative/prefix split for a single attribute statistic bucket.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttributeFormCounts {
    /// Count for declarative-scope forms (e.g. `\bf`).
    pub declaratives: usize,
    /// Count for prefix-wrapper forms (e.g. `\mathbf{...}`).
    pub prefixes: usize,
}

/// One of the attribute axes recognised by the phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attr {
    /// Math-mode font family (e.g. `\mathbf`, `\mathbb`).
    MathFont,
    /// Math-mode size (e.g. `\large`, `\scriptsize`).
    MathSize,
    /// Math-mode style (e.g. `\displaystyle`, `\scriptstyle`).
    MathStyle,
    /// Text-mode font family (roman, sans-serif, typewriter, ...).
    TextFamily,
    /// Text-mode weight (medium or bold).
    TextSeries,
    /// Text-mode shape (upright, italic, slanted, small caps).
    TextShape,
    /// Text-mode size (e.g. `\large` in text mode).
    TextSize,
}

/// Canonical value carried by an attribute slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttrValue {
    /// Math font family identifier, for the `MathFont` axis.
    MathFont(MathFontValue),
    /// Scaled size factor, shared by the `MathSize` and `TextSize` axes.
    Size(SizeValue),
    /// Math style (cramped/display flags and nesting level), for `MathStyle`.
    Style(StyleValue),
    /// Text font family, for the `TextFamily` axis.
    TextFamily(TextFamily),
    /// Text weight, for the `TextSeries` axis.
    TextSeries(TextSeries),
    /// Text shape, for the `TextShape` axis.
    TextShape(TextShape),
}

/// Math font family name carried by the `MathFont` attribute axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MathFontValue(pub &'static str);

/// Stable scaled-integer representation of a size factor (value × 100 rounded).
/// Avoids using a bare `f64` as a `Eq + Hash` key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SizeValue(pub i32);

/// Canonical math style: which `\...style` command and its rendering effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StyleValue {
    /// Style letter (`D`, `T`, `S`, `SS`) identifying the canonical command.
    pub letter: &'static str,
    /// Whether the style renders in display layout.
    pub display: bool,
    /// Script nesting level (0 = display/text, increasing when cramped).
    pub level: u8,
}

/// Canonical text font family for the `TextFamily` axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextFamily {
    /// Roman / serif (`\textrm`).
    Roman,
    /// Sans-serif (`\textsf`).
    SansSerif,
    /// Monospace / typewriter (`\texttt`).
    Typewriter,
    /// Calligraphic.
    Calligraphic,
    /// Italic family.
    Italic,
    /// Oldstyle figures.
    Oldstyle,
}

/// Canonical text weight for the `TextSeries` axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextSeries {
    /// Normal weight (`\textmd`).
    Medium,
    /// Bold weight (`\textbf`).
    Bold,
}

/// Canonical text shape for the `TextShape` axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextShape {
    /// Upright (`\textup`).
    Upright,
    /// Italic (`\textit`).
    Italic,
    /// Slanted (`\textsl`).
    Slanted,
    /// Small caps (`\textsc`).
    SmallCaps,
}

/// One canonical attribute axis paired with its value (e.g. `MathFont` =
/// `mathbf`); the key used to bucket [`AttributeStat`] entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeSet {
    attr: Attr,
    value: AttrValue,
}

impl AttributeSet {
    pub const fn new(attr: Attr, value: AttrValue) -> Self {
        Self { attr, value }
    }

    pub const fn attr(self) -> Attr {
        self.attr
    }

    pub const fn value(self) -> AttrValue {
        self.value
    }
}

impl LowerAttributesReport {
    fn stat_mut(&mut self, set: AttributeSet) -> &mut AttributeStat {
        self.attributes.entry(set).or_default()
    }

    fn record_consumed_declarative(&mut self, set: AttributeSet) {
        self.stat_mut(set).consumed.declaratives += 1;
    }

    fn record_consumed_prefix(&mut self, set: AttributeSet) {
        self.stat_mut(set).consumed.prefixes += 1;
    }

    fn record_redundant_declarative(&mut self, set: AttributeSet) {
        self.stat_mut(set).redundant.declaratives += 1;
    }

    fn record_redundant_prefix(&mut self, set: AttributeSet) {
        self.stat_mut(set).redundant.prefixes += 1;
    }

    fn record_emitted_declarative(&mut self, set: AttributeSet) {
        self.stat_mut(set).emitted.declaratives += 1;
    }

    fn record_emitted_prefix(&mut self, set: AttributeSet) {
        self.stat_mut(set).emitted.prefixes += 1;
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AxisList {
    axes: [Attr; 4],
    len: usize,
}

impl AxisList {
    const fn new() -> Self {
        Self {
            axes: [Attr::MathFont; 4],
            len: 0,
        }
    }

    fn push(&mut self, attr: Attr) {
        self.axes[self.len] = attr;
        self.len += 1;
    }

    fn as_slice(&self) -> &[Attr] {
        &self.axes[..self.len]
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }
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

    fn merge_from(&mut self, other: Self, mode: ContentMode) {
        for &attr in Self::axis_order(mode) {
            if let Some(value) = other.get(attr) {
                *self.slot_mut(attr) = Some(value);
            }
        }
    }

    fn diff_axes(self, inherited: Self, mode: ContentMode) -> AxisList {
        let mut list = AxisList::new();
        for &attr in Self::axis_order(mode) {
            if self.get(attr) != inherited.get(attr) {
                list.push(attr);
            }
        }
        list
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

/// Enablement carrier for the LowerAttributes phase.
///
/// The engine reads [`Self::enabled`] to decide whether to call [`run`].
/// [`run`] itself does not consult this flag; it always processes the subtree
/// it is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LowerAttributesConfig {
    /// Whether the engine runs the LowerAttributes phase. [`run`] does not
    /// read this flag.
    pub enabled: bool,
}

impl LowerAttributesConfig {
    pub const ENABLED: Self = Self { enabled: true };
    pub const DISABLED: Self = Self { enabled: false };
    pub const DEFAULTS: Self = Self::ENABLED;
}

/// Reports whether the AST was modified during attribute lowering.
///
/// Unrelated trees return `false`; subtrees containing attributes that go
/// through rebuild may conservatively return `true` when regenerating an
/// equivalent shape.
pub fn run(
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    _config: &LowerAttributesConfig,
    recorder: &mut ReportRecorder,
) -> bool {
    let tables = lookup_tables();
    let root = ast.root();
    let root_mode = ast.node(root).content_mode().unwrap_or(ContentMode::Math);
    let mut relevant = FxHashSet::default();
    if !mark_relevant_subtrees(ast, root, root_mode, tables, &mut relevant) {
        return false;
    }
    let mut pass = Pass {
        relevant: Some(relevant),
        parse_ctx,
        recorder,
        edges_buf: Vec::new(),
        changed: false,
    };
    canonicalize_subtree(ast, root, AttributeState::default(), root_mode, &mut pass);
    pass.changed
}

struct Pass<'a> {
    // `Some(set)` skips subtrees outside `set`; `None` visits every node (tests).
    relevant: Option<FxHashSet<NodeId>>,
    parse_ctx: &'a ParseContext,
    recorder: &'a mut ReportRecorder,
    edges_buf: Vec<(NodeId, Slot)>,
    changed: bool,
}

impl Pass<'_> {
    #[inline]
    fn is_relevant(&self, node: NodeId) -> bool {
        self.relevant
            .as_ref()
            .is_none_or(|relevant| relevant.contains(&node))
    }

    #[inline]
    fn mark_changed(&mut self) {
        self.changed = true;
    }

    fn lower_attributes(&mut self, record: impl FnOnce(&mut LowerAttributesReport)) {
        self.recorder.lower_attributes(record);
    }
}

#[inline]
fn text_has_boundary_whitespace(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty() && trimmed.len() != text.len()
}

#[inline]
fn has_boundary_whitespace(ast: &Ast, node: NodeId) -> bool {
    matches!(ast.node(node), Node::Text(text) if text_has_boundary_whitespace(text))
}

fn is_node_relevant(node: &Node, mode: ContentMode, tables: &LookupTables) -> bool {
    match node {
        Node::Declarative { name, args } => {
            args.is_empty()
                && tables
                    .declaratives
                    .get(name.as_str())
                    .is_some_and(|slot| slot.get(mode).is_some())
        }
        Node::Command { name, .. } => tables.prefixes.contains_key(name.as_str()),
        Node::Text(text) => mode == ContentMode::Math && text_has_boundary_whitespace(text),
        _ => false,
    }
}

fn content_slot_mode(ast: &Ast, parent: NodeId, slot: Slot) -> Option<ContentMode> {
    match slot {
        Slot::GroupChild(_) => ast.node(parent).content_mode(),
        Slot::Argument(index) => ast.arg_slots(parent)[index]
            .as_ref()
            .and_then(|arg| arg.value.content())
            .map(|(_, mode)| mode),
        Slot::ScriptBase | Slot::ScriptSub | Slot::ScriptSup | Slot::EnvBody => {
            Some(ContentMode::Math)
        }
        // `InfixLeft` / `InfixRight` are skipped by `canonicalize_content_slots`.
        Slot::InfixLeft | Slot::InfixRight => None,
    }
}

fn mark_relevant_subtrees(
    ast: &Ast,
    node: NodeId,
    mode: ContentMode,
    tables: &LookupTables,
    relevant: &mut FxHashSet<NodeId>,
) -> bool {
    let mut is_rel = is_node_relevant(ast.node(node), mode, tables);
    ast.for_each_edge(node, |child, slot| {
        if let Some(child_mode) = content_slot_mode(ast, node, slot)
            && mark_relevant_subtrees(ast, child, child_mode, tables, relevant)
        {
            is_rel = true;
        }
    });
    if is_rel {
        relevant.insert(node);
    }
    is_rel
}

/// Whether `node` can be rewritten, unwrapped, or split at its own container/slot
/// level in `mode`. Nodes for which this is `false` only need subtree recursion.
fn is_direct_attribute_node(ast: &Ast, node: NodeId, mode: ContentMode) -> bool {
    lookup_declarative_at(ast, node, mode).is_some()
        || (lookup_prefix_at(ast, node, mode).is_some()
            && mandatory_content_child(ast, node).is_some())
        || lookup_prefix_for_content_mode(ast, node, mode).is_some()
        || (mode == ContentMode::Math && has_boundary_whitespace(ast, node))
        || (is_brace_group(ast, node) && has_direct_declarative_marker(ast, node, mode))
}

// ---------------------------------------------------------------------------
// Traversal
// ---------------------------------------------------------------------------

fn canonicalize_subtree(
    ast: &mut Ast,
    node_id: NodeId,
    inherited: AttributeState,
    mode: ContentMode,
    recorder: &mut Pass<'_>,
) {
    if !recorder.is_relevant(node_id) {
        return;
    }
    let container_mode = match ast.node(node_id) {
        Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
        _ => None,
    };

    if let Some(container_mode) = container_mode {
        process_container(ast, node_id, inherited, container_mode, recorder);
    } else {
        canonicalize_content_slots(ast, node_id, inherited, mode, recorder);
    }
}

fn canonicalize_content_slots(
    ast: &mut Ast,
    parent: NodeId,
    inherited: AttributeState,
    parent_mode: ContentMode,
    recorder: &mut Pass<'_>,
) {
    let base = recorder.edges_buf.len();
    ast.push_edges(parent, &mut recorder.edges_buf);
    let end = recorder.edges_buf.len();
    for i in base..end {
        let (child, slot) = recorder.edges_buf[i];
        let Some(child_mode) = content_slot_mode(ast, parent, slot) else {
            continue;
        };
        if matches!(slot, Slot::EnvBody) {
            if recorder.is_relevant(child) {
                let child_inherited = inherited_for_content_slot(
                    ast,
                    parent,
                    inherited,
                    parent_mode,
                    child_mode,
                    slot,
                );
                canonicalize_subtree(ast, child, child_inherited, child_mode, recorder);
            }
        } else {
            canonicalize_content_slot(
                ast,
                parent,
                child,
                slot,
                inherited,
                parent_mode,
                child_mode,
                recorder,
            );
        }
    }
    recorder.edges_buf.truncate(base);
}

#[allow(clippy::too_many_arguments)]
fn canonicalize_content_slot(
    ast: &mut Ast,
    parent: NodeId,
    child: NodeId,
    slot: Slot,
    inherited: AttributeState,
    parent_mode: ContentMode,
    child_mode: ContentMode,
    recorder: &mut Pass<'_>,
) {
    if !recorder.is_relevant(child) {
        return;
    }
    let child_inherited =
        inherited_for_content_slot(ast, parent, inherited, parent_mode, child_mode, slot);
    if !is_direct_attribute_node(ast, child, child_mode) {
        canonicalize_subtree(ast, child, child_inherited, child_mode, recorder);
        return;
    }
    // A math argument such as the body of `\pmatrix` or `\substack` may be an
    // alignment, so its own separators close declarative scopes.
    let cells = matches!(slot, Slot::Argument(_))
        && child_mode == ContentMode::Math
        && is_brace_group(ast, child);

    let placeholder = empty_implicit_group(ast, child_mode);
    ast.replace_content_child(child, placeholder);

    let collected = if cells {
        let pairs = collect_brace_group(ast, child, child_inherited, child_mode, true, recorder);
        CollectResult {
            pairs,
            final_state: child_inherited,
        }
    } else {
        collect_single_detached_node(ast, child, child_inherited, child_mode, recorder)
    };
    let rebuilt = segment_and_emit(
        ast,
        collected.pairs,
        child_inherited,
        child_mode,
        cells,
        recorder,
    );
    let replacement = single_content_replacement(ast, rebuilt, child_mode, recorder);
    if replacement != child {
        recorder.mark_changed();
    }
    ast.replace_content_child(placeholder, replacement);
    ast.remove_detached(placeholder);
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

/// Attribute baseline for one nested content slot.
///
/// Command arguments are fail-closed: only commands verified to inherit
/// attributes identically in both MathJax 4 and XeTeX are whitelisted.
/// Commands whose XeTeX implementation boxes or restarts math mode (such as
/// `\sqrt[n]{a}` via amsmath's `\r@@t`, `\overbrace`, `\underbrace`, `\boxed`,
/// `\smash`, `\phantom`) and all unknown or alignment commands reset attributes
/// so inner wrappers are never dropped as redundant.
fn inherited_for_content_slot(
    ast: &Ast,
    parent: NodeId,
    inherited: AttributeState,
    parent_mode: ContentMode,
    child_mode: ContentMode,
    slot: Slot,
) -> AttributeState {
    let mut state = inherited_for_child_mode(inherited, parent_mode, child_mode);
    match slot {
        Slot::EnvBody | Slot::InfixLeft | Slot::InfixRight => state.with_mode_reset(child_mode),
        Slot::ScriptBase | Slot::GroupChild(_) => state,
        Slot::ScriptSub | Slot::ScriptSup => {
            if child_mode == ContentMode::Math {
                state.math_style = None;
                state.math_size = None;
            }
            state
        }
        Slot::Argument(_) => {
            if let Node::Command { name, args, .. } = ast.node(parent) {
                match child_mode {
                    ContentMode::Math if command_inherits_math_font(name, args) => {
                        state.math_style = None;
                        state.math_size = None;
                        return state;
                    }
                    ContentMode::Text if command_inherits_text_attributes(name) => {
                        return state;
                    }
                    _ => {}
                }
            }
            state.with_mode_reset(child_mode)
        }
    }
}

/// Dual-renderer-verified (MathJax 4 + XeTeX) math commands whose math
/// arguments inherit `math_font` (while still resetting `math_style` and
/// `math_size`).
///
/// Fail-closed by design: commands that box or restart math mode in XeTeX
/// (e.g. `\sqrt[n]{a}` with an optional root index via amsmath's `\r@@t`,
/// `\overbrace`, `\underbrace`, `\boxed`, `\smash`, `\phantom`), alignment
/// commands, font-changing commands, and unknown commands are excluded.
fn command_inherits_math_font(name: &str, args: &[Option<Argument>]) -> bool {
    if name == "sqrt" {
        // `\sqrt{a}` inherits `\fam` in both renderers, whereas `\sqrt[n]{a}`
        // invokes amsmath's `\r@@t` (`\hbox{$\m@th ...$}`) in XeTeX, resetting
        // `\fam` for both the root index and the radicand.
        return args.first().is_none_or(Option::is_none);
    }
    matches!(
        name,
        // Fractions and binomials
        "frac"
            | "dfrac"
            | "tfrac"
            | "cfrac"
            | "binom"
            | "dbinom"
            | "tbinom"
            | "genfrac"
            // Simple accents and line decorations
            | "hat"
            | "widehat"
            | "bar"
            | "overline"
            | "tilde"
            | "widetilde"
            | "vec"
            | "dot"
            | "ddot"
            | "dddot"
            | "ddddot"
            | "check"
            | "acute"
            | "grave"
            | "breve"
            | "mathring"
            | "underline"
            | "not"
            // Stacked wrappers
            | "overset"
            | "underset"
            | "stackrel"
            | "stackbin"
            // Math-class wrappers
            | "mathrel"
            | "mathop"
            | "mathbin"
            | "mathord"
            | "mathopen"
            | "mathclose"
            | "mathpunct"
            | "mathinner"
            // Color prefix
            | "textcolor"
    )
}

/// Dual-renderer-verified (MathJax 4 + XeTeX) text commands whose text
/// arguments inherit the enclosing NFSS text attributes.
fn command_inherits_text_attributes(name: &str) -> bool {
    matches!(name, "underline" | "textcolor")
}

fn single_content_replacement(
    ast: &mut Ast,
    mut nodes: Vec<NodeId>,
    mode: ContentMode,
    recorder: &mut Pass<'_>,
) -> NodeId {
    if nodes.len() == 1 {
        return nodes.pop().expect("single content node should exist");
    }

    recorder.mark_changed();
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
    recorder: &mut Pass<'_>,
) {
    let len = ast.children(container).len();
    if len == 0 {
        return;
    }

    if !ast
        .children(container)
        .iter()
        .any(|&child| is_direct_attribute_node(ast, child, mode))
    {
        let mut after_unknown = false;
        for index in 0..len {
            let child = ast.children(container)[index];
            let child_inherited = if after_unknown && is_brace_group(ast, child) {
                inherited.with_mode_reset(mode)
            } else {
                inherited
            };
            after_unknown = next_after_unknown(ast, child, after_unknown);
            canonicalize_subtree(ast, child, child_inherited, mode, recorder);
        }
        return;
    }

    // Environment bodies may be alignments. The root is not a cell, so a
    // top-level `\\` keeps the running state.
    let cells = mode == ContentMode::Math && is_brace_group(ast, container);

    let env = ast
        .parent(container)
        .filter(|link| link.slot == Slot::EnvBody)
        .and_then(|_| ast.parent_id(container));
    let detached = ast.detach_children_range(container, 0..len);
    let collected =
        collect_detached_children(ast, &detached, inherited, mode, cells, env, recorder);
    record_trailing_empty_segment(
        &collected.pairs,
        collected.final_state,
        inherited,
        mode,
        recorder,
    );
    let rebuilt = segment_and_emit(ast, collected.pairs, inherited, mode, cells, recorder);
    if rebuilt != detached {
        recorder.mark_changed();
    }
    let removed = ast.replace_children(container, rebuilt);
    debug_assert!(removed.is_empty());
}

/// Collect a detached child list starting from `inherited`.
///
/// With `cells`, every alignment separator in the list restores `inherited`,
/// closing declarative scopes opened in the preceding cell. `env` is the
/// environment whose body the list is, if any.
fn collect_detached_children(
    ast: &mut Ast,
    children: &[NodeId],
    inherited: AttributeState,
    mode: ContentMode,
    cells: bool,
    env: Option<NodeId>,
    recorder: &mut Pass<'_>,
) -> CollectResult {
    let mut pairs = Vec::with_capacity(children.len());
    let mut state = inherited;
    let mut after_unknown = false;

    for (index, &child) in children.iter().enumerate() {
        if cells && is_alignment_separator(ast, child) {
            state = inherited;
            after_unknown = false;
            pairs.push(Pair { state, node: child });
            continue;
        }
        if vanishes(ast, child, state, mode)
            && removal_creates_hazard(
                ast,
                state,
                mode,
                pairs.last(),
                &children[index + 1..],
                env,
                recorder,
            )
        {
            after_unknown = false;
            pairs.push(Pair { state, node: child });
            continue;
        }
        // Brace groups immediately following an unknown command may be its
        // unattached arguments; keep their braces and reset inherited
        // attributes fail-closed inside them.
        if after_unknown && is_brace_group(ast, child) {
            canonicalize_subtree(ast, child, state.with_mode_reset(mode), mode, recorder);
            pairs.push(Pair { state, node: child });
            continue;
        }
        after_unknown = next_after_unknown(ast, child, after_unknown);
        collect_detached_child(ast, child, &mut state, mode, recorder, &mut pairs);
    }

    CollectResult {
        pairs,
        final_state: state,
    }
}

/// Whether `child` would vanish without a trace: a redundant declarative or
/// a prefix with an empty body.
///
/// Such a node stays when removing it would create a boundary hazard, as in
/// the `physics` `\Im\mathbf{}{x}`: without the empty `\mathbf{}`, `\Im`
/// would read `{x}` as its argument. Keeping it preserves the rendering, which
/// this phase promises; boundary separation would turn the command into an
/// ordinary subformula instead.
fn vanishes(ast: &Ast, child: NodeId, state: AttributeState, mode: ContentMode) -> bool {
    if let Some(entry) = lookup_declarative_at(ast, child, mode) {
        state.get(entry.set.attr) == Some(entry.set.value)
    } else {
        (lookup_prefix_at(ast, child, mode).is_some()
            || lookup_prefix_for_content_mode(ast, child, mode).is_some())
            && mandatory_content_child(ast, child).is_some_and(|body| is_empty_body(ast, body))
    }
}

/// Whether removing a vanishing node between `last` and `after` would create
/// a boundary hazard. At the start of an environment body, the environment
/// is the reader. A neighbor with another state is emitted in another segment
/// and its wrapper separates the two.
fn removal_creates_hazard(
    ast: &Ast,
    state: AttributeState,
    mode: ContentMode,
    last: Option<&Pair>,
    after: &[NodeId],
    env: Option<NodeId>,
    recorder: &Pass<'_>,
) -> bool {
    let parse_ctx = recorder.parse_ctx;
    let catalog = |mode| parse_ctx.catalog(mode);
    let Some(last) = last else {
        return env
            .is_some_and(|env| environment_captures_in(ast, catalog, mode, env, after.iter()));
    };
    if last.state != state {
        return false;
    }
    sequence_boundary_hazard(
        ast,
        catalog,
        mode,
        std::iter::once(&last.node),
        after.iter(),
    )
}

/// Whether a prefix body holds nothing but nested empty brace groups.
fn is_empty_body(ast: &Ast, body: NodeId) -> bool {
    is_brace_group(ast, body)
        && match ast.children(body) {
            [] => true,
            [child] => is_empty_body(ast, *child),
            _ => false,
        }
}

fn collect_detached_child(
    ast: &mut Ast,
    child: NodeId,
    state: &mut AttributeState,
    mode: ContentMode,
    recorder: &mut Pass<'_>,
    pairs: &mut Vec<Pair>,
) {
    if !recorder.is_relevant(child) {
        pairs.push(Pair {
            state: *state,
            node: child,
        });
        return;
    }
    if let Some(entry) = lookup_declarative_at(ast, child, mode) {
        consume_declarative(ast, child, state, entry, recorder);
        return;
    }

    if let Some(entry) = lookup_prefix_at(ast, child, mode)
        && mandatory_content_child(ast, child).is_some()
    {
        let previous = *state;
        let body_pairs = collect_prefix_body(ast, child, previous, entry, mode, recorder);
        recorder.lower_attributes(|report| {
            if prefix_is_fully_absorbed(previous, entry.set, &body_pairs) {
                report.record_redundant_prefix(entry.set);
            }
        });
        pairs.extend(body_pairs);
        ast.remove_detached(child);
        return;
    }

    if let Some((entry, child_mode)) = lookup_prefix_for_content_mode(ast, child, mode) {
        let previous = inherited_for_child_mode(*state, mode, child_mode);
        let body_pairs = collect_prefix_body(ast, child, previous, entry, child_mode, recorder);
        recorder.lower_attributes(|report| {
            if prefix_is_fully_absorbed(previous, entry.set, &body_pairs) {
                report.record_redundant_prefix(entry.set);
            }
        });
        let rebuilt = segment_and_emit(ast, body_pairs, previous, child_mode, false, recorder);
        pairs.extend(rebuilt.into_iter().map(|node| Pair {
            state: *state,
            node,
        }));
        ast.remove_detached(child);
        return;
    }

    if is_brace_group(ast, child) {
        pairs.extend(collect_brace_group(
            ast, child, *state, mode, false, recorder,
        ));
        return;
    }

    canonicalize_subtree(ast, child, *state, mode, recorder);
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
    recorder: &mut Pass<'_>,
) {
    recorder.mark_changed();
    recorder.lower_attributes(|report| report.record_consumed_declarative(entry.set));
    if !state.set(entry.set) {
        recorder.lower_attributes(|report| report.record_redundant_declarative(entry.set));
    }
    ast.remove_detached(node);
}

fn collect_prefix_body(
    ast: &mut Ast,
    prefix: NodeId,
    previous: AttributeState,
    entry: &'static PrefixEntry,
    mode: ContentMode,
    recorder: &mut Pass<'_>,
) -> Vec<Pair> {
    recorder.mark_changed();
    recorder.lower_attributes(|report| report.record_consumed_prefix(entry.set));
    let mut body_state = previous;
    body_state.set(entry.set);
    let mut body =
        mandatory_content_child(ast, prefix).expect("registered prefix should have a body");
    // The argument's own scope also encloses every directly nested brace
    // group. Consume those redundant containers before collecting attributes,
    // so rebuilt singleton arguments are already a local fixed point.
    while is_brace_group(ast, body) {
        let [child] = ast.children(body) else {
            break;
        };
        let child = *child;
        if !is_brace_group(ast, child) {
            break;
        }
        ast.detach_children_range(body, 0..1);
        ast.replace_content_child(body, child);
        ast.remove_detached(body);
        body = child;
    }

    match ast.node(body) {
        Node::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            ..
        } => {
            let len = ast.children(body).len();
            let detached = ast.detach_children_range(body, 0..len);
            detach_body_from_prefix(ast, body, mode);
            let collected =
                collect_detached_children(ast, &detached, body_state, mode, false, None, recorder);
            record_trailing_empty_segment(
                &collected.pairs,
                collected.final_state,
                body_state,
                mode,
                recorder,
            );
            // A prefix argument is one scope. Declarative-only effects inside it
            // have to be normalized in that group; lifting them would emit the
            // declaration beside the rebuilt wrapper.
            if pairs_require_local_declarative(&collected.pairs, body_state, mode) {
                let absorbed = prefix_is_fully_absorbed(previous, entry.set, &collected.pairs);
                let mut nodes =
                    segment_and_emit(ast, collected.pairs, body_state, mode, false, recorder);
                if absorbed {
                    // Rebuilding can move a declarative inside an overriding prefix.
                    // Mark newly emitted nodes so recollection visits them.
                    if let Some(relevant) = &mut recorder.relevant {
                        let tables = lookup_tables();
                        for &node in &nodes {
                            mark_relevant_subtrees(ast, node, mode, tables, relevant);
                        }
                    }
                    let recollected = collect_detached_children(
                        ast, &nodes, body_state, mode, false, None, recorder,
                    );
                    if !pairs_require_local_declarative(&recollected.pairs, body_state, mode) {
                        ast.remove_detached(body);
                        return recollected.pairs;
                    }
                    nodes =
                        segment_and_emit(ast, recollected.pairs, body_state, mode, false, recorder);
                }
                let removed = ast.replace_children(body, nodes);
                debug_assert!(removed.is_empty());
                vec![Pair {
                    state: body_state,
                    node: body,
                }]
            } else {
                ast.remove_detached(body);
                collected.pairs
            }
        }
        _ => {
            detach_body_from_prefix(ast, body, mode);
            collect_single_detached_node(ast, body, body_state, mode, recorder).pairs
        }
    }
}

fn detach_body_from_prefix(ast: &mut Ast, body: NodeId, mode: ContentMode) {
    let placeholder = empty_implicit_group(ast, mode);
    ast.replace_content_child(body, placeholder);
}

fn collect_brace_group(
    ast: &mut Ast,
    group: NodeId,
    inherited: AttributeState,
    mode: ContentMode,
    cells: bool,
    recorder: &mut Pass<'_>,
) -> Vec<Pair> {
    if !has_direct_declarative_marker(ast, group, mode) {
        canonicalize_subtree(ast, group, inherited, mode, recorder);
        return vec![Pair {
            state: inherited,
            node: group,
        }];
    }

    let len = ast.children(group).len();
    let detached = ast.detach_children_range(group, 0..len);
    let inner = collect_detached_children(ast, &detached, inherited, mode, cells, None, recorder);
    record_trailing_empty_segment(&inner.pairs, inner.final_state, inherited, mode, recorder);

    // Keep the original braces when any carried effect still needs a declarative.
    // Prefix-only effects may be lifted; the caller continues with `inherited`,
    // which is the state from before this group.
    if pairs_require_local_declarative(&inner.pairs, inherited, mode) {
        let nodes = segment_and_emit(ast, inner.pairs, inherited, mode, cells, recorder);
        if nodes != detached {
            recorder.mark_changed();
        }
        let removed = ast.replace_children(group, nodes);
        debug_assert!(removed.is_empty());
        return vec![Pair {
            state: inherited,
            node: group,
        }];
    }

    if !inner.pairs.is_empty() && inner.pairs.iter().any(|pair| pair.state != inherited) {
        recorder.mark_changed();
        ast.remove_detached(group);
        return inner.pairs;
    }

    let nodes: Vec<NodeId> = inner.pairs.into_iter().map(|pair| pair.node).collect();
    if nodes != detached {
        recorder.mark_changed();
    }
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
    recorder: &mut Pass<'_>,
) -> CollectResult {
    let mut pairs = Vec::with_capacity(1);
    let mut state = inherited;
    collect_detached_child(ast, node, &mut state, mode, recorder, &mut pairs);
    record_trailing_empty_segment(&pairs, state, inherited, mode, recorder);
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
    recorder: &mut Pass<'_>,
) {
    recorder.lower_attributes(|report| {
        let segment_state = pairs.last().map_or(inherited, |pair| pair.state);
        if !final_state.diff_axes(segment_state, mode).is_empty() {
            report.eliminated_empty_segments += 1;
        }
    });
}

fn has_direct_declarative_marker(ast: &Ast, group: NodeId, mode: ContentMode) -> bool {
    ast.children(group)
        .iter()
        .any(|child| lookup_declarative_at(ast, *child, mode).is_some())
}

fn is_alignment_separator(ast: &Ast, node: NodeId) -> bool {
    match ast.node(node) {
        Node::AlignmentTab => true,
        Node::Command { name, .. } => [&base::cmd::_BACKSLASH, &base::cmd::NEWLINE, &base::cmd::CR]
            .iter()
            .any(|command| command.name == name),
        _ => false,
    }
}

fn is_brace_group(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(node),
        Node::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            ..
        }
    )
}

fn is_unknown_command_node(ast: &Ast, node: NodeId) -> bool {
    match ast.node(node) {
        Node::Command { known: false, .. } => true,
        Node::Scripted { base, .. } => is_unknown_command_node(ast, *base),
        _ => false,
    }
}

fn next_after_unknown(ast: &Ast, node: NodeId, after_unknown: bool) -> bool {
    is_unknown_command_node(ast, node)
        || (after_unknown && (is_brace_group(ast, node) || is_plain_whitespace_node(ast, node)))
}

fn mandatory_content_child(ast: &Ast, node: NodeId) -> Option<NodeId> {
    let mut found = None;
    for argument in ast.arg_slots(node).iter().flatten() {
        if !matches!(argument.kind, crate::ast::ArgumentKind::Mandatory) {
            continue;
        }
        let child = match argument.value {
            ArgumentValue::MathContent(child)
            | ArgumentValue::TextContent(child)
            | ArgumentValue::OperatorNameContent(child) => child,
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

fn pairs_require_local_declarative(
    pairs: &[Pair],
    baseline: AttributeState,
    mode: ContentMode,
) -> bool {
    pairs
        .iter()
        .any(|pair| state_requires_local_declarative(pair.state, baseline, mode))
}

fn state_requires_local_declarative(
    state: AttributeState,
    baseline: AttributeState,
    mode: ContentMode,
) -> bool {
    state
        .diff_axes(baseline, mode)
        .as_slice()
        .iter()
        .any(|&attr| {
            state
                .get(attr)
                .is_some_and(|value| attribute_needs_local_declarative(attr, value, mode))
        })
}

/// True when the canonical target for this value has no prefix wrapper.
///
/// The decision comes from `data.yaml` via [`lookup_target`]. A missing target
/// also keeps the scope: the value cannot be handed to a self-contained wrapper.
fn attribute_needs_local_declarative(attr: Attr, value: AttrValue, mode: ContentMode) -> bool {
    match lookup_target(attr, value, mode) {
        Some(target) => target.prefix.is_none(),
        None => true,
    }
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

    body_pairs.is_empty()
        || body_pairs
            .iter()
            .all(|pair| pair.state.get(set.attr) != Some(set.value))
}

fn segment_and_emit(
    ast: &mut Ast,
    pairs: Vec<Pair>,
    inherited: AttributeState,
    mode: ContentMode,
    cells: bool,
    recorder: &mut Pass<'_>,
) -> Vec<NodeId> {
    let mut rebuilt = Vec::with_capacity(pairs.len());
    // `ambient` is the state actually in effect after the declarations emitted
    // so far. Only a declarative that stays outside later prefix wrappers
    // remains active for the next sibling. The next segment is diffed against
    // this emitted state.
    // Restoring an explicit value that matches the state from before the group
    // still emits that known declarative. Nothing here invents a default style
    // or size when the recorded value is absent.
    let mut ambient = inherited;
    let mut iter = pairs.into_iter().peekable();

    // With `cells`, a separator collected at this level (it carries the list
    // state) is its own segment and starts the next cell with no emitted
    // declaration in effect. Separators lifted from a nested group with another
    // state stay in that group's segment, as before.
    let is_cell_separator = |ast: &Ast, pair: &Pair| {
        cells && pair.state == inherited && is_alignment_separator(ast, pair.node)
    };
    while let Some(first) = iter.next() {
        if is_cell_separator(ast, &first) {
            ambient = inherited;
            rebuilt.push(first.node);
            continue;
        }
        let segment_state = first.state;
        if segment_state.diff_axes(ambient, mode).is_empty() {
            // No attribute wrapper or declarative is emitted for this run;
            // push nodes directly (splitting boundary whitespace in math mode).
            push_ambient_node(ast, first.node, mode, recorder, &mut rebuilt);
            while let Some(next) = iter.peek() {
                if next.state != segment_state || is_cell_separator(ast, next) {
                    break;
                }
                let node = iter.next().expect("peeked segment pair should exist").node;
                push_ambient_node(ast, node, mode, recorder, &mut rebuilt);
            }
            continue;
        }

        let mut segment = vec![first.node];

        while let Some(next) = iter.peek() {
            if next.state != segment_state || is_cell_separator(ast, next) {
                break;
            }
            segment.push(iter.next().expect("peeked segment pair should exist").node);
        }

        let wrapped = wrap_with_canonical(ast, segment, segment_state, ambient, mode, recorder);
        rebuilt.extend(wrapped.nodes);
        ambient.merge_from(wrapped.outer_declaratives, mode);
    }

    rebuilt
}

fn push_ambient_node(
    ast: &mut Ast,
    node: NodeId,
    mode: ContentMode,
    recorder: &mut Pass<'_>,
    out: &mut Vec<NodeId>,
) {
    if mode == ContentMode::Math && has_boundary_whitespace(ast, node) {
        push_split_text_boundary_whitespace(ast, node, recorder, out);
    } else {
        out.push(node);
    }
}

fn is_plain_whitespace_node(ast: &Ast, node: NodeId) -> bool {
    matches!(ast.node(node), Node::Text(text) if text.chars().all(char::is_whitespace))
}

fn segment_is_plain_whitespace(ast: &Ast, nodes: &[NodeId]) -> bool {
    !nodes.is_empty()
        && nodes
            .iter()
            .all(|node| is_plain_whitespace_node(ast, *node))
}

fn push_split_text_boundary_whitespace(
    ast: &mut Ast,
    node: NodeId,
    recorder: &mut Pass<'_>,
    out: &mut Vec<NodeId>,
) {
    let Node::Text(text) = ast.node(node) else {
        out.push(node);
        return;
    };
    let leading_len = text.len() - text.trim_start_matches(char::is_whitespace).len();
    let trailing_start = text.trim_end_matches(char::is_whitespace).len();
    let has_middle = leading_len < trailing_start;
    let has_leading = leading_len > 0;
    let has_trailing = trailing_start < text.len();

    if !has_middle || (!has_leading && !has_trailing) {
        out.push(node);
        return;
    }

    let leading = has_leading.then(|| text[..leading_len].to_string());
    let middle = text[leading_len..trailing_start].to_string();
    let trailing = has_trailing.then(|| text[trailing_start..].to_string());

    recorder.mark_changed();
    if let Some(leading) = leading {
        out.push(ast.new_node(Node::Text(leading)));
    }
    out.push(ast.new_node(Node::Text(middle)));
    if let Some(trailing) = trailing {
        out.push(ast.new_node(Node::Text(trailing)));
    }
    ast.remove_detached(node);
}

fn split_whitespace_boundaries(
    ast: &mut Ast,
    nodes: Vec<NodeId>,
    recorder: &mut Pass<'_>,
) -> Vec<Vec<NodeId>> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    let mut split_buf = Vec::with_capacity(3);

    for node in nodes {
        split_buf.clear();
        push_split_text_boundary_whitespace(ast, node, recorder, &mut split_buf);
        for &part in &split_buf {
            if is_plain_whitespace_node(ast, part) {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                out.push(vec![part]);
            } else {
                current.push(part);
            }
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

struct WrappedSegment {
    nodes: Vec<NodeId>,
    outer_declaratives: AttributeState,
}

fn wrap_with_canonical(
    ast: &mut Ast,
    children: Vec<NodeId>,
    state: AttributeState,
    inherited: AttributeState,
    mode: ContentMode,
    recorder: &mut Pass<'_>,
) -> WrappedSegment {
    debug_assert!(
        !children.is_empty(),
        "segment_and_emit must not call wrap_with_canonical with an empty segment"
    );

    if matches!(mode, ContentMode::Text)
        || !children
            .iter()
            .any(|&id| is_plain_whitespace_node(ast, id) || has_boundary_whitespace(ast, id))
    {
        return wrap_segment_with_canonical(ast, children, state, inherited, mode, recorder);
    }

    let mut rebuilt = Vec::with_capacity(children.len());
    let mut outer_declaratives = AttributeState::default();
    for run in split_whitespace_boundaries(ast, children, recorder) {
        if segment_is_plain_whitespace(ast, &run) {
            rebuilt.extend(run);
            continue;
        }
        let wrapped = wrap_segment_with_canonical(ast, run, state, inherited, mode, recorder);
        rebuilt.extend(wrapped.nodes);
        outer_declaratives.merge_from(wrapped.outer_declaratives, mode);
    }
    WrappedSegment {
        nodes: rebuilt,
        outer_declaratives,
    }
}

/// Wrap a non-empty segment body with the active attributes in mode-specific
/// order: attributes with a `prefix` target wrap the body into an implicit
/// group + prefix command (innermost first), while attributes without a prefix
/// prepend the corresponding declarative.
fn wrap_segment_with_canonical(
    ast: &mut Ast,
    mut children: Vec<NodeId>,
    state: AttributeState,
    inherited: AttributeState,
    mode: ContentMode,
    recorder: &mut Pass<'_>,
) -> WrappedSegment {
    let mut outer_declaratives = AttributeState::default();
    for &attr in emit_axis_order(state, inherited, mode).as_slice() {
        let Some(value) = state.get(attr) else {
            continue;
        };
        let Some(target) = lookup_target(attr, value, mode) else {
            continue;
        };

        if let Some(prefix) = target.prefix {
            // A rebuilt argument owns its container. Normalize that container
            // now rather than changing its shape on the next lowering pass.
            while children.len() == 1 && is_brace_group(ast, children[0]) {
                let group = children[0];
                let Node::Group {
                    mode: group_mode, ..
                } = ast.node(group)
                else {
                    unreachable!()
                };
                if *group_mode != mode {
                    break;
                }
                let len = ast.children(group).len();
                if len > 1 {
                    if let Some(Node::Group { kind, .. }) = ast.node_opt_mut(group)
                        && *kind != GroupKind::Implicit
                    {
                        *kind = GroupKind::Implicit;
                        recorder.mark_changed();
                    }
                    break;
                }
                recorder.mark_changed();
                children = ast.detach_children_range(group, 0..len);
                ast.remove_detached(group);
            }
            if children.is_empty() {
                return WrappedSegment {
                    nodes: Vec::new(),
                    outer_declaratives: AttributeState::default(),
                };
            }
            outer_declaratives = AttributeState::default();
            recorder.mark_changed();
            let group = if children.len() == 1 {
                children.pop().expect("single content node should exist")
            } else {
                ast.new_node(Node::Group {
                    children,
                    kind: GroupKind::Implicit,
                    mode,
                })
            };
            let command = ast.new_node(Node::Command {
                name: prefix.name.to_string(),
                args: vec![mandatory_content_slot(group, mode)],
                known: true,
            });
            recorder.lower_attributes(|report| {
                report.record_emitted_prefix(AttributeSet::new(attr, value))
            });
            children = vec![command];
        } else {
            recorder.mark_changed();
            children.insert(0, new_declarative_node(ast, target.declarative));
            recorder.lower_attributes(|report| {
                report.record_emitted_declarative(AttributeSet::new(attr, value));
            });
            outer_declaratives.set(AttributeSet::new(attr, value));
        }
    }

    WrappedSegment {
        nodes: children,
        outer_declaratives,
    }
}

fn emit_axis_order(
    state: AttributeState,
    inherited: AttributeState,
    mode: ContentMode,
) -> AxisList {
    let mut axes = state.diff_axes(inherited, mode);
    // Size is emitted before a math font so the font prefix wraps it
    // (`\mathbf{\large x}`). Text size stays last, so it remains outside
    // series and family prefixes. The wrapper returns the declarations that
    // remain outside those prefixes, so callers update the ambient state from
    // the actual emitted structure.
    if matches!(mode, ContentMode::Math) && axes.as_slice() == [Attr::MathFont, Attr::MathSize] {
        axes.axes.swap(0, 1);
    }
    axes
}

// ---------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------

struct ModeIndexedEntry<T: 'static> {
    math: Option<&'static T>,
    text: Option<&'static T>,
}

impl<T: 'static> Default for ModeIndexedEntry<T> {
    fn default() -> Self {
        Self {
            math: None,
            text: None,
        }
    }
}

impl<T: 'static> ModeIndexedEntry<T> {
    fn get(&self, mode: ContentMode) -> Option<&'static T> {
        match mode {
            ContentMode::Math => self.math,
            ContentMode::Text => self.text,
        }
    }

    fn set(&mut self, mode: ContentMode, entry: &'static T) {
        let slot = match mode {
            ContentMode::Math => &mut self.math,
            ContentMode::Text => &mut self.text,
        };
        debug_assert!(slot.is_none());
        *slot = Some(entry);
    }
}

struct LookupTables {
    declaratives: FxHashMap<&'static str, ModeIndexedEntry<DeclarativeEntry>>,
    prefixes: FxHashMap<&'static str, ModeIndexedEntry<PrefixEntry>>,
    targets: FxHashMap<AttributeSet, &'static generated::AttributeTargetEntry>,
}

fn lookup_tables() -> &'static LookupTables {
    static TABLES: OnceLock<LookupTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut declaratives: FxHashMap<&'static str, ModeIndexedEntry<DeclarativeEntry>> =
            FxHashMap::default();
        for entry in generated::DECLARATIVES {
            declaratives
                .entry(entry.name)
                .or_default()
                .set(entry.allowed_mode, entry);
        }

        let mut prefixes: FxHashMap<&'static str, ModeIndexedEntry<PrefixEntry>> =
            FxHashMap::default();
        for entry in generated::PREFIXES {
            prefixes
                .entry(entry.name)
                .or_default()
                .set(entry.allowed_mode, entry);
        }

        let mut targets: FxHashMap<AttributeSet, &'static generated::AttributeTargetEntry> =
            FxHashMap::default();
        for entry in generated::ATTRIBUTE_TARGETS {
            let prev = targets.insert(AttributeSet::new(entry.attr, entry.value), entry);
            debug_assert!(prev.is_none());
        }

        LookupTables {
            declaratives,
            prefixes,
            targets,
        }
    })
}

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

/// Names of the math-mode declaratives this phase consumes, such as `Large`.
pub(crate) fn declarative_names() -> impl Iterator<Item = &'static str> {
    generated::DECLARATIVES
        .iter()
        .filter(|entry| entry.allowed_mode == ContentMode::Math)
        .map(|entry| entry.name)
}

/// Names of the math-mode prefix wrappers this phase consumes, such as
/// `mathbf`.
pub(crate) fn prefix_names() -> impl Iterator<Item = &'static str> {
    generated::PREFIXES
        .iter()
        .filter(|entry| entry.allowed_mode == ContentMode::Math)
        .map(|entry| entry.name)
}

fn lookup_declarative(mode: ContentMode, name: &str) -> Option<&'static DeclarativeEntry> {
    lookup_tables()
        .declaratives
        .get(name)
        .and_then(|slot| slot.get(mode))
}

fn lookup_prefix_at(ast: &Ast, node_id: NodeId, mode: ContentMode) -> Option<&'static PrefixEntry> {
    let Node::Command { name, .. } = ast.node(node_id) else {
        return None;
    };
    lookup_tables()
        .prefixes
        .get(name.as_str())
        .and_then(|slot| slot.get(mode))
}

fn lookup_prefix_for_content_mode(
    ast: &Ast,
    node_id: NodeId,
    parent_mode: ContentMode,
) -> Option<(&'static PrefixEntry, ContentMode)> {
    let child_mode = mandatory_content_child_mode(ast, node_id)?;
    if child_mode == parent_mode {
        return None;
    }
    lookup_prefix_at(ast, node_id, child_mode).map(|entry| (entry, child_mode))
}

fn mandatory_content_child_mode(ast: &Ast, node: NodeId) -> Option<ContentMode> {
    let mut found = None;
    for argument in ast.arg_slots(node).iter().flatten() {
        if !matches!(argument.kind, crate::ast::ArgumentKind::Mandatory) {
            continue;
        }
        let child_mode = match argument.value {
            ArgumentValue::MathContent(_) | ArgumentValue::OperatorNameContent(_) => {
                ContentMode::Math
            }
            ArgumentValue::TextContent(_) => ContentMode::Text,
            _ => continue,
        };
        if found.replace(child_mode).is_some() {
            return None;
        }
    }
    found
}

fn lookup_target(attr: Attr, value: AttrValue, mode: ContentMode) -> Option<&'static ModeTarget> {
    lookup_tables()
        .targets
        .get(&AttributeSet::new(attr, value))
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
    fn skipping_irrelevant_subtrees_matches_full_traversal() {
        use super::*;
        let parser =
            crate::parse::ParseContext::from_packages(&texform_knowledge::packages::IMPORT_ORDER);
        for source in phase_cases::combinations() {
            let Ok((document, _)) = parser
                .parse(&source, &crate::parse::ParseConfig::default())
                .try_into_document()
            else {
                continue;
            };
            let mut fast = Ast::from_syntax_root(&document.to_syntax());
            let mut full = fast.clone();
            let before = fast.to_syntax_root();
            let mut fast_recorder = ReportRecorder::collecting();
            let changed = run(
                &mut fast,
                &parser,
                &LowerAttributesConfig::ENABLED,
                &mut fast_recorder,
            );
            let root = full.root();
            let mut full_recorder = ReportRecorder::collecting();
            canonicalize_subtree(
                &mut full,
                root,
                AttributeState::default(),
                ContentMode::Math,
                &mut Pass {
                    relevant: None,
                    parse_ctx: &parser,
                    recorder: &mut full_recorder,
                    edges_buf: Vec::new(),
                    changed: false,
                },
            );
            let fast_after = fast.to_syntax_root();
            assert_eq!(fast_after, full.to_syntax_root(), "{source}");
            assert_eq!(
                fast_recorder.into_report().lower_attributes,
                full_recorder.into_report().lower_attributes,
                "report mismatch for {source}"
            );
            assert!(
                changed || before == fast_after,
                "returned false despite mutating AST for {source}"
            );
        }
    }

    #[test]
    fn run_returns_false_on_unrelated_trees() {
        use super::*;
        let parser =
            crate::parse::ParseContext::from_packages(&["base", "ams", "physics", "textmacros"]);
        for source in [
            r"x + y",
            r"\frac{a}{b}",
            r"x_i^2 + e^k",
            r"\sum_{i=1}^n x_i",
            r"\text{ and }",
            r"\begin{array}{cc} a & b \end{array}",
        ] {
            let (document, _) = parser
                .parse(source, &crate::parse::ParseConfig::default())
                .try_into_document()
                .unwrap_or_else(|_| panic!("should parse: {source}"));
            let mut ast = Ast::from_syntax_root(&document.to_syntax());
            let before = ast.to_syntax_root();
            let changed = run(
                &mut ast,
                &parser,
                &LowerAttributesConfig::ENABLED,
                &mut ReportRecorder::disabled(),
            );
            assert!(!changed, "expected no change for unrelated {source}");
            assert_eq!(ast.to_syntax_root(), before, "{source}");
        }
    }

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
        let mut state = AttributeState::default();
        state.set(AttributeSet {
            attr: Attr::MathStyle,
            value: AttrValue::Style(super::StyleValue {
                letter: "D",
                display: true,
                level: 0,
            }),
        });
        state.set(AttributeSet {
            attr: Attr::MathFont,
            value: AttrValue::MathFont(MathFontValue("VARIANT.BOLD")),
        });
        state.set(AttributeSet {
            attr: Attr::TextFamily,
            value: AttrValue::TextFamily(TextFamily::Roman),
        });
        state.set(AttributeSet {
            attr: Attr::TextShape,
            value: AttrValue::TextShape(TextShape::Italic),
        });
        state.set(AttributeSet {
            attr: Attr::TextSeries,
            value: AttrValue::TextSeries(TextSeries::Bold),
        });

        assert_eq!(
            state.diff_axes(inherited, ContentMode::Math).as_slice(),
            &[Attr::MathFont, Attr::MathStyle]
        );
        assert_eq!(
            state.diff_axes(inherited, ContentMode::Text).as_slice(),
            &[Attr::TextShape, Attr::TextSeries, Attr::TextFamily]
        );
    }

    #[test]
    fn attribute_state_mode_reset_preserves_other_mode() {
        let mut state = AttributeState::default();
        state.set(AttributeSet {
            attr: Attr::MathSize,
            value: AttrValue::Size(SizeValue(120)),
        });
        state.set(AttributeSet {
            attr: Attr::TextSize,
            value: AttrValue::Size(SizeValue(85)),
        });
        state.set(AttributeSet {
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

#[cfg(test)]
#[path = "../../tests/support/phase_cases.rs"]
mod phase_cases;
