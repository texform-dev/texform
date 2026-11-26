//! Deterministic AST serializer — converts [`Ast`] back to LaTeX text.
//!
//! The serializer is independent of the transform stage: it covers the full AST
//! node vocabulary and makes no assumptions about whether the input has been
//! normalized. Its default style targets the `corpus` / `equiv` use cases with
//! strong disambiguation and explicit token boundaries in math mode, while text
//! mode content is preserved verbatim.
//!
//! # Architecture
//!
//! ```text
//! Serializer (recursive AST walk)
//!   -> emit atom with kind + mode
//!   -> AtomWriter decides inter-atom boundary
//!   -> String
//! ```
//!
//! Most spacing rules are concentrated in the atom writer's boundary decision.
//! Optional math spacing inspects the previous atom, the next atom, the
//! content mode, and the active [`SerializeOptions`]. Lexical separators are
//! a separate decision that no option can remove. Control-word separation
//! combines the previous atom's identity with the emitted ending and the next
//! character, following the lexer's ASCII command names; it covers control
//! sequences, `\left` / `\right` delimiters such as `\langle`, and error
//! snippets. In math mode, a control sequence also stays separated from a
//! sibling `[` or `*`, which could otherwise be absorbed as an optional or
//! star argument (`\\ [6pt]`). Text mode adds no other spaces.
//!
//! A few wrapper/scalar helpers still append a preformatted space directly
//! (empty padded groups). That byte is part of the
//! emitted ending, so later boundary checks must not insert a second lexical
//! separator from the old atom kind.
//! The serializer never repairs spacing with a post-pass string replacement.

use crate::{argument_delimiters::ContentBoundary, lexer::Token};
use logos::Logos;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::HashMap, ops::Range};

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind,
    Node, NodeId,
};
use crate::document::NodeSlot;

/// Serialize an AST to LaTeX using the default spaced style.
pub fn serialize(ast: &Ast) -> String {
    serialize_with(ast, &SerializeOptions::default())
}

/// Serialize an AST to LaTeX with explicit style options.
pub fn serialize_with(ast: &Ast, options: &SerializeOptions) -> String {
    let protection = RefCell::new(HashMap::new());
    let settings = options.style.settings();
    let mut serializer = Serializer::new(ast, &settings, NoopRecorder, &protection);
    serializer.serialize_root();
    serializer.finish()
}

/// Serialize an AST to LaTeX and record its typed output fragments.
pub fn serialize_fragmented(ast: &Ast) -> FragmentedLatex {
    serialize_fragmented_with(ast, &SerializeOptions::default())
}

/// Serialize an AST to LaTeX and typed output fragments with explicit options.
pub fn serialize_fragmented_with(ast: &Ast, options: &SerializeOptions) -> FragmentedLatex {
    let protection = RefCell::new(HashMap::new());
    let settings = options.style.settings();
    let mut serializer = Serializer::new(
        ast,
        &settings,
        FragmentRecorder::with_capacity(ast.node_count()),
        &protection,
    );
    serializer.serialize_root();
    serializer.finish()
}

/// Error type for fallible LaTeX serialization.
///
/// The current serializer is infallible; this type exists so the
/// public `Document::to_latex*` API can stay stable if serialization later
/// grows validation or IO-free failure modes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SerializeError {
    /// Reserved for future fallible serialization paths.
    Unsupported,
}

impl std::fmt::Display for SerializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializeError::Unsupported => f.write_str("unsupported serialization operation"),
        }
    }
}

impl std::error::Error for SerializeError {}

/// Serialized LaTeX and the typed fragments recorded during the same serialization pass.
///
/// Fragment categories and boundaries are part of the stable `texform` facade
/// contract. Fragments are ordered and non-empty; gaps contain only inserted
/// separator whitespace, while semantic whitespace remains inside its fragment.
///
/// - Wrapper delimiters are separate fragments from scalar
///   [`FragmentKind::Raw`] content.
/// - An escaped character is one [`FragmentKind::Character`] fragment,
///   including its escape sequence.
/// - An empty error snippet emits no fragment. The document still contains an
///   error node, so its `has_errors()` remains true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FragmentedLatex {
    /// Serialized LaTeX string, identical to [`Document::to_latex`].
    pub latex: String,
    /// Non-empty fragments covering `latex`, in order. Empty error snippets emit no fragment.
    pub fragments: Vec<SerializationFragment>,
}

/// A non-empty serialization fragment with its UTF-8 byte span and syntactic provenance.
///
/// `span` indexes the returned [`FragmentedLatex::latex`] string and selects
/// exactly `text`. Spans do not overlap. `mode` records the semantic math/text
/// mode, independently of formatting whitespace policy. `role` identifies the
/// syntactic role of the emitted fragment, `node_path` always resolves to the
/// emitting AST node via [`crate::document::Document::node_at`], and `slot`
/// identifies the parent-owned [`NodeSlot`] when the fragment belongs to a
/// specific slot on `node_path` (or `None` when emitted directly by the node
/// itself).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerializationFragment {
    /// Fragment text, exactly the slice `latex[span]`.
    pub text: String,
    /// UTF-8 byte range into [`FragmentedLatex::latex`]; spans do not overlap.
    pub span: Range<usize>,
    /// Semantic category of the fragment.
    pub kind: FragmentKind,
    /// Syntactic role of the fragment within the emitting AST construct.
    pub role: FragmentRole,
    /// Root-relative path of the emitting AST node, always valid for `Document::node_at`.
    pub node_path: String,
    /// Immediate slot on `node_path` that this fragment belongs to, or `None` for node-owned fragments.
    pub slot: Option<NodeSlot>,
    /// Semantic math/text mode, independent of spacing policy.
    pub mode: ContentMode,
}

/// Stable semantic categories for serialization fragments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentKind {
    /// A control sequence such as `\frac` or `\,`, including the leading backslash.
    ControlSequence,
    /// A single character atom, including script markers such as `_` and `^`.
    Character,
    /// A structural delimiter such as `{`, `}`, `[`, or `]`.
    Delimiter,
    /// A verbatim text-mode chunk that the serializer does not split or respace.
    Text,
    /// A source fragment emitted without further classification (dimension, environment name, ...).
    Raw,
    /// A recovered error snippet. Empty snippets emit no fragment.
    Error,
}

/// Syntactic roles for serialization fragments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentRole {
    /// Construct head control sequence (`\frac`, `\bf`, `\over`, `\begin`, `\end`).
    ///
    /// An environment node emits two `Head` fragments (`\begin` and `\end`), each
    /// followed by its own `Open` / `Name` / `Close` group (`slot: None`),
    /// whereas `\left` and `\right` emit `Open` and `Close`.
    Head,
    /// Opening boundary of any paired construct (`{`, `$`, `\left`, `[`, paired/delimited/protective/script/infix opener).
    ///
    /// Protective braces share the same `(node_path, slot)` as the slot's outer
    /// delimiters (for example, `\sqrt[{a[b]}]{x}` emits both `[` and `{` as
    /// `Open` on `arg.0`), so consumers pairing delimiters by `(node_path, slot)`
    /// should use a stack.
    Open,
    /// Closing boundary paired with `Open` (`}`, `$`, `\right`, `]`, paired/delimited/until/protective/script/infix closer).
    ///
    /// Protective braces share the same `(node_path, slot)` as the slot's outer
    /// delimiters, so consumers pairing delimiters by `(node_path, slot)` should
    /// use a stack.
    Close,
    /// Environment name text inside `\begin{...}` or `\end{...}`.
    Name,
    /// Script operator (`_` or `^`).
    Operator,
    /// Delimiter value after `\left`/`\right` or in a delimiter argument slot (including `.`).
    Delimiter,
    /// Scalar argument value (dimension, column, keyval, csname, integer, or star `*`).
    Value,
    /// Leaf node itself (`Char`, `Text`, `Prime`, `ActiveSpace`, `AlignmentTab`, `Error`).
    Leaf,
}

impl FragmentRole {
    /// Stable snake_case identifier used by language bindings and JSON output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Open => "open",
            Self::Close => "close",
            Self::Name => "name",
            Self::Operator => "operator",
            Self::Delimiter => "delimiter",
            Self::Value => "value",
            Self::Leaf => "leaf",
        }
    }
}

/// Output style. Both styles are deterministic and text-idempotent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    /// Explicit token spacing, byte-stable across releases.
    #[default]
    Spaced,
    /// Lexically required spaces, and unbraced scripts for single tokens that
    /// take no arguments and switch no state (`x_i`, `x^\alpha`).
    /// Layout may improve across releases while preserving parsed structure.
    Compact,
}

/// Serialization options. Omitted fields use the spaced style; unknown keys fail.
/// Spaced output is byte-stable; compact output preserves parsed structure and
/// is idempotent, but its layout may improve across releases.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SerializeOptions {
    /// Output style; [`Style::Spaced`] when omitted.
    pub style: Style,
}

impl From<Style> for SerializeOptions {
    fn from(style: Style) -> Self {
        Self { style }
    }
}

/// Layout decisions resolved from a [`Style`]. Both styles write subscripts
/// first and brace infix operands only when required, so neither is a setting.
#[derive(Clone, Copy)]
pub(crate) struct StyleSettings {
    /// Insert optional math-mode spaces between atoms.
    optional_space: bool,
    /// Drop a script's braces when it is a single safe token
    /// (see `Serializer::single_script_token`).
    bare_scripts: bool,
}

impl Style {
    pub(crate) const fn settings(self) -> StyleSettings {
        let compact = matches!(self, Self::Compact);
        StyleSettings {
            optional_space: !compact,
            bare_scripts: compact,
        }
    }
}

/// Private atom classification used solely by [`AtomWriter`] to decide
/// inter-atom boundaries. This does not appear in the AST; the serializer
/// assigns a kind to each piece of text it emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomKind {
    /// `\frac`, `\alpha`, `\\`, `\,` — any control sequence body
    ControlSequence,
    /// Verbatim text-mode chunk (never split or spaced internally)
    TextChunk,
    /// Single math-mode character atom
    MathChar,
    /// ASCII digit in math mode
    MathDigit,
    /// Prime shorthand mark(s)
    Prime,
    /// `{`, `}`, `[`, `]` — structural delimiters
    Brace,
    /// Delimiter token after `\left` / `\right` or in argument pairs
    DelimiterToken,
    /// `_` or `^`
    ScriptMark,
    /// `$` for inline math boundaries
    Dollar,
    /// `~` (active character space)
    ActiveChar,
    /// Raw fragment (dimension, column spec, environment name, etc.) that must
    /// not be token-spaced
    RawFragment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BoundaryPolicy {
    #[default]
    Auto,
    SuppressOptionalSpace,
}

#[derive(Clone, Copy)]
struct EmissionModes {
    spacing: ContentMode,
    semantic: ContentMode,
}

impl EmissionModes {
    fn same(mode: ContentMode) -> Self {
        Self {
            spacing: mode,
            semantic: mode,
        }
    }

    fn new(spacing: ContentMode, semantic: ContentMode) -> Self {
        Self { spacing, semantic }
    }
}

trait Recorder {
    type Output;
    const ENABLED: bool;

    fn path_checkpoint(&self) -> usize;
    fn push_path_segment(&mut self, segment: &str);
    fn push_path_indexed(&mut self, prefix: &str, index: usize, suffix: &str);
    fn restore_path(&mut self, checkpoint: usize);

    fn record(
        &mut self,
        output: &str,
        span: Range<usize>,
        kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        mode: ContentMode,
    );

    fn finish(self, latex: String) -> Self::Output;
}

#[derive(Default)]
struct NoopRecorder;

impl Recorder for NoopRecorder {
    type Output = String;
    const ENABLED: bool = false;

    #[inline(always)]
    fn path_checkpoint(&self) -> usize {
        0
    }

    #[inline(always)]
    fn push_path_segment(&mut self, _segment: &str) {}

    #[inline(always)]
    fn push_path_indexed(&mut self, _prefix: &str, _index: usize, _suffix: &str) {}

    #[inline(always)]
    fn restore_path(&mut self, _checkpoint: usize) {}

    #[inline]
    fn record(
        &mut self,
        _output: &str,
        _span: Range<usize>,
        _kind: FragmentKind,
        _role: FragmentRole,
        _slot: Option<NodeSlot>,
        _mode: ContentMode,
    ) {
    }

    fn finish(self, latex: String) -> Self::Output {
        latex
    }
}

struct FragmentRecorder {
    fragments: Vec<SerializationFragment>,
    path: String,
}

impl FragmentRecorder {
    fn with_capacity(capacity: usize) -> Self {
        let mut path = String::with_capacity(64);
        path.push_str("root");
        Self {
            fragments: Vec::with_capacity(capacity),
            path,
        }
    }
}

impl Recorder for FragmentRecorder {
    type Output = FragmentedLatex;
    const ENABLED: bool = true;

    #[inline]
    fn path_checkpoint(&self) -> usize {
        self.path.len()
    }

    #[inline]
    fn push_path_segment(&mut self, segment: &str) {
        self.path.push('.');
        self.path.push_str(segment);
    }

    #[inline]
    fn push_path_indexed(&mut self, prefix: &str, index: usize, suffix: &str) {
        use std::fmt::Write as _;
        let _ = write!(self.path, ".{prefix}.{index}{suffix}");
    }

    #[inline]
    fn restore_path(&mut self, checkpoint: usize) {
        self.path.truncate(checkpoint);
    }

    fn record(
        &mut self,
        output: &str,
        span: Range<usize>,
        kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        mode: ContentMode,
    ) {
        self.fragments.push(SerializationFragment {
            text: output[span.clone()].to_string(),
            span,
            kind,
            role,
            node_path: self.path.clone(),
            slot,
            mode,
        });
    }

    fn finish(self, latex: String) -> Self::Output {
        FragmentedLatex {
            latex,
            fragments: self.fragments,
        }
    }
}

/// Accumulates output text and decides where to insert inter-atom spaces.
///
/// Most boundary rules live in the atom writer's central decision function,
/// making them testable in isolation without constructing a full AST. A few
/// helpers append a space directly; the control-word check reads that byte
/// back in constant time instead of trusting the previous atom kind alone.
/// The writer tracks only the *previous* atom kind — no look-ahead — so the
/// serializer must emit atoms in final output order. Empty emissions leave
/// that kind in place.
struct AtomWriter<R> {
    output: String,
    previous: Option<AtomKind>,
    unbraced_script: bool,
    recorder: R,
}

impl<R: Recorder> AtomWriter<R> {
    #[cfg(test)]
    #[inline(always)]
    fn new(recorder: R) -> Self {
        Self::with_capacity(recorder, 0)
    }

    #[inline(always)]
    fn with_capacity(recorder: R, capacity: usize) -> Self {
        Self {
            output: String::with_capacity(capacity),
            previous: None,
            unbraced_script: false,
            recorder,
        }
    }

    #[inline(always)]
    fn emit_control_seq(
        &mut self,
        mode: ContentMode,
        name: &str,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        options: &StyleSettings,
    ) {
        self.emit_backslash_prefixed(
            EmissionModes::same(mode),
            AtomKind::ControlSequence,
            FragmentKind::ControlSequence,
            role,
            slot,
            name,
            BoundaryPolicy::Auto,
            options,
        );
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn emit_backslash_prefixed(
        &mut self,
        modes: EmissionModes,
        spacing_kind: AtomKind,
        public_kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        suffix: &str,
        boundary: BoundaryPolicy,
        options: &StyleSettings,
    ) {
        if self.should_insert_space(modes.spacing, spacing_kind, r"\", boundary, options) {
            self.output.push(' ');
        }
        self.unbraced_script = false;
        let start = self.output.len();
        self.output.push('\\');
        self.output.push_str(suffix);
        let end = self.output.len();
        if R::ENABLED && start != end {
            self.recorder.record(
                &self.output,
                start..end,
                public_kind,
                role,
                slot,
                modes.semantic,
            );
        }
        self.previous = Some(spacing_kind);
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn emit(
        &mut self,
        mode: ContentMode,
        kind: AtomKind,
        public_kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        text: &str,
        options: &StyleSettings,
    ) {
        self.emit_with_boundary(
            mode,
            kind,
            public_kind,
            role,
            slot,
            text,
            BoundaryPolicy::Auto,
            options,
        );
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn emit_with_boundary(
        &mut self,
        mode: ContentMode,
        spacing_kind: AtomKind,
        public_kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        text: &str,
        boundary: BoundaryPolicy,
        options: &StyleSettings,
    ) {
        self.emit_with_semantic_mode(
            EmissionModes::same(mode),
            spacing_kind,
            public_kind,
            role,
            slot,
            text,
            boundary,
            options,
        );
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn emit_with_semantic_mode(
        &mut self,
        modes: EmissionModes,
        spacing_kind: AtomKind,
        public_kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        text: &str,
        boundary: BoundaryPolicy,
        options: &StyleSettings,
    ) {
        // Empty text is not a token. Updating `previous` here would drop an
        // open control word before the next real emission.
        if text.is_empty() {
            return;
        }
        if self.should_insert_space(modes.spacing, spacing_kind, text, boundary, options) {
            self.output.push(' ');
        }
        self.unbraced_script = false;
        let start = self.output.len();
        self.output.push_str(text);
        let end = self.output.len();
        if R::ENABLED && start != end {
            self.recorder.record(
                &self.output,
                start..end,
                public_kind,
                role,
                slot,
                modes.semantic,
            );
        }
        self.previous = Some(spacing_kind);
    }

    #[inline(always)]
    fn emit_attached(
        &mut self,
        mode: ContentMode,
        kind: AtomKind,
        public_kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        text: &str,
    ) {
        // Same empty-emission rule as `emit`: attached text that adds no bytes
        // must not replace the atom identity later checks still rely on.
        if text.is_empty() {
            return;
        }
        self.unbraced_script = false;
        let start = self.output.len();
        self.output.push_str(text);
        let end = self.output.len();
        if R::ENABLED && start != end {
            self.recorder
                .record(&self.output, start..end, public_kind, role, slot, mode);
        }
        self.previous = Some(kind);
    }

    /// Append `*` directly — star must glue to the preceding control sequence
    /// without any boundary space (`\operatorname*`, not `\operatorname *`).
    ///
    /// Restoring `previous` keeps the current command-to-structure formatting.
    /// The lexical check still sees the trailing `*`, so the control word is
    /// already closed and a following letter is not given a terminator space.
    fn emit_star_suffix(&mut self, mode: ContentMode, slot: Option<NodeSlot>) {
        let previous = self.previous;
        self.emit_attached(
            mode,
            AtomKind::MathChar,
            FragmentKind::Character,
            FragmentRole::Value,
            slot,
            "*",
        );
        self.previous = previous;
    }

    /// Central boundary-decision function.
    ///
    /// Returns `true` when a space should be inserted between the previous atom
    /// and the upcoming `next` atom. Rules are checked top-down; the first
    /// matching branch wins.
    ///
    /// The first rules are lexical separators. They are not optional math
    /// spacing: compact options and [`BoundaryPolicy::SuppressOptionalSpace`]
    /// cannot cancel them.
    ///
    /// - An unterminated control word followed by an ASCII letter needs a
    ///   space. Text mode has no later rule that could add it. A letter at the
    ///   end of ordinary text is not a command name, and a control symbol such
    ///   as `\,` is already terminated.
    /// - In math mode, a control sequence followed by a sibling `[` or `*`
    ///   keeps a space. See [`Self::needs_sibling_argument_separator`].
    #[inline(always)]
    fn should_insert_space(
        &self,
        mode: ContentMode,
        next: AtomKind,
        next_text: &str,
        boundary: BoundaryPolicy,
        options: &StyleSettings,
    ) -> bool {
        let Some(prev) = self.previous else {
            return false;
        };

        if self.needs_control_word_separator(next_text)
            || self.needs_sibling_argument_separator(mode, next, next_text)
        {
            return true;
        }

        if !options.optional_space {
            return self.unbraced_script
                && matches!(mode, ContentMode::Math)
                && next_text
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric);
        }

        if matches!(boundary, BoundaryPolicy::SuppressOptionalSpace) {
            return false;
        }

        // Text mode never injects extra spaces.
        if matches!(mode, ContentMode::Text) {
            return false;
        }

        // Math mode separates every atom pair except digit runs and `$`
        // delimiters, which bind tightly to their content (`$x$`). Prime marks
        // are emitted attached to their script base, so only the atom after a
        // prime run reaches this decision.
        match (prev, next) {
            (AtomKind::MathDigit, AtomKind::MathDigit) => false,
            (AtomKind::ControlSequence | AtomKind::Prime, _) => true,
            (AtomKind::Dollar, _) | (_, AtomKind::Dollar) => false,
            _ => true,
        }
    }

    /// `true` when `next_text` would extend an unterminated ASCII control word.
    ///
    /// The lexer treats `\` plus consecutive ASCII letters as one control word
    /// and `\` plus a single other character as a finished control symbol.
    /// Reading one trailing byte is enough: `*`, whitespace, `)`, digits, and
    /// any other non-letter mean the name is already closed. Direct space
    /// appends (empty-group padding) show up here
    /// without a second scan of the output.
    #[inline]
    fn needs_control_word_separator(&self, next_text: &str) -> bool {
        let continues = next_text
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
        continues && self.ends_unterminated_control_word()
    }

    /// A control-sequence atom always starts with `\`, so its last byte
    /// decides. Delimiter tokens (`\langle` after `\left`) and raw fragments
    /// (error snippets such as `\frac`) may also end in a control word, but
    /// they may equally be a plain character or ordinary letters (`(`, a
    /// dimension, an environment name). For those, scan the trailing letter
    /// run and require an odd backslash run before it: `\\b` is the control
    /// symbol `\\` followed by a letter, not a control word.
    #[inline]
    fn ends_unterminated_control_word(&self) -> bool {
        let bytes = self.output.as_bytes();
        match self.previous {
            Some(AtomKind::ControlSequence) => bytes.last().is_some_and(u8::is_ascii_alphabetic),
            Some(AtomKind::DelimiterToken | AtomKind::RawFragment) => {
                let name_start = bytes
                    .iter()
                    .rposition(|byte| !byte.is_ascii_alphabetic())
                    .map_or(0, |index| index + 1);
                let backslashes = bytes[..name_start]
                    .iter()
                    .rev()
                    .take_while(|&&byte| byte == b'\\')
                    .count();
                name_start < bytes.len() && backslashes % 2 == 1
            }
            _ => false,
        }
    }

    /// `true` when a sibling `[` or `*` would be read as an argument of the
    /// preceding control sequence.
    ///
    /// Argspecs such as `\\`'s `!s !o` refuse to skip spaces before the
    /// optional star and bracket, so the parser keeps `\\ [6pt]` (row
    /// content) and `\\ *` as siblings. Gluing them would reparse as a row
    /// spacing argument or a starred form. The serializer has no knowledge
    /// base, so it keeps the space after every control sequence, including
    /// unknown ones and `\\*` (the star suffix restores the control-sequence
    /// identity). Argument-owned openers are emitted as [`AtomKind::Brace`] or
    /// [`AtomKind::DelimiterToken`] (`\sqrt[3]`, `\big[`), and a star argument
    /// is attached without a boundary decision, so they stay unaffected.
    ///
    /// Text mode is exempt: math mode drops the source space from the tree,
    /// while text content keeps it in the following text chunk (`\\ [x]`).
    #[inline]
    fn needs_sibling_argument_separator(
        &self,
        mode: ContentMode,
        next: AtomKind,
        next_text: &str,
    ) -> bool {
        matches!(mode, ContentMode::Math)
            && matches!(self.previous, Some(AtomKind::ControlSequence))
            && !matches!(next, AtomKind::Brace | AtomKind::DelimiterToken)
            && matches!(next_text.as_bytes().first(), Some(b'[' | b'*'))
    }

    fn finish(self) -> R::Output {
        self.recorder.finish(self.output)
    }
}

/// Recursive AST walker that emits atoms into an [`AtomWriter`].
///
/// Mode is tracked through the recursion stack — each `visit` call receives
/// the content mode of its parent context, so no separate mutable mode stack
/// is needed.
struct Serializer<'a, R> {
    ast: &'a Ast,
    options: &'a StyleSettings,
    writer: AtomWriter<R>,
    protection: &'a RefCell<HashMap<NodeId, bool>>,
}

impl<'a, R: Recorder> Serializer<'a, R> {
    fn new(
        ast: &'a Ast,
        options: &'a StyleSettings,
        recorder: R,
        protection: &'a RefCell<HashMap<NodeId, bool>>,
    ) -> Self {
        // Spaced output averages ~3–4 bytes per AST node; pre-sizing avoids
        // repeated output-buffer reallocations for typical formulas.
        let initial_cap = ast.node_count().saturating_mul(4).max(32);
        Self {
            ast,
            options,
            writer: AtomWriter::with_capacity(recorder, initial_cap),
            protection,
        }
    }
}

impl<'a> Serializer<'a, NoopRecorder> {
    /// Preview serializer for a single argument subtree. Uses a small fixed
    /// buffer rather than scaling by the full AST node count.
    fn new_preview(
        ast: &'a Ast,
        options: &'a StyleSettings,
        protection: &'a RefCell<HashMap<NodeId, bool>>,
    ) -> Self {
        Self {
            ast,
            options,
            writer: AtomWriter::with_capacity(NoopRecorder, 32),
            protection,
        }
    }
}

impl<'a, R: Recorder> Serializer<'a, R> {
    #[inline(always)]
    fn push_child(&mut self, index: usize) -> usize {
        let checkpoint = self.writer.recorder.path_checkpoint();
        if R::ENABLED {
            self.writer.recorder.push_path_indexed("child", index, "");
        }
        checkpoint
    }

    #[inline(always)]
    fn push_slot(&mut self, slot: NodeSlot) -> usize {
        let checkpoint = self.writer.recorder.path_checkpoint();
        if R::ENABLED {
            match slot {
                NodeSlot::Child(index) => {
                    self.writer.recorder.push_path_indexed("child", index, "");
                }
                NodeSlot::Arg(index) => {
                    self.writer
                        .recorder
                        .push_path_indexed("arg", index, ".content");
                }
                NodeSlot::ScriptBase => self.writer.recorder.push_path_segment("base"),
                NodeSlot::Subscript => self.writer.recorder.push_path_segment("sub"),
                NodeSlot::Superscript => self.writer.recorder.push_path_segment("sup"),
                NodeSlot::InfixLeft => self.writer.recorder.push_path_segment("left"),
                NodeSlot::InfixRight => self.writer.recorder.push_path_segment("right"),
                NodeSlot::EnvBody => self.writer.recorder.push_path_segment("body"),
            }
        }
        checkpoint
    }

    #[inline(always)]
    fn restore_path(&mut self, checkpoint: usize) {
        if R::ENABLED {
            self.writer.recorder.restore_path(checkpoint);
        }
    }

    /// Emit the formula content without root-level braces.
    ///
    /// The top-level API serializes "formula content", not "a group node".
    /// Root braces are intentionally suppressed regardless of whether the
    /// root is Explicit or Implicit.
    fn serialize_root(&mut self) {
        let root = self.ast.root();
        let Node::Root { children, mode } = self.ast.node(root) else {
            unreachable!("root must be a root node")
        };

        for (index, &child) in children.iter().enumerate() {
            let checkpoint = self.push_child(index);
            self.visit(child, *mode);
            self.restore_path(checkpoint);
        }
    }

    fn visit(&mut self, id: NodeId, mode: ContentMode) {
        match self.ast.node(id) {
            Node::Root { .. } => unreachable!("root node must be handled by serialize_root"),
            Node::Environment {
                name, args, body, ..
            } => self.visit_environment(name, args, *body, mode),
            Node::Infix {
                name,
                args,
                left,
                right,
            } => self.visit_infix(name, args, *left, *right),
            Node::Declarative { name, args } => self.visit_declarative(name, args, mode),
            Node::Group {
                children,
                kind,
                mode: child_mode,
            } => self.visit_group(kind, *child_mode, children),
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => self.visit_scripted(*base, *subscript, *superscript),
            Node::Command { name, args, .. } => self.visit_command(name, args, mode),
            Node::Prime { count } => self.visit_prime(*count, mode),
            Node::Char(ch) => self.visit_char(*ch, mode),
            Node::Text(text) => self.writer.emit(
                mode,
                AtomKind::TextChunk,
                FragmentKind::Text,
                FragmentRole::Leaf,
                None,
                text,
                self.options,
            ),
            Node::ActiveSpace => self.writer.emit(
                mode,
                AtomKind::ActiveChar,
                FragmentKind::Character,
                FragmentRole::Leaf,
                None,
                "~",
                self.options,
            ),
            // Keeps the `MathChar` boundary behavior that `&` had as a `Char`.
            Node::AlignmentTab => self.writer.emit(
                mode,
                AtomKind::MathChar,
                FragmentKind::Character,
                FragmentRole::Leaf,
                None,
                "&",
                self.options,
            ),
            Node::Error { snippet, .. } => self.writer.emit(
                mode,
                AtomKind::RawFragment,
                FragmentKind::Error,
                FragmentRole::Leaf,
                None,
                snippet,
                self.options,
            ),
        }
    }

    /// Emit a group node.
    ///
    /// `Explicit` and `Implicit` are treated identically as brace groups — the
    /// distinction is parser/transform history and must not leak into the text.
    fn visit_group(&mut self, kind: &GroupKind, child_mode: ContentMode, children: &[NodeId]) {
        match kind {
            GroupKind::Explicit | GroupKind::Implicit => {
                self.emit_wrapped(child_mode, AtomKind::Brace, "{", "}", children);
            }
            GroupKind::Delimited { left, right } => {
                self.writer.emit(
                    ContentMode::Math,
                    AtomKind::ControlSequence,
                    FragmentKind::ControlSequence,
                    FragmentRole::Open,
                    None,
                    r"\left",
                    self.options,
                );
                self.emit_delimiter(left, ContentMode::Math, FragmentRole::Delimiter, None);
                for (index, &child) in children.iter().enumerate() {
                    let checkpoint = self.push_child(index);
                    self.visit(child, ContentMode::Math);
                    self.restore_path(checkpoint);
                }
                self.writer.emit(
                    ContentMode::Math,
                    AtomKind::ControlSequence,
                    FragmentKind::ControlSequence,
                    FragmentRole::Close,
                    None,
                    r"\right",
                    self.options,
                );
                self.emit_delimiter(right, ContentMode::Math, FragmentRole::Delimiter, None);
            }
            GroupKind::InlineMath => self.visit_inline_math(children),
        }
    }

    fn visit_command(&mut self, name: &str, args: &[Option<Argument>], mode: ContentMode) {
        self.writer
            .emit_control_seq(mode, name, FragmentRole::Head, None, self.options);

        let empty_genfrac_delimiters = is_genfrac_without_delimiters(name, args);
        for (index, slot) in args.iter().enumerate() {
            if empty_genfrac_delimiters && index < 2 {
                self.emit_empty_brace_argument(index, slot, mode);
            } else {
                self.visit_argument_slot(index, slot, mode);
            }
        }
    }

    /// Emit a mandatory slot as `{}`.
    ///
    /// Only `\genfrac` delimiter slots use this. Both `{}` and `.` parse to
    /// `Delimiter::None`, but they render differently: MathJax and the XeTeX
    /// branch of amsmath treat `.` as a delimiter, dropping the null
    /// delimiter space around the fraction, while `{}{}` renders exactly as
    /// `\atop`/`\above`/`\over`.
    fn emit_empty_brace_argument(
        &mut self,
        arg_index: usize,
        slot: &Option<Argument>,
        mode: ContentMode,
    ) {
        let opening_boundary = match slot {
            Some(arg) if arg.no_leading_space => BoundaryPolicy::SuppressOptionalSpace,
            _ => BoundaryPolicy::Auto,
        };
        let node_slot = Some(NodeSlot::Arg(arg_index));
        self.writer.emit_with_boundary(
            mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            node_slot,
            "{",
            opening_boundary,
            self.options,
        );
        self.writer.emit_with_boundary(
            mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            node_slot,
            "}",
            BoundaryPolicy::SuppressOptionalSpace,
            self.options,
        );
    }

    /// Emit an infix command in its original syntactic form.
    ///
    /// The serializer does not assume the infix has been desugared by a
    /// transform rule; an un-rewritten `\over` still round-trips correctly.
    fn visit_infix(&mut self, name: &str, args: &[ArgumentSlot], left: NodeId, right: NodeId) {
        self.emit_infix_operand(NodeSlot::InfixLeft, left);
        self.writer.emit_control_seq(
            ContentMode::Math,
            name,
            FragmentRole::Head,
            None,
            self.options,
        );
        for (index, slot) in args.iter().enumerate() {
            self.visit_argument_slot(index, slot, ContentMode::Math);
        }
        self.emit_infix_operand(NodeSlot::InfixRight, right);
    }

    /// Emit a declarative command with its explicit arguments.
    fn visit_declarative(&mut self, name: &str, args: &[ArgumentSlot], mode: ContentMode) {
        self.writer
            .emit_control_seq(mode, name, FragmentRole::Head, None, self.options);
        for (index, slot) in args.iter().enumerate() {
            self.visit_argument_slot(index, slot, mode);
        }
    }

    fn visit_environment(
        &mut self,
        name: &str,
        args: &[ArgumentSlot],
        body: NodeId,
        mode: ContentMode,
    ) {
        self.emit_environment_head(mode, r"\begin", name);
        for (index, slot) in args.iter().enumerate() {
            self.visit_argument_slot(index, slot, mode);
        }

        let body_checkpoint = self.push_slot(NodeSlot::EnvBody);
        match self.ast.node(body) {
            Node::Group {
                children,
                mode: body_mode,
                kind: GroupKind::Implicit,
            } => {
                for (index, &child) in children.iter().enumerate() {
                    let child_checkpoint = self.push_child(index);
                    self.visit(child, *body_mode);
                    self.restore_path(child_checkpoint);
                }
            }
            Node::Group {
                mode: body_mode, ..
            } => self.visit(body, *body_mode),
            other => unreachable!("environment body must remain a group, got {:?}", other),
        }
        self.restore_path(body_checkpoint);

        self.emit_environment_head(mode, r"\end", name);
    }

    /// Emit `\begin {name}` or `\end {name}` (or compact `\begin{name}`).
    fn emit_environment_head(&mut self, outer_mode: ContentMode, head: &str, name: &str) {
        self.writer.emit(
            outer_mode,
            AtomKind::ControlSequence,
            FragmentKind::ControlSequence,
            FragmentRole::Head,
            None,
            head,
            self.options,
        );

        self.writer.emit_with_semantic_mode(
            EmissionModes::new(ContentMode::Math, outer_mode),
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            None,
            "{",
            BoundaryPolicy::Auto,
            self.options,
        );
        self.writer.emit_with_boundary(
            outer_mode,
            AtomKind::RawFragment,
            FragmentKind::Raw,
            FragmentRole::Name,
            None,
            name,
            BoundaryPolicy::SuppressOptionalSpace,
            self.options,
        );
        self.writer.emit_with_boundary(
            outer_mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            None,
            "}",
            BoundaryPolicy::SuppressOptionalSpace,
            self.options,
        );
    }

    /// Dispatch a single argument slot to the appropriate emitter.
    ///
    /// Content arguments (`MathContent` / `TextContent`) recurse into the
    /// serializer; scalar arguments are emitted as opaque fragments that
    /// bypass math-mode token spacing.
    fn visit_argument_slot(
        &mut self,
        arg_index: usize,
        slot: &Option<Argument>,
        mode: ContentMode,
    ) {
        let Some(arg) = slot else {
            return;
        };
        let opening_boundary = if arg.no_leading_space {
            BoundaryPolicy::SuppressOptionalSpace
        } else {
            BoundaryPolicy::Auto
        };

        match (&arg.kind, &arg.value) {
            (ArgumentKind::Star, ArgumentValue::Boolean(true)) => self
                .writer
                .emit_star_suffix(mode, Some(NodeSlot::Arg(arg_index))),
            (ArgumentKind::Star, ArgumentValue::Boolean(false)) => {}
            (ArgumentKind::Star, _) => {
                unreachable!("star slots must carry boolean values")
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, ArgumentValue::MathContent(child)) => {
                self.emit_argument_content(
                    arg_index,
                    *child,
                    ContentMode::Math,
                    "{",
                    "}",
                    mode,
                    opening_boundary,
                );
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, ArgumentValue::TextContent(child)) => {
                self.emit_argument_content(
                    arg_index,
                    *child,
                    ContentMode::Text,
                    "{",
                    "}",
                    mode,
                    opening_boundary,
                );
            }
            (
                ArgumentKind::Mandatory | ArgumentKind::Group,
                ArgumentValue::OperatorNameContent(child),
            ) => {
                self.emit_operator_name_argument_content(
                    arg_index,
                    *child,
                    "{",
                    "}",
                    mode,
                    opening_boundary,
                );
            }
            (ArgumentKind::Optional, ArgumentValue::MathContent(child)) => {
                self.emit_argument_content(
                    arg_index,
                    *child,
                    ContentMode::Math,
                    "[",
                    "]",
                    mode,
                    opening_boundary,
                );
            }
            (ArgumentKind::Optional, ArgumentValue::TextContent(child)) => {
                self.emit_argument_content(
                    arg_index,
                    *child,
                    ContentMode::Text,
                    "[",
                    "]",
                    mode,
                    opening_boundary,
                );
            }
            (ArgumentKind::Optional, ArgumentValue::OperatorNameContent(child)) => {
                self.emit_operator_name_argument_content(
                    arg_index,
                    *child,
                    "[",
                    "]",
                    mode,
                    opening_boundary,
                );
            }
            // A mandatory delimiter slot consumes a bare delimiter token in TeX
            // (`\middle\vert`, `\big(`, `\atopwithdelims()`). MathJax rejects a
            // braced delimiter for `\middle`, `\left`/`\right`, and the
            // `\...withdelims` family, so only the Group form (which required
            // braces in the source) keeps its braces.
            (ArgumentKind::Mandatory, ArgumentValue::Delimiter(delimiter)) => self
                .emit_delimiter_with_boundary(
                    delimiter,
                    mode,
                    mode,
                    FragmentRole::Delimiter,
                    Some(NodeSlot::Arg(arg_index)),
                    opening_boundary,
                ),
            (ArgumentKind::Mandatory | ArgumentKind::Group, value) => {
                self.emit_scalar_wrapped(arg_index, value, "{", "}", mode, opening_boundary)
            }
            (ArgumentKind::Optional, value) => {
                self.emit_scalar_wrapped(arg_index, value, "[", "]", mode, opening_boundary)
            }
            (ArgumentKind::Until { close }, value) => {
                self.emit_until_argument(arg_index, close, value, mode, opening_boundary)
            }
            (ArgumentKind::Delimited { open, close }, ArgumentValue::MathContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::MathContent(node)) => self
                .emit_recorded_delimiters(
                    arg_index,
                    open,
                    close,
                    *node,
                    ContentMode::Math,
                    mode,
                    opening_boundary,
                ),
            (ArgumentKind::Delimited { open, close }, ArgumentValue::TextContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::TextContent(node)) => self
                .emit_recorded_delimiters(
                    arg_index,
                    open,
                    close,
                    *node,
                    ContentMode::Text,
                    mode,
                    opening_boundary,
                ),
            (ArgumentKind::Delimited { open, close }, ArgumentValue::OperatorNameContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::OperatorNameContent(node)) => {
                self.emit_operator_name_between_delimiters(
                    arg_index,
                    open,
                    close,
                    *node,
                    mode,
                    opening_boundary,
                )
            }
            (ArgumentKind::Delimited { open, close }, value)
            | (ArgumentKind::Paired { open, close }, value) => self.emit_scalar_between_delimiters(
                arg_index,
                open,
                close,
                value,
                mode,
                opening_boundary,
            ),
        }
    }

    /// Inspect the exact serialized content, including delimiters emitted by
    /// nested commands. Ordinary braced arguments never need this extra pass.
    fn argument_needs_protection(
        &self,
        child: NodeId,
        mode: ContentMode,
        open: Option<Token<'_>>,
        close: Option<Token<'_>>,
        operator_name: bool,
    ) -> bool {
        if open == Some(Token::LBrace) {
            return false;
        }
        let Some(close) = close else {
            return false;
        };
        // Each node has one parent slot. Share its result with nested previews
        // so deeply nested optional arguments do not double the work per level.
        if let Some(&protected) = self.protection.borrow().get(&child) {
            return protected;
        }
        let mut preview = Serializer::new_preview(self.ast, self.options, self.protection);
        if operator_name {
            preview.visit_operator_name_content_node(child);
        } else {
            preview.visit_argument_content_node(child, mode);
        }
        let text = preview.finish();
        let protected = ContentBoundary::new(open, close)
            .needs_protection_iter(Token::lexer(&text).filter_map(Result::ok));
        self.protection.borrow_mut().insert(child, protected);
        protected
    }

    /// Emit a content argument wrapped in its matching delimiters.
    ///
    /// `content_mode` is the mode the argument was parsed in (from the
    /// `MathContent` / `TextContent` variant), while `wrapper_mode` controls
    /// boundary spacing around the outer delimiters.
    #[allow(clippy::too_many_arguments)]
    fn emit_argument_content(
        &mut self,
        arg_index: usize,
        child: NodeId,
        content_mode: ContentMode,
        open: &str,
        close: &str,
        wrapper_mode: ContentMode,
        opening_boundary: BoundaryPolicy,
    ) {
        let slot = NodeSlot::Arg(arg_index);
        if open == "["
            && self.argument_needs_protection(
                child,
                content_mode,
                None,
                Token::lexer(close).next().and_then(Result::ok),
                false,
            )
        {
            self.writer.emit_with_boundary(
                wrapper_mode,
                AtomKind::Brace,
                FragmentKind::Delimiter,
                FragmentRole::Open,
                Some(slot),
                open,
                opening_boundary,
                self.options,
            );
            self.emit_wrapped_content(
                slot,
                child,
                wrapper_mode,
                content_mode,
                "{",
                "}",
                BoundaryPolicy::Auto,
            );
            self.writer.emit(
                wrapper_mode,
                AtomKind::Brace,
                FragmentKind::Delimiter,
                FragmentRole::Close,
                Some(slot),
                close,
                self.options,
            );
            return;
        }
        self.emit_wrapped_content(
            slot,
            child,
            wrapper_mode,
            content_mode,
            open,
            close,
            opening_boundary,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_operator_name_argument_content(
        &mut self,
        arg_index: usize,
        child: NodeId,
        open: &str,
        close: &str,
        wrapper_mode: ContentMode,
        opening_boundary: BoundaryPolicy,
    ) {
        let slot = NodeSlot::Arg(arg_index);
        self.writer.emit_with_boundary(
            wrapper_mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            Some(slot),
            open,
            opening_boundary,
            self.options,
        );
        let protect = open == "["
            && self.argument_needs_protection(
                child,
                ContentMode::Text,
                None,
                Token::lexer(close).next().and_then(Result::ok),
                true,
            );
        if protect {
            self.emit_operator_name_argument_content(
                arg_index,
                child,
                "{",
                "}",
                wrapper_mode,
                BoundaryPolicy::Auto,
            );
        } else {
            let checkpoint = self.push_slot(slot);
            self.visit_operator_name_content_node(child);
            self.restore_path(checkpoint);
        }
        self.writer.emit_with_boundary(
            wrapper_mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            Some(slot),
            close,
            BoundaryPolicy::SuppressOptionalSpace,
            self.options,
        );
    }

    fn visit_scripted(
        &mut self,
        base: NodeId,
        subscript: Option<NodeId>,
        superscript: Option<NodeId>,
    ) {
        let base_checkpoint = self.push_slot(NodeSlot::ScriptBase);
        self.visit(base, ContentMode::Math);
        self.restore_path(base_checkpoint);
        if let Some(node) = subscript {
            self.emit_script("_", NodeSlot::Subscript, node);
        }
        if let Some(node) = superscript {
            self.emit_superscript(node);
        }
    }

    fn visit_inline_math(&mut self, children: &[NodeId]) {
        self.writer.emit(
            ContentMode::Text,
            AtomKind::Dollar,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            None,
            "$",
            self.options,
        );
        for (index, &child) in children.iter().enumerate() {
            let checkpoint = self.push_child(index);
            self.visit(child, ContentMode::Math);
            self.restore_path(checkpoint);
        }
        self.writer.emit(
            ContentMode::Text,
            AtomKind::Dollar,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            None,
            "$",
            self.options,
        );
    }

    /// Emit a script, omitting braces only for a single standalone token.
    fn emit_script(&mut self, marker: &str, slot: NodeSlot, node: NodeId) {
        self.writer.emit_with_boundary(
            ContentMode::Math,
            AtomKind::ScriptMark,
            FragmentKind::Character,
            FragmentRole::Operator,
            Some(slot),
            marker,
            BoundaryPolicy::Auto,
            self.options,
        );
        if self.options.bare_scripts
            && let Some((token, unwrapped_group)) = self.single_script_token(node)
        {
            let slot_checkpoint = self.push_slot(slot);
            let child_checkpoint = if unwrapped_group {
                Some(self.push_child(0))
            } else {
                None
            };
            self.visit(token, ContentMode::Math);
            if let Some(cp) = child_checkpoint {
                self.restore_path(cp);
            }
            self.restore_path(slot_checkpoint);
            self.writer.unbraced_script = true;
            return;
        }
        self.emit_wrapped_content(
            slot,
            node,
            ContentMode::Math,
            ContentMode::Math,
            "{",
            "}",
            BoundaryPolicy::Auto,
        );
    }

    /// Inspect the emitted shape after removing only the slot's own container.
    ///
    /// A bare script must reparse as exactly this token and keep its TeX
    /// meaning. A known command qualifies only when its matched argspec has no
    /// slots: an omitted optional, star, or delimited slot would capture the
    /// following sibling once the closing brace is gone (`x^{\sin}[2]`).
    /// Unknown commands may be argument-taking macros outside the knowledge
    /// base, and a declarative switch would extend its scope past the script
    /// (`x^{\rm} a`), so both keep their braces.
    fn single_script_token(&self, node: NodeId) -> Option<(NodeId, bool)> {
        let (node, unwrapped_group) = match self.ast.node(node) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                ..
            } if children.len() == 1 => (children[0], true),
            _ => (node, false),
        };
        match self.ast.node(node) {
            Node::Char(_) | Node::ActiveSpace | Node::Prime { count: 1 } => {
                Some((node, unwrapped_group))
            }
            Node::Command {
                args, known: true, ..
            } if args.is_empty() => Some((node, unwrapped_group)),
            _ => None,
        }
    }

    fn emit_superscript(&mut self, node: NodeId) {
        if let Node::Prime { count } = self.ast.node(node) {
            let checkpoint = self.push_slot(NodeSlot::Superscript);
            self.emit_prime_marks(*count);
            self.restore_path(checkpoint);
        } else {
            self.emit_script("^", NodeSlot::Superscript, node);
        }
    }

    /// Emit children surrounded by open/close delimiters.
    fn emit_wrapped(
        &mut self,
        mode: ContentMode,
        kind: AtomKind,
        open: &str,
        close: &str,
        children: &[NodeId],
    ) {
        // Empty math brace groups need special handling to produce `{ }`
        // instead of `{}` under spaced style — the normal visitor path would
        // emit `{` then immediately `}` with no content in between.
        if children.is_empty()
            && matches!(mode, ContentMode::Math)
            && matches!(kind, AtomKind::Brace)
            && self.options.optional_space
        {
            self.emit_padded_empty_group(mode, kind, open, close);
            return;
        }

        self.writer.emit(
            mode,
            kind,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            None,
            open,
            self.options,
        );
        for (index, &child) in children.iter().enumerate() {
            let checkpoint = self.push_child(index);
            self.visit(child, mode);
            self.restore_path(checkpoint);
        }
        self.writer.emit(
            mode,
            kind,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            None,
            close,
            self.options,
        );
    }

    /// Emit `{ }` as a single pre-formatted unit.
    ///
    /// Bypasses the normal atom pipeline because there is no interior content
    /// to visit, yet the padding space must still appear between the braces.
    /// That space is part of the emitted ending, so a later atom must not
    /// treat the open brace's predecessor as still unterminated.
    fn emit_padded_empty_group(
        &mut self,
        mode: ContentMode,
        kind: AtomKind,
        open: &str,
        close: &str,
    ) {
        self.writer.emit(
            mode,
            kind,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            None,
            open,
            self.options,
        );
        self.writer.output.push(' ');
        self.writer.emit_with_boundary(
            mode,
            kind,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            None,
            close,
            BoundaryPolicy::SuppressOptionalSpace,
            self.options,
        );
    }

    /// Emit a child node inside wrapper-owned delimiters (e.g. `{ ... }`).
    ///
    /// When the child is itself a brace group, its children are inlined
    /// directly to avoid double-bracing (`{ { a } }` → `{ a }`). This is
    /// safe because the wrapper already provides the grouping delimiter.
    #[allow(clippy::too_many_arguments)]
    fn emit_wrapped_content(
        &mut self,
        slot: NodeSlot,
        child: NodeId,
        wrapper_mode: ContentMode,
        content_mode: ContentMode,
        open: &str,
        close: &str,
        opening_boundary: BoundaryPolicy,
    ) {
        self.writer.emit_with_boundary(
            wrapper_mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            Some(slot),
            open,
            opening_boundary,
            self.options,
        );

        let slot_checkpoint = self.push_slot(slot);
        match self.ast.node(child) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                mode: child_mode,
            } => {
                if children.is_empty()
                    && matches!(*child_mode, ContentMode::Math)
                    && self.options.optional_space
                {
                    self.restore_path(slot_checkpoint);
                    // Same direct padding as `emit_padded_empty_group`: the space is
                    // the emitted ending, not a second lexical separator.
                    self.writer.output.push(' ');
                    self.writer.emit_with_boundary(
                        wrapper_mode,
                        AtomKind::Brace,
                        FragmentKind::Delimiter,
                        FragmentRole::Close,
                        Some(slot),
                        close,
                        BoundaryPolicy::SuppressOptionalSpace,
                        self.options,
                    );
                    return;
                }
                for (index, &grandchild) in children.iter().enumerate() {
                    let child_checkpoint = self.push_child(index);
                    self.visit(grandchild, *child_mode);
                    self.restore_path(child_checkpoint);
                }
            }
            _ => self.visit(child, content_mode),
        }
        self.restore_path(slot_checkpoint);

        self.writer.emit_with_semantic_mode(
            EmissionModes::new(content_mode, wrapper_mode),
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            Some(slot),
            close,
            BoundaryPolicy::Auto,
            self.options,
        );
    }

    fn emit_infix_operand(&mut self, slot: NodeSlot, node: NodeId) {
        if self.is_empty_infix_operand(node) {
            return;
        }

        if self.infix_operand_requires_braces(node) {
            self.emit_wrapped_content(
                slot,
                node,
                ContentMode::Math,
                ContentMode::Math,
                "{",
                "}",
                BoundaryPolicy::Auto,
            );
        } else {
            let checkpoint = self.push_slot(slot);
            self.emit_unwrapped_infix_operand(node);
            self.restore_path(checkpoint);
        }
    }

    fn emit_unwrapped_infix_operand(&mut self, node: NodeId) {
        match self.ast.node(node) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                mode,
            } => {
                for (index, &child) in children.iter().enumerate() {
                    let checkpoint = self.push_child(index);
                    self.visit(child, *mode);
                    self.restore_path(checkpoint);
                }
            }
            _ => self.visit(node, ContentMode::Math),
        }
    }

    fn is_empty_infix_operand(&self, node: NodeId) -> bool {
        matches!(
            self.ast.node(node),
            Node::Group {
                children,
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            } if children.is_empty()
        )
    }

    fn infix_operand_requires_braces(&self, node: NodeId) -> bool {
        match self.ast.node(node) {
            Node::Infix { .. } => true,
            Node::Group {
                kind: GroupKind::Explicit,
                ..
            } => true,
            Node::Group {
                children,
                kind: GroupKind::Implicit,
                ..
            } => children
                .iter()
                .any(|&child| matches!(self.ast.node(child), Node::Infix { .. })),
            _ => false,
        }
    }

    /// Emit a scalar argument value inside delimiters as a single opaque chunk.
    ///
    /// Scalars (dimensions, column specs, etc.) are written directly into the
    /// output buffer to prevent math-mode token spacing from corrupting them
    /// (e.g. `1pt` must not become `1 p t`).
    fn emit_scalar_wrapped(
        &mut self,
        arg_index: usize,
        value: &ArgumentValue,
        open: &str,
        close: &str,
        mode: ContentMode,
        opening_boundary: BoundaryPolicy,
    ) {
        let slot = Some(NodeSlot::Arg(arg_index));
        self.writer.emit_with_boundary(
            mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Open,
            slot,
            open,
            opening_boundary,
            self.options,
        );
        self.emit_scalar_argument_value(
            arg_index,
            value,
            mode,
            BoundaryPolicy::SuppressOptionalSpace,
        );
        self.writer.emit_with_boundary(
            mode,
            AtomKind::Brace,
            FragmentKind::Delimiter,
            FragmentRole::Close,
            slot,
            close,
            BoundaryPolicy::SuppressOptionalSpace,
            self.options,
        );
    }

    fn emit_until_argument(
        &mut self,
        arg_index: usize,
        close: &Delimiter,
        value: &ArgumentValue,
        mode: ContentMode,
        opening_boundary: BoundaryPolicy,
    ) {
        let slot = NodeSlot::Arg(arg_index);
        if let Some((child, content_mode)) = value.content() {
            self.writer.emit_with_boundary(
                mode,
                AtomKind::Brace,
                FragmentKind::Delimiter,
                FragmentRole::Open,
                Some(slot),
                "{",
                opening_boundary,
                self.options,
            );
            // A TeX delimited parameter strips exactly this protective brace layer.
            // Explicit content groups belong to the argument and must survive.
            let content_mode = if matches!(value, ArgumentValue::OperatorNameContent(_)) {
                ContentMode::Text
            } else {
                content_mode
            };

            let slot_checkpoint = self.push_slot(slot);
            match self.ast.node(child) {
                Node::Group {
                    children,
                    kind: GroupKind::Implicit,
                    ..
                } => {
                    for (index, &child) in children.iter().enumerate() {
                        let child_checkpoint = self.push_child(index);
                        self.visit(child, content_mode);
                        self.restore_path(child_checkpoint);
                    }
                }
                _ => self.visit(child, content_mode),
            }
            self.restore_path(slot_checkpoint);
            self.writer.emit_with_semantic_mode(
                EmissionModes::new(content_mode, mode),
                AtomKind::Brace,
                FragmentKind::Delimiter,
                FragmentRole::Close,
                Some(slot),
                "}",
                BoundaryPolicy::Auto,
                self.options,
            );
        } else {
            self.emit_scalar_wrapped(arg_index, value, "{", "}", mode, opening_boundary);
        }
        self.emit_delimiter_with_boundary(
            close,
            mode,
            mode,
            FragmentRole::Close,
            Some(slot),
            BoundaryPolicy::SuppressOptionalSpace,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_recorded_delimiters(
        &mut self,
        arg_index: usize,
        open: &Delimiter,
        close: &Delimiter,
        node: NodeId,
        content_mode: ContentMode,
        wrapper_mode: ContentMode,
        opening_boundary: BoundaryPolicy,
    ) {
        let slot = NodeSlot::Arg(arg_index);
        self.emit_delimiter_with_boundary(
            open,
            content_mode,
            wrapper_mode,
            FragmentRole::Open,
            Some(slot),
            opening_boundary,
        );
        if self.argument_needs_protection(
            node,
            content_mode,
            delimiter_token(open),
            delimiter_token(close),
            false,
        ) {
            self.emit_wrapped_content(
                slot,
                node,
                wrapper_mode,
                content_mode,
                "{",
                "}",
                BoundaryPolicy::Auto,
            );
        } else {
            let checkpoint = self.push_slot(slot);
            self.visit_argument_content_node(node, content_mode);
            self.restore_path(checkpoint);
        }
        self.emit_delimiter_with_boundary(
            close,
            content_mode,
            wrapper_mode,
            FragmentRole::Close,
            Some(slot),
            BoundaryPolicy::Auto,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_operator_name_between_delimiters(
        &mut self,
        arg_index: usize,
        open: &Delimiter,
        close: &Delimiter,
        node: NodeId,
        mode: ContentMode,
        opening_boundary: BoundaryPolicy,
    ) {
        let slot = NodeSlot::Arg(arg_index);
        self.emit_delimiter_with_boundary(
            open,
            mode,
            mode,
            FragmentRole::Open,
            Some(slot),
            opening_boundary,
        );
        if self.argument_needs_protection(
            node,
            ContentMode::Text,
            delimiter_token(open),
            delimiter_token(close),
            true,
        ) {
            self.emit_operator_name_argument_content(
                arg_index,
                node,
                "{",
                "}",
                mode,
                BoundaryPolicy::Auto,
            );
        } else {
            let checkpoint = self.push_slot(slot);
            self.visit_operator_name_content_node(node);
            self.restore_path(checkpoint);
        }
        self.emit_delimiter_with_boundary(
            close,
            mode,
            mode,
            FragmentRole::Close,
            Some(slot),
            BoundaryPolicy::SuppressOptionalSpace,
        );
    }

    fn visit_operator_name_content_node(&mut self, node: NodeId) {
        match self.ast.node(node) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                ..
            } => {
                for (index, &child) in children.iter().enumerate() {
                    let checkpoint = self.push_child(index);
                    self.visit(child, ContentMode::Text);
                    self.restore_path(checkpoint);
                }
            }
            _ => self.visit(node, ContentMode::Text),
        }
    }

    fn emit_scalar_between_delimiters(
        &mut self,
        arg_index: usize,
        open: &Delimiter,
        close: &Delimiter,
        value: &ArgumentValue,
        mode: ContentMode,
        opening_boundary: BoundaryPolicy,
    ) {
        let slot = Some(NodeSlot::Arg(arg_index));
        self.emit_delimiter_with_boundary(
            open,
            mode,
            mode,
            FragmentRole::Open,
            slot,
            opening_boundary,
        );
        self.emit_scalar_argument_value(arg_index, value, mode, BoundaryPolicy::Auto);
        self.emit_delimiter(close, mode, FragmentRole::Close, slot);
    }

    fn emit_scalar_argument_value(
        &mut self,
        arg_index: usize,
        value: &ArgumentValue,
        mode: ContentMode,
        boundary: BoundaryPolicy,
    ) {
        let slot = Some(NodeSlot::Arg(arg_index));
        match value {
            ArgumentValue::Delimiter(delimiter) => self.emit_delimiter_atom(
                delimiter,
                EmissionModes::same(mode),
                AtomKind::RawFragment,
                FragmentKind::Raw,
                FragmentRole::Delimiter,
                slot,
                boundary,
            ),
            ArgumentValue::CSName(name)
            | ArgumentValue::ControlSequence(name)
            | ArgumentValue::Dimension(name)
            | ArgumentValue::Integer(name)
            | ArgumentValue::Real(name)
            | ArgumentValue::KeyVal(name)
            | ArgumentValue::Column(name) => self.writer.emit_with_boundary(
                mode,
                AtomKind::RawFragment,
                FragmentKind::Raw,
                FragmentRole::Value,
                slot,
                name,
                boundary,
                self.options,
            ),
            ArgumentValue::Boolean(_) => {
                unreachable!("boolean values are only valid in star slots")
            }
            ArgumentValue::MathContent(_)
            | ArgumentValue::TextContent(_)
            | ArgumentValue::OperatorNameContent(_) => {
                unreachable!("content variants must be serialized as child nodes")
            }
        }
    }

    fn emit_delimiter(
        &mut self,
        delimiter: &Delimiter,
        mode: ContentMode,
        role: FragmentRole,
        slot: Option<NodeSlot>,
    ) {
        self.emit_delimiter_with_boundary(delimiter, mode, mode, role, slot, BoundaryPolicy::Auto);
    }

    fn emit_delimiter_with_boundary(
        &mut self,
        delimiter: &Delimiter,
        spacing_mode: ContentMode,
        semantic_mode: ContentMode,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        boundary: BoundaryPolicy,
    ) {
        self.emit_delimiter_atom(
            delimiter,
            EmissionModes::new(spacing_mode, semantic_mode),
            AtomKind::DelimiterToken,
            FragmentKind::Delimiter,
            role,
            slot,
            boundary,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_delimiter_atom(
        &mut self,
        delimiter: &Delimiter,
        modes: EmissionModes,
        spacing_kind: AtomKind,
        public_kind: FragmentKind,
        role: FragmentRole,
        slot: Option<NodeSlot>,
        boundary: BoundaryPolicy,
    ) {
        match delimiter {
            Delimiter::None => self.writer.emit_with_semantic_mode(
                modes,
                spacing_kind,
                public_kind,
                role,
                slot,
                ".",
                boundary,
                self.options,
            ),
            Delimiter::Char(ch) => {
                let mut buf = [0u8; 4];
                self.writer.emit_with_semantic_mode(
                    modes,
                    spacing_kind,
                    public_kind,
                    role,
                    slot,
                    ch.encode_utf8(&mut buf),
                    boundary,
                    self.options,
                );
            }
            Delimiter::Control(name) => self.writer.emit_backslash_prefixed(
                modes,
                spacing_kind,
                public_kind,
                role,
                slot,
                name,
                boundary,
                self.options,
            ),
        }
    }

    /// Visit content inside a `Delimited` / `Paired` argument, unwrapping
    /// any top-level brace group to avoid redundant nesting.
    fn visit_argument_content_node(&mut self, node: NodeId, mode: ContentMode) {
        match self.ast.node(node) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                mode: child_mode,
            } => {
                for (index, &child) in children.iter().enumerate() {
                    let checkpoint = self.push_child(index);
                    self.visit(child, *child_mode);
                    self.restore_path(checkpoint);
                }
            }
            _ => self.visit(node, mode),
        }
    }

    /// Emit a `Char` node — classified by mode and digit status
    /// depending on the surrounding mode so boundary rules apply correctly.
    fn visit_char(&mut self, ch: char, mode: ContentMode) {
        let kind = if matches!(mode, ContentMode::Text) {
            AtomKind::TextChunk
        } else if ch.is_ascii_digit() {
            AtomKind::MathDigit
        } else {
            AtomKind::MathChar
        };
        // `Char('&')` is always a literal ampersand; alignment tabs are
        // `Node::AlignmentTab`, so both modes escape the same set.
        let mut buf = [0u8; 4];
        let text = match ch {
            '%' => r"\%",
            '$' => r"\$",
            '&' => r"\&",
            '#' => r"\#",
            '_' => r"\_",
            '{' => r"\{",
            '}' => r"\}",
            _ => ch.encode_utf8(&mut buf),
        };
        self.writer.emit(
            mode,
            kind,
            FragmentKind::Character,
            FragmentRole::Leaf,
            None,
            text,
            self.options,
        );
    }

    fn visit_prime(&mut self, count: usize, mode: ContentMode) {
        // Operator-name content uses text spacing even for math-only symbols.
        for _ in 0..count {
            self.writer.emit_with_semantic_mode(
                EmissionModes::new(mode, ContentMode::Math),
                AtomKind::ControlSequence,
                FragmentKind::ControlSequence,
                FragmentRole::Leaf,
                None,
                r"\prime",
                BoundaryPolicy::Auto,
                self.options,
            );
        }
    }

    fn emit_prime_marks(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        let owned;
        let marks = match count {
            1 => "'",
            2 => "''",
            3 => "'''",
            4 => "''''",
            _ => {
                owned = "'".repeat(count);
                &owned
            }
        };
        self.writer.emit_attached(
            ContentMode::Math,
            AtomKind::Prime,
            FragmentKind::Character,
            FragmentRole::Leaf,
            None,
            marks,
        );
    }

    fn finish(self) -> R::Output {
        self.writer.finish()
    }
}

/// Convert a serialized delimiter into its leading lexer token without
/// allocating an intermediate string.
fn delimiter_token(delimiter: &Delimiter) -> Option<Token<'_>> {
    match delimiter {
        Delimiter::None => Some(Token::Char('.')),
        Delimiter::Char(ch) => {
            let mut buf = [0u8; 4];
            let token = Token::lexer(ch.encode_utf8(&mut buf)).next()?.ok()?;
            Some(match token {
                // A single-character input only lexes as `ControlSeq` for `\`,
                // which yields the static `" "` slice.
                Token::ControlSeq(_) => Token::ControlSeq(" "),
                Token::ActiveChar => Token::ActiveChar,
                Token::LBrace => Token::LBrace,
                Token::RBrace => Token::RBrace,
                Token::MathShift => Token::MathShift,
                Token::Alignment => Token::Alignment,
                Token::Parameter => Token::Parameter,
                Token::Superscript => Token::Superscript,
                Token::Subscript => Token::Subscript,
                Token::Star => Token::Star,
                Token::LBracket => Token::LBracket,
                Token::RBracket => Token::RBracket,
                Token::Prime(n) => Token::Prime(n),
                Token::Whitespaces => Token::Whitespaces,
                Token::Comment => Token::Comment,
                Token::Char(c) => Token::Char(c),
            })
        }
        Delimiter::Control(name) => Some(Token::ControlSeq(control_seq_token_name(name))),
    }
}

fn control_seq_token_name(name: &str) -> &str {
    let bytes = name.as_bytes();
    let Some(&first) = bytes.first() else {
        return " ";
    };
    if first.is_ascii_alphabetic() {
        let len = bytes.iter().take_while(|b| b.is_ascii_alphabetic()).count();
        &name[..len]
    } else if matches!(first, b'\r' | b'\n') {
        " "
    } else {
        let ch = name
            .chars()
            .next()
            .expect("non-empty string has first char");
        &name[..ch.len_utf8()]
    }
}

/// Whether a `\genfrac` command has no delimiter on either side.
///
/// Only the two-sided case is written as `{}{}`. With a single delimiter,
/// pdfTeX's amsmath requires `.` for the missing side (`\genfrac({}` raises
/// "Missing delimiter"), and MathJax renders `.` and `{}` identically there.
fn is_genfrac_without_delimiters(name: &str, args: &[Option<Argument>]) -> bool {
    name == "genfrac"
        && args.len() >= 2
        && args[..2].iter().all(|slot| {
            matches!(
                slot,
                Some(Argument {
                    kind: ArgumentKind::Mandatory,
                    value: ArgumentValue::Delimiter(Delimiter::None),
                    ..
                })
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_error_node_as_snippet() {
        use crate::ast::{Ast, Node};

        let mut ast = Ast::new();
        let error = ast.new_node(Node::Error {
            message: "unexpected".to_string(),
            snippet: r"\bad{".to_string(),
        });
        ast.append_child(ast.root(), error);

        assert_eq!(serialize(&ast), r"\bad{");
    }

    #[test]
    fn test_atom_writer_glues_star_to_control_sequence() {
        let options = Style::Spaced.settings();
        let mut writer = AtomWriter::new(NoopRecorder);

        writer.emit(
            ContentMode::Math,
            AtomKind::ControlSequence,
            FragmentKind::ControlSequence,
            FragmentRole::Head,
            None,
            r"\operatorname",
            &options,
        );
        writer.emit_star_suffix(ContentMode::Math, Some(NodeSlot::Arg(0)));

        assert_eq!(writer.finish(), r"\operatorname*");
    }

    #[test]
    fn lexical_separator_overrides_suppressed_optional_space() {
        let options = Style::Spaced.settings();
        let mut writer = AtomWriter::new(NoopRecorder);

        writer.emit(
            ContentMode::Math,
            AtomKind::ControlSequence,
            FragmentKind::ControlSequence,
            FragmentRole::Head,
            None,
            r"\alpha",
            &options,
        );
        writer.emit_with_boundary(
            ContentMode::Math,
            AtomKind::MathChar,
            FragmentKind::Character,
            FragmentRole::Leaf,
            None,
            "x",
            BoundaryPolicy::SuppressOptionalSpace,
            &options,
        );

        assert_eq!(writer.finish(), r"\alpha x");
    }

    #[test]
    fn test_atom_writer_keeps_text_chunk_compact() {
        let options = Style::Spaced.settings();
        let mut writer = AtomWriter::new(NoopRecorder);

        writer.emit(
            ContentMode::Text,
            AtomKind::TextChunk,
            FragmentKind::Text,
            FragmentRole::Leaf,
            None,
            "abc",
            &options,
        );
        writer.emit(
            ContentMode::Text,
            AtomKind::TextChunk,
            FragmentKind::Text,
            FragmentRole::Leaf,
            None,
            " def",
            &options,
        );

        assert_eq!(writer.finish(), "abc def");
    }
    use AtomKind::{
        Brace, ControlSequence, DelimiterToken, MathChar, MathDigit, Prime, RawFragment, TextChunk,
    };
    use ContentMode::{Math, Text};

    /// Ordinary atom: the writer decides the boundary.
    const EMIT: Option<BoundaryPolicy> = Some(BoundaryPolicy::Auto);
    /// Atom whose optional math spacing is suppressed by the caller.
    const SUPPRESS: Option<BoundaryPolicy> = Some(BoundaryPolicy::SuppressOptionalSpace);
    /// Atom appended with no boundary decision at all (star suffix, prime run).
    const ATTACH: Option<BoundaryPolicy> = None;

    /// Write `(mode, kind, boundary, text)` atoms with default options.
    ///
    /// Boundary decisions never read the public token kind, role, or slot, so
    /// they are pinned here and the cases vary only mode, atom kind, boundary,
    /// and text.
    fn write_atoms(atoms: &[(ContentMode, AtomKind, Option<BoundaryPolicy>, &str)]) -> String {
        let options = Style::Spaced.settings();
        let public = FragmentKind::Character;
        let role = FragmentRole::Leaf;
        let mut writer = AtomWriter::new(NoopRecorder);
        for (mode, kind, boundary, text) in atoms {
            match boundary {
                Some(boundary) => {
                    writer.emit_with_boundary(
                        *mode, *kind, public, role, None, text, *boundary, &options,
                    );
                }
                None => writer.emit_attached(*mode, *kind, public, role, None, text),
            }
        }
        writer.finish()
    }

    #[test]
    fn ordinary_trailing_letters_are_not_control_words() {
        // Only a control sequence can absorb the next letter. A letter that
        // merely ends a text chunk must not gain a separator.
        assert_eq!(
            write_atoms(&[
                (Text, TextChunk, EMIT, "pre"),
                (Text, TextChunk, EMIT, "fix")
            ]),
            "prefix"
        );
    }

    #[test]
    fn unicode_letter_does_not_continue_an_ascii_control_word() {
        // Control-word names are ASCII only, so no lexical separator is due;
        // math mode still applies its own optional command spacing.
        assert_eq!(
            write_atoms(&[
                (Text, ControlSequence, EMIT, r"\alpha"),
                (Text, TextChunk, EMIT, "α"),
            ]),
            "\\alphaα"
        );
        assert_eq!(
            write_atoms(&[
                (Math, ControlSequence, EMIT, r"\alpha"),
                (Math, MathChar, EMIT, "α"),
            ]),
            "\\alpha α"
        );
    }

    #[test]
    fn whitespace_in_the_next_atom_closes_a_control_word() {
        // The next atom already starts with whitespace, so the name is
        // terminated and the original spacing survives unchanged.
        assert_eq!(
            write_atoms(&[
                (Text, ControlSequence, EMIT, r"\alpha"),
                (Text, TextChunk, EMIT, "  x"),
            ]),
            r"\alpha  x"
        );
    }

    #[test]
    fn suppressed_boundary_drops_optional_space_but_not_control_word_separator() {
        // Suppression only cancels optional math spacing. A digit closes the
        // control word by itself, and a letter after `\,` never opened one.
        assert_eq!(
            write_atoms(&[
                (Math, ControlSequence, EMIT, r"\alpha"),
                (Math, MathDigit, SUPPRESS, "1"),
            ]),
            r"\alpha1"
        );
        assert_eq!(
            write_atoms(&[
                (Text, ControlSequence, EMIT, r"\,"),
                (Text, TextChunk, SUPPRESS, "b"),
            ]),
            r"\,b"
        );
    }

    #[test]
    fn empty_attached_emission_does_not_close_an_open_control_word() {
        // An attached emission that writes nothing (a prime run of zero marks)
        // is not a token, so it must not replace the atom identity that the
        // next boundary check still depends on.
        assert_eq!(
            write_atoms(&[
                (Math, ControlSequence, EMIT, r"\alpha"),
                (Math, Prime, ATTACH, ""),
                (Math, MathChar, EMIT, "x"),
            ]),
            r"\alpha x"
        );
    }

    #[test]
    fn control_word_delimiters_and_error_snippets_keep_letter_separator() {
        // `\left\langle B` and an error snippet ending in `\frac` must not
        // glue into a longer control word, even with optional space suppressed.
        assert_eq!(
            write_atoms(&[
                (Math, DelimiterToken, SUPPRESS, r"\langle"),
                (Math, MathChar, SUPPRESS, "B"),
            ]),
            r"\langle B"
        );
        assert_eq!(
            write_atoms(&[
                (Text, RawFragment, EMIT, r"\frac"),
                (Text, TextChunk, EMIT, "b"),
            ]),
            r"\frac b"
        );
        // A plain delimiter, ordinary raw letters, or `\\` before a letter
        // ends no control word.
        assert_eq!(
            write_atoms(&[
                (Math, DelimiterToken, SUPPRESS, "("),
                (Math, MathChar, SUPPRESS, "B"),
            ]),
            "(B"
        );
        assert_eq!(
            write_atoms(&[
                (Text, RawFragment, EMIT, r"\\pt"),
                (Text, TextChunk, EMIT, "b"),
            ]),
            r"\\ptb"
        );
    }

    #[test]
    fn sibling_bracket_or_star_stays_apart_from_control_sequence() {
        // `\\ [6pt]` and `\\ *` as row content would be absorbed by `\\`'s
        // `!s !o` slots if glued.
        assert_eq!(
            write_atoms(&[
                (Math, ControlSequence, EMIT, r"\\"),
                (Math, MathChar, SUPPRESS, "["),
            ]),
            r"\\ ["
        );
        assert_eq!(
            write_atoms(&[
                (Math, ControlSequence, EMIT, r"\\"),
                (Math, MathChar, SUPPRESS, "*"),
            ]),
            r"\\ *"
        );
        let options = Style::Spaced.settings();
        let mut writer = AtomWriter::new(NoopRecorder);
        let public = FragmentKind::Character;
        let role = FragmentRole::Leaf;
        writer.emit(Math, ControlSequence, public, role, None, r"\\", &options);
        writer.emit_star_suffix(Math, Some(NodeSlot::Arg(0)));
        writer.emit_with_boundary(
            Math,
            MathChar,
            public,
            role,
            None,
            "[",
            BoundaryPolicy::SuppressOptionalSpace,
            &options,
        );
        assert_eq!(writer.finish(), r"\\* [");
    }

    #[test]
    fn argument_owned_bracket_follows_command_spacing() {
        // Optional-argument brackets and delimiter arguments are not siblings.
        assert_eq!(
            write_atoms(&[
                (Math, ControlSequence, EMIT, r"\sqrt"),
                (Math, Brace, SUPPRESS, "["),
            ]),
            r"\sqrt["
        );
        assert_eq!(
            write_atoms(&[
                (Math, ControlSequence, EMIT, r"\big"),
                (Math, DelimiterToken, SUPPRESS, "["),
            ]),
            r"\big["
        );
    }

    #[test]
    fn delimiter_token_matches_lexer_on_serialized_delimiter() {
        let cases = [
            Delimiter::None,
            Delimiter::Char('('),
            Delimiter::Char('{'),
            Delimiter::Char(']'),
            Delimiter::Char('\\'),
            Delimiter::Char('%'),
            Delimiter::Char(' '),
            Delimiter::Char('\x00'),
            Delimiter::Char('α'),
            Delimiter::Control(String::new()),
            Delimiter::Control("langle".to_string()),
            Delimiter::Control("|".to_string()),
            Delimiter::Control("\n".to_string()),
            Delimiter::Control("\r\n".to_string()),
            Delimiter::Control("foo1".to_string()),
            Delimiter::Control("α".to_string()),
        ];
        for delimiter in &cases {
            let text = match delimiter {
                Delimiter::None => ".".to_string(),
                Delimiter::Char(ch) => ch.to_string(),
                Delimiter::Control(name) => format!(r"\{name}"),
            };
            let expected = Token::lexer(&text).next().and_then(Result::ok);
            assert_eq!(
                delimiter_token(delimiter),
                expected,
                "mismatch for delimiter {delimiter:?}"
            );
        }
    }
}
