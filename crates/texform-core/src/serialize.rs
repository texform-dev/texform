//! Canonical AST serializer — converts [`Ast`] back to LaTeX text.
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
//! Most spacing rules are concentrated in the atom writer's boundary decision,
//! which inspects the previous atom, the next atom, the current content mode,
//! and the active [`SerializeOptions`]. A few wrapper/scalar helpers still emit
//! preformatted spaces directly for cases that cannot be expressed as a simple
//! previous/next atom decision (for example empty padded groups). This keeps
//! the boundary logic local and avoids post-hoc string cleanup — important
//! because TeX whitespace carries both lexical and semantic weight.

use serde::{Deserialize, Serialize};

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind,
    Node, NodeId,
};

/// Serialize an AST to LaTeX using the default canonical style.
pub fn serialize(ast: &Ast) -> String {
    serialize_with(ast, &SerializeOptions::default())
}

/// Serialize an AST to LaTeX with explicit style options.
pub fn serialize_with(ast: &Ast, options: &SerializeOptions) -> String {
    let mut serializer = Serializer::new(ast, options);
    serializer.serialize_root();
    serializer.finish()
}

/// Error type for fallible LaTeX serialization.
///
/// The current canonical serializer is infallible; this type exists so the
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

/// Top-level serialization options, grouped by scope.
///
/// `math.*` controls math-mode-specific behavior; `syntax.*` controls
/// structural LaTeX syntax that is mode-independent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SerializeOptions {
    /// Math-mode-specific output controls (spacing, scripts, infix grouping).
    pub math: MathSerializeOptions,
    /// Mode-independent structural syntax controls (e.g. environment headers).
    pub syntax: SyntaxSerializeOptions,
}

/// Math-mode serialization options.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MathSerializeOptions {
    pub spacing: MathSpacingOptions,
    pub scripts: MathScriptOptions,
    pub infix: MathInfixOptions,
}

/// Infix serialization options for math mode.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MathInfixOptions {
    pub grouping: InfixGrouping,
}

/// Spacing controls within math mode.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MathSpacingOptions {
    pub commands: CommandSpacing,
    pub group_inner_spacing: MathGroupInnerSpacing,
    pub adjacent_chars: AdjacentCharSpacing,
}

/// Sub/superscript formatting controls.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MathScriptOptions {
    pub spacing: ScriptSpacing,
    pub order: ScriptOrder,
}

/// Structural syntax options (mode-independent).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyntaxSerializeOptions {
    pub environments: EnvironmentSerializeOptions,
}

/// Environment header formatting options.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentSerializeOptions {
    pub name_spacing: EnvironmentNameSpacing,
}

/// Whether to insert a space between a command and the following structural
/// token in math mode.
///
/// `Spaced`: `\frac { a }` — `Minimal`: `\frac{ a }`.
/// `Minimal` only removes the command-to-structure boundary itself; it still
/// preserves lexical separation when omitting a space would merge a following
/// letter-like token into the control sequence name (e.g. `\alpha x`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandSpacing {
    #[default]
    Spaced,
    Minimal,
}

/// Controls the inside spacing of math brace groups.
///
/// `Padded`: `{ a }`, `{ }`, `x ^ { 2 }`.
/// `Compact`: `{a}`, `{}`, `x ^ {2}`.
///
/// This applies both to explicit/implicit `Group` nodes and to wrapper-owned
/// braces emitted for command/script arguments. Text-mode content and scalar
/// fragments (environment names, dimensions, etc.) are never padded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathGroupInnerSpacing {
    #[default]
    Padded,
    Compact,
}

/// Whether adjacent math character atoms get explicit space separation.
///
/// `Spaced`: `a b c + d` — `Compact`: `abc+d`.
/// Letters and symbols follow this setting; adjacent digits are always
/// glued (see `MathDigit`), so multi-digit numbers stay compact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdjacentCharSpacing {
    #[default]
    Spaced,
    Compact,
}

/// Whether to insert spaces immediately around `_` and `^` markers.
///
/// `Spaced`: `x _ { i }` — `Compact`: `x_{ i }`.
/// This only controls the marker boundary itself; inner brace spacing still
/// follows the normal math group rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptSpacing {
    #[default]
    Spaced,
    Compact,
}

/// Fixed output order for subscript and superscript.
///
/// `SubFirst`: `x _ { i } ^ { 2 }` — `SupFirst`: `x ^ { 2 } _ { i }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptOrder {
    #[default]
    SubFirst,
    SupFirst,
}

/// Whether math infix operands are always braced or only when needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfixGrouping {
    AlwaysExplicit,
    #[default]
    WhenRequired,
}

/// Whether `\begin` / `\end` get a space before the name brace.
///
/// `Spaced` -> `\begin {matrix}`, `Compact` -> `\begin{matrix}`.
/// The environment name inside `{}` is always compact, and this setting is
/// independent from [`CommandSpacing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentNameSpacing {
    #[default]
    Spaced,
    Compact,
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

/// Accumulates output text and decides where to insert inter-atom spaces.
///
/// Most boundary rules live in the atom writer's central decision function,
/// making them testable in isolation without constructing a full AST. A few helpers still
/// bypass it for preformatted cases such as empty padded groups. The writer
/// tracks only the *previous* atom kind — no look-ahead — so the serializer
/// must emit atoms in final output order.
#[derive(Default)]
struct AtomWriter {
    output: String,
    previous: Option<AtomKind>,
}

impl AtomWriter {
    fn emit(&mut self, mode: ContentMode, kind: AtomKind, text: &str, options: &SerializeOptions) {
        if self.should_insert_space(mode, kind, options) {
            self.output.push(' ');
        }
        self.output.push_str(text);
        self.previous = Some(kind);
    }

    /// Append `*` directly — star must glue to the preceding control sequence
    /// without any boundary space (`\operatorname*`, not `\operatorname *`).
    fn emit_star_suffix(&mut self) {
        self.output.push('*');
    }

    /// Central boundary-decision function.
    ///
    /// Returns `true` when a space should be inserted between the previous atom
    /// and the upcoming `next` atom. Rules are checked top-down; the first
    /// matching branch wins.
    fn should_insert_space(
        &self,
        mode: ContentMode,
        next: AtomKind,
        options: &SerializeOptions,
    ) -> bool {
        let Some(prev) = self.previous else {
            return false;
        };

        // A control sequence followed by a letter-like atom always needs a
        // boundary; without it the letter would be absorbed into the command
        // name during re-lexing (e.g. `\alphax` vs `\alpha x`).
        if matches!(prev, AtomKind::ControlSequence)
            && matches!(
                next,
                AtomKind::TextChunk | AtomKind::MathChar | AtomKind::RawFragment
            )
        {
            return true;
        }

        // Text mode never injects extra spaces. Some callers also reuse
        // `ContentMode::Text` as a synthetic "compact boundary" mode.
        if matches!(mode, ContentMode::Text) {
            return false;
        }

        // --- Below this point, we are in math mode ---

        if matches!(prev, AtomKind::ControlSequence) {
            return match next {
                AtomKind::Brace | AtomKind::DelimiterToken => {
                    matches!(options.math.spacing.commands, CommandSpacing::Spaced)
                }
                _ => true,
            };
        }

        if matches!((prev, next), (AtomKind::MathDigit, AtomKind::MathDigit)) {
            return false;
        }

        if matches!(prev, AtomKind::MathChar | AtomKind::MathDigit)
            && matches!(next, AtomKind::MathChar | AtomKind::MathDigit)
        {
            return matches!(
                options.math.spacing.adjacent_chars,
                AdjacentCharSpacing::Spaced
            );
        }

        // Prime marks attach tightly to the preceding atom. A following atom
        // still gets separated so a leading prime stays readable as its own
        // item in canonical output.
        if matches!(next, AtomKind::Prime) {
            return !matches!(
                prev,
                AtomKind::ControlSequence
                    | AtomKind::MathChar
                    | AtomKind::MathDigit
                    | AtomKind::Prime
            );
        }
        if matches!(prev, AtomKind::Prime) && matches!(next, AtomKind::ScriptMark) {
            return matches!(options.math.scripts.spacing, ScriptSpacing::Spaced);
        }
        if matches!(prev, AtomKind::Prime) {
            return true;
        }

        // `$` delimiters bind tightly to their content (`$x$`, not `$ x $`).
        if matches!(prev, AtomKind::Dollar) || matches!(next, AtomKind::Dollar) {
            return false;
        }

        if matches!(prev, AtomKind::ScriptMark) || matches!(next, AtomKind::ScriptMark) {
            return matches!(options.math.scripts.spacing, ScriptSpacing::Spaced);
        }

        true
    }

    fn finish(self) -> String {
        self.output
    }
}

/// Recursive AST walker that emits atoms into an [`AtomWriter`].
///
/// Mode is tracked through the recursion stack — each `visit` call receives
/// the content mode of its parent context, so no separate mutable mode stack
/// is needed.
struct Serializer<'a> {
    ast: &'a Ast,
    options: &'a SerializeOptions,
    writer: AtomWriter,
}

impl<'a> Serializer<'a> {
    fn new(ast: &'a Ast, options: &'a SerializeOptions) -> Self {
        Self {
            ast,
            options,
            writer: AtomWriter::default(),
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

        for &child in children {
            self.visit(child, *mode);
        }
    }

    fn visit(&mut self, id: NodeId, mode: ContentMode) {
        match self.ast.node(id).clone() {
            Node::Root { .. } => unreachable!("root node must be handled by serialize_root"),
            Node::Environment {
                name, args, body, ..
            } => self.visit_environment(&name, &args, body, mode),
            Node::Infix {
                name,
                args,
                left,
                right,
            } => self.visit_infix(&name, &args, left, right),
            Node::Declarative { name, args } => self.visit_declarative(&name, &args, mode),
            Node::Group {
                children,
                kind,
                mode: child_mode,
            } => self.visit_group(kind, child_mode, &children),
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => self.visit_scripted(base, subscript, superscript),
            Node::Command { name, args, .. } => self.visit_command(&name, &args, mode),
            Node::Prime { count } => self.visit_prime(count, mode),
            Node::Char(ch) => self.visit_char(ch, mode),
            Node::Text(text) => self
                .writer
                .emit(mode, AtomKind::TextChunk, &text, self.options),
            Node::ActiveSpace => self
                .writer
                .emit(mode, AtomKind::ActiveChar, "~", self.options),
            Node::Error { snippet, .. } => {
                self.writer
                    .emit(mode, AtomKind::RawFragment, &snippet, self.options)
            }
        }
    }

    /// Emit a group node.
    ///
    /// `Explicit` and `Implicit` are treated identically as brace groups — the
    /// distinction is parser/transform history and must not leak into the text.
    fn visit_group(&mut self, kind: GroupKind, child_mode: ContentMode, children: &[NodeId]) {
        match kind {
            GroupKind::Explicit | GroupKind::Implicit => {
                if matches!(child_mode, ContentMode::Math)
                    && matches!(
                        self.options.math.spacing.group_inner_spacing,
                        MathGroupInnerSpacing::Compact
                    )
                {
                    self.emit_compact_math_brace_group(children);
                } else {
                    self.emit_wrapped(child_mode, AtomKind::Brace, "{", "}", children);
                }
            }
            GroupKind::Delimited { left, right } => {
                self.writer.emit(
                    ContentMode::Math,
                    AtomKind::ControlSequence,
                    r"\left",
                    self.options,
                );
                self.emit_delimiter(&left, ContentMode::Math);
                for &child in children {
                    self.visit(child, ContentMode::Math);
                }
                self.writer.emit(
                    ContentMode::Math,
                    AtomKind::ControlSequence,
                    r"\right",
                    self.options,
                );
                self.emit_delimiter(&right, ContentMode::Math);
            }
            GroupKind::InlineMath => self.visit_inline_math(children),
        }
    }

    fn visit_command(&mut self, name: &str, args: &[Option<Argument>], mode: ContentMode) {
        self.writer.emit(
            mode,
            AtomKind::ControlSequence,
            &format!(r"\{}", name),
            self.options,
        );

        for slot in args {
            self.visit_argument_slot(slot, mode);
        }
    }

    /// Emit an infix command in its original syntactic form.
    ///
    /// The serializer does not assume the infix has been desugared by a
    /// transform rule; an un-rewritten `\over` still round-trips correctly.
    fn visit_infix(&mut self, name: &str, args: &[ArgumentSlot], left: NodeId, right: NodeId) {
        self.emit_infix_operand(left);
        self.writer.emit(
            ContentMode::Math,
            AtomKind::ControlSequence,
            &format!(r"\{}", name),
            self.options,
        );
        for slot in args {
            self.visit_argument_slot(slot, ContentMode::Math);
        }
        self.emit_infix_operand(right);
    }

    /// Emit a declarative command with its explicit arguments.
    fn visit_declarative(&mut self, name: &str, args: &[ArgumentSlot], mode: ContentMode) {
        self.writer.emit(
            mode,
            AtomKind::ControlSequence,
            &format!(r"\{}", name),
            self.options,
        );
        for slot in args {
            self.visit_argument_slot(slot, mode);
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
        for slot in args {
            self.visit_argument_slot(slot, mode);
        }

        match self.ast.node(body).clone() {
            Node::Group {
                children,
                mode: body_mode,
                kind: GroupKind::Implicit,
            } => {
                for child in children {
                    self.visit(child, body_mode);
                }
            }
            Node::Group {
                mode: body_mode, ..
            } => self.visit(body, body_mode),
            other => unreachable!("environment body must remain a group, got {:?}", other),
        }

        self.emit_environment_head(mode, r"\end", name);
    }

    /// Emit `\begin {name}` or `\end {name}` (or compact `\begin{name}`).
    ///
    /// Environment header spacing is intentionally controlled here instead of
    /// piggybacking on the generic command-to-brace rule, so it stays
    /// independent from `CommandSpacing`.
    fn emit_environment_head(&mut self, outer_mode: ContentMode, head: &str, name: &str) {
        self.writer
            .emit(outer_mode, AtomKind::ControlSequence, head, self.options);

        if matches!(
            self.options.syntax.environments.name_spacing,
            EnvironmentNameSpacing::Spaced
        ) {
            self.writer.output.push(' ');
        }

        self.writer.output.push('{');
        self.writer.output.push_str(name);
        self.writer.output.push('}');
        self.writer.previous = Some(AtomKind::Brace);
    }

    /// Dispatch a single argument slot to the appropriate emitter.
    ///
    /// Content arguments (`MathContent` / `TextContent`) recurse into the
    /// serializer; scalar arguments are emitted as opaque fragments that
    /// bypass math-mode token spacing.
    fn visit_argument_slot(&mut self, slot: &Option<Argument>, mode: ContentMode) {
        let Some(arg) = slot else {
            return;
        };

        match (&arg.kind, &arg.value) {
            (ArgumentKind::Star, ArgumentValue::Boolean(true)) => self.writer.emit_star_suffix(),
            (ArgumentKind::Star, ArgumentValue::Boolean(false)) => {}
            (ArgumentKind::Star, _) => {
                unreachable!("star slots must carry boolean values")
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, ArgumentValue::MathContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Math, "{", "}", mode);
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, ArgumentValue::TextContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Text, "{", "}", mode);
            }
            (
                ArgumentKind::Mandatory | ArgumentKind::Group,
                ArgumentValue::OperatorNameContent(child),
            ) => {
                self.emit_operator_name_argument_content(*child, "{", "}", mode);
            }
            (ArgumentKind::Optional, ArgumentValue::MathContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Math, "[", "]", mode);
            }
            (ArgumentKind::Optional, ArgumentValue::TextContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Text, "[", "]", mode);
            }
            (ArgumentKind::Optional, ArgumentValue::OperatorNameContent(child)) => {
                self.emit_operator_name_argument_content(*child, "[", "]", mode);
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, value) => {
                self.emit_scalar_wrapped(value, "{", "}", mode)
            }
            (ArgumentKind::Optional, value) => self.emit_scalar_wrapped(value, "[", "]", mode),
            (ArgumentKind::Delimited { open, close }, ArgumentValue::MathContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::MathContent(node)) => {
                self.emit_recorded_delimiters(open, close, *node, ContentMode::Math)
            }
            (ArgumentKind::Delimited { open, close }, ArgumentValue::TextContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::TextContent(node)) => {
                self.emit_recorded_delimiters(open, close, *node, ContentMode::Text)
            }
            (ArgumentKind::Delimited { open, close }, ArgumentValue::OperatorNameContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::OperatorNameContent(node)) => {
                self.emit_operator_name_between_delimiters(open, close, *node, mode)
            }
            (ArgumentKind::Delimited { open, close }, value)
            | (ArgumentKind::Paired { open, close }, value) => {
                self.emit_scalar_between_delimiters(open, close, value, mode)
            }
        }
    }

    /// Emit a content argument wrapped in its matching delimiters.
    ///
    /// `content_mode` is the mode the argument was parsed in (from the
    /// `MathContent` / `TextContent` variant), while `wrapper_mode` controls
    /// boundary spacing around the outer delimiters.
    fn emit_argument_content(
        &mut self,
        child: NodeId,
        content_mode: ContentMode,
        open: &str,
        close: &str,
        wrapper_mode: ContentMode,
    ) {
        self.emit_wrapped_content(child, wrapper_mode, content_mode, open, close);
    }

    fn emit_operator_name_argument_content(
        &mut self,
        child: NodeId,
        open: &str,
        close: &str,
        wrapper_mode: ContentMode,
    ) {
        self.writer
            .emit(wrapper_mode, AtomKind::Brace, open, self.options);
        self.visit_operator_name_content_node(child);
        self.writer
            .emit(ContentMode::Text, AtomKind::Brace, close, self.options);
    }

    fn visit_scripted(
        &mut self,
        base: NodeId,
        subscript: Option<NodeId>,
        superscript: Option<NodeId>,
    ) {
        self.visit(base, ContentMode::Math);

        match self.options.math.scripts.order {
            ScriptOrder::SubFirst => {
                if let Some(node) = subscript {
                    self.emit_script('_', node);
                }
                if let Some(node) = superscript {
                    self.emit_superscript(node);
                }
            }
            ScriptOrder::SupFirst => {
                if let Some(node) = superscript {
                    self.emit_superscript(node);
                }
                if let Some(node) = subscript {
                    self.emit_script('_', node);
                }
            }
        }
    }

    fn visit_inline_math(&mut self, children: &[NodeId]) {
        self.writer
            .emit(ContentMode::Text, AtomKind::Dollar, "$", self.options);
        for &child in children {
            self.visit(child, ContentMode::Math);
        }
        self.writer
            .emit(ContentMode::Text, AtomKind::Dollar, "$", self.options);
    }

    /// Emit a single `_` or `^` followed by its braced argument.
    ///
    /// Script spacing is controlled by emitting the marker in a synthetic
    /// mode: `Math` triggers boundary insertion while `Text` suppresses it,
    /// reusing the existing boundary logic without a dedicated
    /// script-mark branch in every caller.
    fn emit_script(&mut self, marker: char, node: NodeId) {
        let mode = match self.options.math.scripts.spacing {
            ScriptSpacing::Spaced => ContentMode::Math,
            ScriptSpacing::Compact => ContentMode::Text,
        };
        self.writer.emit(
            mode,
            AtomKind::ScriptMark,
            &marker.to_string(),
            self.options,
        );
        self.emit_wrapped_content(node, ContentMode::Math, ContentMode::Math, "{", "}");
    }

    fn emit_superscript(&mut self, node: NodeId) {
        if let Node::Prime { count } = self.ast.node(node) {
            self.emit_prime_marks(*count);
        } else {
            self.emit_script('^', node);
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
        // instead of `{}` under Padded mode — the normal visitor path would
        // emit `{` then immediately `}` with no content in between.
        if children.is_empty()
            && matches!(mode, ContentMode::Math)
            && matches!(kind, AtomKind::Brace)
            && matches!(
                self.options.math.spacing.group_inner_spacing,
                MathGroupInnerSpacing::Padded
            )
        {
            self.emit_padded_empty_group(mode, kind, open, close);
            return;
        }

        self.writer.emit(mode, kind, open, self.options);
        for &child in children {
            self.visit(child, mode);
        }
        self.writer.emit(mode, kind, close, self.options);
    }

    fn emit_compact_math_brace_group(&mut self, children: &[NodeId]) {
        self.writer
            .emit(ContentMode::Math, AtomKind::Brace, "{", self.options);

        self.writer.previous = None;
        for &child in children {
            self.visit(child, ContentMode::Math);
        }

        self.writer
            .emit(ContentMode::Text, AtomKind::Brace, "}", self.options);
    }

    /// Emit `{ }` as a single pre-formatted unit.
    ///
    /// Bypasses the normal atom pipeline because there is no interior content
    /// to visit, yet the padding space must still appear between the braces.
    fn emit_padded_empty_group(
        &mut self,
        mode: ContentMode,
        kind: AtomKind,
        open: &str,
        close: &str,
    ) {
        if self.writer.should_insert_space(mode, kind, self.options) {
            self.writer.output.push(' ');
        }
        self.writer.output.push_str(open);
        self.writer.output.push(' ');
        self.writer.output.push_str(close);
        self.writer.previous = Some(kind);
    }

    /// Emit a child node inside wrapper-owned delimiters (e.g. `{ ... }`).
    ///
    /// When the child is itself a brace group, its children are inlined
    /// directly to avoid double-bracing (`{ { a } }` → `{ a }`). This is
    /// safe because the wrapper already provides the grouping delimiter.
    fn emit_wrapped_content(
        &mut self,
        child: NodeId,
        wrapper_mode: ContentMode,
        content_mode: ContentMode,
        open: &str,
        close: &str,
    ) {
        self.writer
            .emit(wrapper_mode, AtomKind::Brace, open, self.options);

        let compact_math_inner = matches!(content_mode, ContentMode::Math)
            && matches!(
                self.options.math.spacing.group_inner_spacing,
                MathGroupInnerSpacing::Compact
            );

        if compact_math_inner {
            self.writer.previous = None;
        }

        match self.ast.node(child) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                mode: child_mode,
            } => {
                if children.is_empty()
                    && matches!(*child_mode, ContentMode::Math)
                    && matches!(
                        self.options.math.spacing.group_inner_spacing,
                        MathGroupInnerSpacing::Padded
                    )
                {
                    self.writer.output.push(' ');
                    self.writer.output.push_str(close);
                    self.writer.previous = Some(AtomKind::Brace);
                    return;
                }
                for &grandchild in children {
                    self.visit(grandchild, *child_mode);
                }
            }
            _ => self.visit(child, content_mode),
        }

        let close_mode = if compact_math_inner {
            ContentMode::Text
        } else {
            content_mode
        };
        self.writer
            .emit(close_mode, AtomKind::Brace, close, self.options);
    }

    fn emit_infix_operand(&mut self, node: NodeId) {
        if self.is_empty_infix_operand(node) {
            return;
        }

        match self.options.math.infix.grouping {
            InfixGrouping::AlwaysExplicit => {
                self.emit_wrapped_content(node, ContentMode::Math, ContentMode::Math, "{", "}")
            }
            InfixGrouping::WhenRequired => {
                if self.infix_operand_requires_braces(node) {
                    self.emit_wrapped_content(node, ContentMode::Math, ContentMode::Math, "{", "}");
                } else {
                    self.emit_unwrapped_infix_operand(node);
                }
            }
        }
    }

    fn emit_unwrapped_infix_operand(&mut self, node: NodeId) {
        match self.ast.node(node) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                mode,
            } => {
                for &child in children {
                    self.visit(child, *mode);
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
        value: &ArgumentValue,
        open: &str,
        close: &str,
        mode: ContentMode,
    ) {
        if self
            .writer
            .should_insert_space(mode, AtomKind::Brace, self.options)
        {
            self.writer.output.push(' ');
        }
        self.writer.output.push_str(open);
        self.writer
            .output
            .push_str(&self.scalar_argument_text(value));
        self.writer.output.push_str(close);
        self.writer.previous = Some(AtomKind::Brace);
    }

    fn emit_recorded_delimiters(
        &mut self,
        open: &Delimiter,
        close: &Delimiter,
        node: NodeId,
        mode: ContentMode,
    ) {
        self.emit_delimiter(open, mode);
        self.visit_argument_content_node(node, mode);
        self.emit_delimiter(close, mode);
    }

    fn emit_operator_name_between_delimiters(
        &mut self,
        open: &Delimiter,
        close: &Delimiter,
        node: NodeId,
        mode: ContentMode,
    ) {
        self.emit_delimiter(open, mode);
        self.visit_operator_name_content_node(node);
        self.emit_delimiter(close, ContentMode::Text);
    }

    fn visit_operator_name_content_node(&mut self, node: NodeId) {
        match self.ast.node(node) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                ..
            } => {
                for &child in children {
                    self.visit(child, ContentMode::Text);
                }
            }
            _ => self.visit(node, ContentMode::Text),
        }
    }

    fn emit_scalar_between_delimiters(
        &mut self,
        open: &Delimiter,
        close: &Delimiter,
        value: &ArgumentValue,
        mode: ContentMode,
    ) {
        self.emit_delimiter(open, mode);
        let text = self.scalar_argument_text(value);
        self.writer
            .emit(mode, AtomKind::RawFragment, &text, self.options);
        self.emit_delimiter(close, mode);
    }

    fn scalar_argument_text(&self, value: &ArgumentValue) -> String {
        match value {
            ArgumentValue::Delimiter(delimiter) => self.delimiter_text(delimiter),
            ArgumentValue::CSName(name)
            | ArgumentValue::Dimension(name)
            | ArgumentValue::Integer(name)
            | ArgumentValue::KeyVal(name)
            | ArgumentValue::Column(name) => name.clone(),
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

    fn delimiter_text(&self, delimiter: &Delimiter) -> String {
        match delimiter {
            Delimiter::None => ".".to_string(),
            Delimiter::Char(ch) => ch.to_string(),
            Delimiter::Control(name) => format!(r"\{}", name),
        }
    }

    fn emit_delimiter(&mut self, delimiter: &Delimiter, mode: ContentMode) {
        match delimiter {
            Delimiter::None => self
                .writer
                .emit(mode, AtomKind::DelimiterToken, ".", self.options),
            Delimiter::Char(ch) => self.writer.emit(
                mode,
                AtomKind::DelimiterToken,
                &ch.to_string(),
                self.options,
            ),
            Delimiter::Control(name) => self.writer.emit(
                mode,
                AtomKind::DelimiterToken,
                &format!(r"\{}", name),
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
                for &child in children {
                    self.visit(child, *child_mode);
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
        let text = serialized_char(ch, mode);
        self.writer.emit(mode, kind, &text, self.options);
    }

    fn visit_prime(&mut self, count: usize, mode: ContentMode) {
        if matches!(mode, ContentMode::Math) {
            self.writer
                .emit(mode, AtomKind::Prime, &"'".repeat(count), self.options);
        } else {
            self.writer
                .emit(mode, AtomKind::TextChunk, &"'".repeat(count), self.options);
        }
    }

    fn emit_prime_marks(&mut self, count: usize) {
        self.writer.output.push_str(&"'".repeat(count));
        self.writer.previous = Some(AtomKind::Prime);
    }

    fn finish(self) -> String {
        self.writer.finish()
    }
}

fn serialized_char(ch: char, mode: ContentMode) -> String {
    let needs_escape = match mode {
        ContentMode::Math => matches!(ch, '%' | '$' | '#' | '_' | '{' | '}'),
        ContentMode::Text => matches!(ch, '%' | '$' | '&' | '#' | '_' | '{' | '}'),
    };

    if needs_escape {
        format!(r"\{ch}")
    } else {
        ch.to_string()
    }
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
        let options = SerializeOptions::default();
        let mut writer = AtomWriter::default();

        writer.emit(
            ContentMode::Math,
            AtomKind::ControlSequence,
            r"\operatorname",
            &options,
        );
        writer.emit_star_suffix();

        assert_eq!(writer.finish(), r"\operatorname*");
    }

    #[test]
    fn test_atom_writer_keeps_text_chunk_compact() {
        let options = SerializeOptions::default();
        let mut writer = AtomWriter::default();

        writer.emit(ContentMode::Text, AtomKind::TextChunk, "abc", &options);
        writer.emit(ContentMode::Text, AtomKind::TextChunk, " def", &options);

        assert_eq!(writer.finish(), "abc def");
    }
}
