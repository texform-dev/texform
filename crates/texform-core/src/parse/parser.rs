//! Core hand-written recursive descent parser state, scope guards,
//! checkpointing, and diagnostic emission primitives.

use std::borrow::Cow;

use texform_interface::syntax_node::ContentMode;

use crate::lexer::Token;
use crate::parse::config::ParseConfig;
use crate::parse::context::ParseContext;
use crate::parse::cursor::Cursor;
use crate::parse::diagnostics;
use crate::parse::result::{ParseDiagnostic, ParseDiagnosticContext, Span};

/// Global hard limit on parser recursion depth across recursive entry points
/// (content loops, argument slots, delimited groups, environments, inline math)
/// to prevent stack overflow on pathological non-brace nesting.
///
/// Explicit `{...}` groups enter `with_recursion` once per level (via their
/// content loop), so a limit of 200 comfortably allows the full default
/// `max_group_depth` (128) while bounding heavier ~2-step-per-level nesting
/// (`\frac`, `\left...\right`, `\begin...\end`) to ~100 levels — safely inside
/// a 2 MiB unoptimized debug thread stack and constrained WASM stacks.
pub(crate) const MAX_RECURSION_DEPTH: usize = 200;

/// Internal parser result carrying a lightweight control-flow signal on error.
///
/// The failing construct always emits its diagnostic(s) into `TokenParser`
/// before returning `Err(ParseAbort)`.
pub(crate) type PResult<T> = Result<T, ParseAbort>;

/// Control-flow abort signal returned after a construct emits its diagnostic(s).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParseAbort {
    pub hint: RecoveryHint,
}

/// Strategy hint telling `recovery.rs` how to recover from a failed item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryHint {
    /// Generic Sync-Set skip.
    Skip,
    /// Propagate directly to the enclosing construct without item-level token
    /// skipping (used for `\left` delimiter/closing errors and unknown `\begin`
    /// headers). Downgraded to [`Self::Skip`] at argument-slot boundaries.
    Propagate,
    /// Unrecoverable global parser failure (recursion-depth overflow);
    /// propagates past all item recovery and argument-slot downgrading.
    Fatal,
    /// Unknown command head: consume only the control sequence token.
    UnknownCommandHead,
    /// Command used in the wrong mode: also consume trailing `[...]` / `{...}`
    /// groups.
    CommandModeError,
    /// `_` / `^` without an operand: replace the scripted atom with an `Error`
    /// node in place.
    MissingScriptContent,
}

/// Live entry on the parser's open-environment stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenEnv<'src> {
    pub name: Cow<'src, str>,
    pub begin_span: Span,
    pub name_span: Span,
    pub body_mode: ContentMode,
    /// Diagnostic index where an EOF `UnclosedEnvironment` was already emitted
    /// for this environment; cleared symmetrically on [`TokenParser::rewind`].
    eof_reported_at: Option<usize>,
}

impl<'src> OpenEnv<'src> {
    #[inline]
    pub(crate) fn new(
        name: Cow<'src, str>,
        begin_span: Span,
        name_span: Span,
        body_mode: ContentMode,
    ) -> Self {
        Self {
            name,
            begin_span,
            name_span,
            body_mode,
            eof_reported_at: None,
        }
    }
}

/// Extra stop token for math content loops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MathStop {
    /// Standard math boundaries (`EOF`, `}`, `$`, `\right`, `\end`).
    Default,
    /// Inside `\(...\)`: `\)` also ends math content.
    ControlParen,
}

/// Returns `true` when `token` is a hard closing boundary for math content
/// (excluding infix commands).
///
/// Single source of truth shared by math content loops, script operand
/// boundaries, and Sync-Set recovery.
#[inline]
pub(crate) fn is_math_hard_stop(token: Option<Token<'_>>, math_stop: MathStop) -> bool {
    match token {
        None | Some(Token::RBrace | Token::MathShift | Token::ControlSeq("right" | "end")) => true,
        Some(Token::ControlSeq(")")) => math_stop == MathStop::ControlParen,
        Some(_) => false,
    }
}

/// Saved parser position and diagnostic watermark for speculative lookahead or
/// item-level error recovery.
///
/// Rewinding is only valid within the same `with_*` scope where the checkpoint
/// was created; debug builds assert that stack depths are unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Checkpoint {
    pub pos: usize,
    pub diag_len: usize,
    #[cfg(debug_assertions)]
    context_depth: usize,
    #[cfg(debug_assertions)]
    env_depth: usize,
}

/// Lightweight context frame pushed on the happy path without heap-allocating
/// label strings until a diagnostic actually snapshots the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ContextFrame {
    label: &'static str,
    start: usize,
    fixed_end: Option<usize>,
}

/// Stateful recursive descent parser over a SoA [`Cursor`].
///
/// All semantic and diagnostic state lives in plain mutable fields (no `Cell`
/// or `RefCell`) and is scoped strictly through closure-based `with_*` helpers.
pub(crate) struct TokenParser<'tok, 'src, 'ctx> {
    pub(crate) cursor: Cursor<'tok, 'src>,
    pub(crate) src: &'src str,
    pub(crate) ctx: &'ctx ParseContext,
    pub(crate) config: &'ctx ParseConfig,
    // Semantic state
    group_depth: usize,
    recursion_depth: usize,
    row_break_skips_spaces: bool,
    math_stop: MathStop,
    // Diagnostics and recovery context, maintained live instead of re-lexing afterwards
    diagnostics: Vec<ParseDiagnostic>,
    context_stack: Vec<ContextFrame>,
    env_stack: Vec<OpenEnv<'src>>,
    /// Active floor of `env_stack` for the current sub-window; isolates command
    /// arguments from outer environments.
    env_stack_base: usize,
}

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Construct a fresh parser positioned at the start of `cursor`.
    pub(crate) fn new(
        cursor: Cursor<'tok, 'src>,
        src: &'src str,
        ctx: &'ctx ParseContext,
        config: &'ctx ParseConfig,
    ) -> Self {
        Self {
            cursor,
            src,
            ctx,
            config,
            group_depth: 0,
            recursion_depth: 0,
            row_break_skips_spaces: false,
            math_stop: MathStop::Default,
            diagnostics: Vec::new(),
            context_stack: Vec::new(),
            env_stack: Vec::new(),
            env_stack_base: 0,
        }
    }

    /// Whether `\\` skips spaces before an optional `[...]` argument in the
    /// current environment.
    #[inline]
    pub(crate) fn row_break_skips_spaces(&self) -> bool {
        self.row_break_skips_spaces
    }

    /// Extra stop policy for the current math content loop.
    #[inline]
    pub(crate) fn math_stop(&self) -> MathStop {
        self.math_stop
    }

    /// Current explicit `{...}` brace group nesting depth within this window.
    #[cfg(test)]
    #[inline]
    pub(crate) fn group_depth(&self) -> usize {
        self.group_depth
    }

    /// Current global recursion depth across all windows.
    #[cfg(test)]
    #[inline]
    pub(crate) fn recursion_depth(&self) -> usize {
        self.recursion_depth
    }

    /// Active open environments visible in the current sub-window.
    #[inline]
    pub(crate) fn active_envs(&self) -> &[OpenEnv<'src>] {
        &self.env_stack[self.env_stack_base..]
    }

    /// Returns `true` if the innermost active environment already had an EOF
    /// `UnclosedEnvironment` diagnostic emitted during its body.
    #[inline]
    pub(crate) fn active_env_eof_reported(&self) -> bool {
        self.active_envs()
            .last()
            .is_some_and(|env| env.eof_reported_at.is_some())
    }

    /// Record that an EOF `UnclosedEnvironment` for `name` was emitted at
    /// `diag_idx` on all matching active environments in the current sub-window.
    #[inline]
    pub(crate) fn mark_active_envs_eof_reported(&mut self, name: &str, diag_idx: usize) {
        let base = self.env_stack_base;
        for env in &mut self.env_stack[base..] {
            if env.name.as_ref() == name {
                env.eof_reported_at.get_or_insert(diag_idx);
            }
        }
    }

    /// Borrow all diagnostics emitted so far.
    #[inline]
    pub(crate) fn diagnostics(&self) -> &[ParseDiagnostic] {
        &self.diagnostics
    }

    /// Mutably borrow diagnostics emitted so far for localized post-adjustment.
    #[inline]
    pub(crate) fn diagnostics_mut(&mut self) -> &mut Vec<ParseDiagnostic> {
        &mut self.diagnostics
    }

    /// Consume the parser and return its raw emitted diagnostics.
    #[inline]
    pub(crate) fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    /// Slice the original source snippet for `span`, defaulting to empty on
    /// out-of-range offsets.
    #[inline]
    pub(crate) fn slice_snippet(&self, span: Span) -> String {
        self.src.get(span.start..span.end).unwrap_or("").to_string()
    }

    /// Push a dynamic-end diagnostic context starting at byte offset `start`
    /// for the duration of `f`.
    pub(crate) fn with_context<R>(
        &mut self,
        label: &'static str,
        start: usize,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.context_stack.push(ContextFrame {
            label,
            start,
            fixed_end: None,
        });
        let result = f(self);
        self.context_stack.pop();
        result
    }

    /// Push a fixed-span diagnostic context (e.g. a pre-scanned argument
    /// surface span) for the duration of `f`.
    pub(crate) fn with_context_span<R>(
        &mut self,
        label: &'static str,
        span: Span,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.context_stack.push(ContextFrame {
            label,
            start: span.start,
            fixed_end: Some(span.end),
        });
        let result = f(self);
        self.context_stack.pop();
        result
    }

    /// Execute `f` over an isolated sub-window cursor.
    ///
    /// Resets `row_break_skips_spaces`, `math_stop`, and `group_depth` to
    /// window defaults and raises `env_stack_base` to isolate outer
    /// environments, while keeping `recursion_depth` globally cumulative. All
    /// saved state is restored symmetrically on return.
    pub(crate) fn with_sub_cursor<R>(
        &mut self,
        sub: Cursor<'tok, 'src>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let saved_cursor = self.cursor;
        let saved_row_break = self.row_break_skips_spaces;
        let saved_math_stop = self.math_stop;
        let saved_group_depth = self.group_depth;
        let saved_env_base = self.env_stack_base;

        self.cursor = sub;
        self.row_break_skips_spaces = false;
        self.math_stop = MathStop::Default;
        self.group_depth = 0;
        self.env_stack_base = self.env_stack.len();

        let result = f(self);

        self.cursor = saved_cursor;
        self.row_break_skips_spaces = saved_row_break;
        self.math_stop = saved_math_stop;
        self.group_depth = saved_group_depth;
        self.env_stack_base = saved_env_base;
        result
    }

    /// Execute `f` inside an open environment, pushing `env` onto `env_stack`
    /// and updating `row_break_skips_spaces`.
    pub(crate) fn with_env<R>(
        &mut self,
        env: OpenEnv<'src>,
        row_break_skips_spaces: bool,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let saved_row_break = self.row_break_skips_spaces;
        self.env_stack.push(env);
        self.row_break_skips_spaces = row_break_skips_spaces;
        let result = f(self);
        self.env_stack.pop();
        self.row_break_skips_spaces = saved_row_break;
        result
    }

    /// Execute `f` with an overridden [`MathStop`] policy.
    pub(crate) fn with_math_stop<R>(
        &mut self,
        stop: MathStop,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let saved = self.math_stop;
        self.math_stop = stop;
        let result = f(self);
        self.math_stop = saved;
        result
    }

    /// Guard explicit `{...}` group nesting against `config.max_group_depth`.
    ///
    /// Returns `None` without calling `f` when the limit has already been
    /// reached.
    pub(crate) fn with_group<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> Option<R> {
        if self.group_depth >= self.config.max_group_depth {
            return None;
        }
        self.group_depth += 1;
        let result = f(self);
        self.group_depth -= 1;
        Some(result)
    }

    /// Guard recursive parser entry points against [`MAX_RECURSION_DEPTH`].
    ///
    /// On overflow, emits [`ParseDiagnosticKind::MaxGroupDepthExceeded`] with
    /// `"max recursion depth ({MAX_RECURSION_DEPTH}) exceeded"` and returns
    /// `Err(ParseAbort { hint: RecoveryHint::Fatal })`.
    pub(crate) fn with_recursion<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> PResult<R>,
    ) -> PResult<R> {
        if self.recursion_depth >= MAX_RECURSION_DEPTH {
            let span = self.cursor.peek_span();
            return self.fail_err(
                diagnostics::max_recursion_depth_exceeded(MAX_RECURSION_DEPTH, span),
                RecoveryHint::Fatal,
            );
        }
        self.recursion_depth += 1;
        let result = f(self);
        self.recursion_depth -= 1;
        result
    }

    /// Capture the current cursor index and diagnostic count.
    #[inline]
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.cursor.pos(),
            diag_len: self.diagnostics.len(),
            #[cfg(debug_assertions)]
            context_depth: self.context_stack.len(),
            #[cfg(debug_assertions)]
            env_depth: self.env_stack.len(),
        }
    }

    /// Restore the cursor and truncate diagnostics emitted since `cp`.
    #[inline]
    pub(crate) fn rewind(&mut self, cp: Checkpoint) {
        #[cfg(debug_assertions)]
        {
            debug_assert_eq!(
                self.context_stack.len(),
                cp.context_depth,
                "rewind called across a context_stack scope boundary"
            );
            debug_assert_eq!(
                self.env_stack.len(),
                cp.env_depth,
                "rewind called across an env_stack scope boundary"
            );
        }
        self.cursor.set_pos(cp.pos);
        self.diagnostics.truncate(cp.diag_len);
        for env in &mut self.env_stack {
            if env.eof_reported_at.is_some_and(|idx| idx >= cp.diag_len) {
                env.eof_reported_at = None;
            }
        }
    }

    /// Snapshot active diagnostic contexts (innermost-first). Open contexts
    /// extend through `error_end`; fixed-span contexts preserve their
    /// pre-scanned span.
    fn snapshot_contexts(&self, error_end: usize) -> Vec<ParseDiagnosticContext> {
        self.context_stack
            .iter()
            .rev()
            .map(|frame| ParseDiagnosticContext {
                label: frame.label.to_string(),
                span: Span::new(
                    frame.start,
                    frame
                        .fixed_end
                        .unwrap_or_else(|| error_end.max(frame.start)),
                ),
            })
            .collect()
    }

    /// Populate `diag.contexts` from the live context stack (if empty) and
    /// append `diag` without deduplicating so `diagnostics[cp.diag_len]`
    /// remains a stable watermark during item recovery.
    #[inline]
    pub(crate) fn emit_diag(&mut self, mut diag: ParseDiagnostic) {
        if diag.contexts.is_empty() {
            diag.contexts = self.snapshot_contexts(diag.span.end);
        }
        self.diagnostics.push(diag);
    }

    /// Emit `diag` with live context snapshots and return a [`ParseAbort`]
    /// carrying `hint`.
    #[inline]
    pub(crate) fn fail(&mut self, diag: ParseDiagnostic, hint: RecoveryHint) -> ParseAbort {
        self.emit_diag(diag);
        ParseAbort { hint }
    }

    /// Convenience wrapper around [`Self::fail`] returning `Err(ParseAbort)`.
    #[inline]
    pub(crate) fn fail_err<T>(&mut self, diag: ParseDiagnostic, hint: RecoveryHint) -> PResult<T> {
        Err(self.fail(diag, hint))
    }

    /// Consume `expected_tok` and return its byte span, or emit a
    /// [`ParseDiagnosticKind::RawExpectedFound`] diagnostic labelled `label`.
    pub(crate) fn expect(
        &mut self,
        expected_tok: Token<'static>,
        label: &'static str,
    ) -> PResult<Span> {
        if let Some(span) = self.cursor.eat(expected_tok) {
            Ok(span)
        } else {
            let span = self.cursor.peek_span();
            let found = self.cursor.peek();
            Err(self.emit_expected(span, &[label], found))
        }
    }

    /// Emit a [`ParseDiagnosticKind::RawExpectedFound`] diagnostic and return
    /// a [`ParseAbort`] with [`RecoveryHint::Skip`].
    pub(crate) fn emit_expected(
        &mut self,
        span: Span,
        expected: &[&str],
        found: Option<Token<'_>>,
    ) -> ParseAbort {
        self.fail(
            diagnostics::raw_expected_found(span, expected, found),
            RecoveryHint::Skip,
        )
    }

    /// Handle EOF unclosed-construct upgrades per §5.2:
    ///
    /// 1. When `comment_check_start` is provided and the remaining source slice
    ///    contains an unescaped `%`, emit `CommentTruncatedArgument`.
    /// 2. Otherwise, when `active_envs()` is non-empty, emit
    ///    `UnclosedEnvironment` for the innermost active environment.
    ///
    /// Returns `true` if an upgraded diagnostic was emitted (only possible at
    /// end of window).
    pub(crate) fn try_emit_eof_upgrade(
        &mut self,
        span: Span,
        comment_check_start: Option<usize>,
    ) -> bool {
        if !self.cursor.is_at_end() {
            return false;
        }
        if let Some(start) = comment_check_start
            && self
                .src
                .get(start..)
                .is_some_and(diagnostics::has_unescaped_percent)
        {
            self.emit_diag(diagnostics::comment_truncated_argument(span));
            return true;
        }
        if let Some(top_env) = self.active_envs().last() {
            let env_name = top_env.name.as_ref().to_owned();
            let diag_idx = self.diagnostics.len();
            self.emit_diag(diagnostics::unclosed_environment(&env_name, span, None));
            self.mark_active_envs_eof_reported(&env_name, diag_idx);
            return true;
        }
        false
    }

    /// Emit either an EOF upgrade (`CommentTruncatedArgument` /
    /// `UnclosedEnvironment`) or fall back to [`Self::emit_expected`].
    pub(crate) fn emit_unclosed_at_eof_or_expected(
        &mut self,
        span: Span,
        expected: &[&str],
        found: Option<Token<'_>>,
        comment_check_start: Option<usize>,
    ) -> ParseAbort {
        if found.is_none() && self.try_emit_eof_upgrade(span, comment_check_start) {
            return ParseAbort {
                hint: RecoveryHint::Skip,
            };
        }
        self.emit_expected(span, expected, found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex_source;
    use crate::parse::ParseDiagnosticKind;

    fn make_env(name: &'static str) -> OpenEnv<'static> {
        OpenEnv::new(
            Cow::Borrowed(name),
            Span::new(0, 6),
            Span::new(6, 6 + name.len() + 2),
            ContentMode::Math,
        )
    }

    #[test]
    fn with_sub_cursor_isolates_window_state_preserves_recursion_and_restores_on_err() {
        let src = "{a}";
        let stream = lex_source(src).unwrap();
        let cursor = Cursor::from_stream(&stream, src.len());
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut parser = TokenParser::new(cursor, src, &ctx, &config);

        let outer_result: PResult<()> = parser.with_env(make_env("array"), true, |p| {
            p.with_math_stop(MathStop::ControlParen, |p| {
                p.with_group(|p| {
                    p.with_recursion(|p| {
                        assert_eq!(p.group_depth(), 1);
                        assert_eq!(p.recursion_depth(), 1);
                        assert!(p.row_break_skips_spaces());
                        assert_eq!(p.math_stop(), MathStop::ControlParen);
                        assert_eq!(p.active_envs().len(), 1);

                        let sub = p.cursor.sub_window(1..2, 1);
                        let sub_res: PResult<()> = p.with_sub_cursor(sub, |p| {
                            // Window-scoped state is reset and outer env stack is isolated.
                            assert_eq!(p.group_depth(), 0);
                            assert!(!p.row_break_skips_spaces());
                            assert_eq!(p.math_stop(), MathStop::Default);
                            assert!(p.active_envs().is_empty());
                            // Global recursion depth stays cumulative across sub-windows.
                            assert_eq!(p.recursion_depth(), 1);
                            assert_eq!(p.cursor.peek(), Some(Token::Char('a')));

                            // Mutate sub-window state and return Err to verify restoration.
                            let _ = p.with_group(|p| {
                                assert_eq!(p.group_depth(), 1);
                            });
                            Err(ParseAbort {
                                hint: RecoveryHint::Skip,
                            })
                        });
                        assert!(sub_res.is_err());

                        // Outer state is symmetrically restored even after Err.
                        assert_eq!(p.cursor.pos(), 0);
                        assert_eq!(p.cursor.len(), 3);
                        assert_eq!(p.group_depth(), 1);
                        assert_eq!(p.recursion_depth(), 1);
                        assert!(p.row_break_skips_spaces());
                        assert_eq!(p.math_stop(), MathStop::ControlParen);
                        assert_eq!(p.active_envs().len(), 1);
                        assert_eq!(p.active_envs()[0].name.as_ref(), "array");
                        Ok(())
                    })
                })
                .expect("group within limit")
            })
        });
        assert!(outer_result.is_ok());
        assert_eq!(parser.group_depth(), 0);
        assert_eq!(parser.recursion_depth(), 0);
        assert!(!parser.row_break_skips_spaces());
        assert_eq!(parser.math_stop(), MathStop::Default);
        assert!(parser.active_envs().is_empty());
    }

    #[test]
    fn checkpoint_rewind_restores_cursor_and_truncates_diagnostics() {
        let src = "a + b";
        let stream = lex_source(src).unwrap();
        let cursor = Cursor::from_stream(&stream, src.len());
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut parser = TokenParser::new(cursor, src, &ctx, &config);

        parser.emit_diag(diagnostics::unknown_command("keep", Span::new(0, 1)));
        let cp = parser.checkpoint();
        assert_eq!(cp.pos, 0);
        assert_eq!(cp.diag_len, 1);

        parser.cursor.bump();
        parser.cursor.bump();
        parser.emit_diag(diagnostics::unknown_command("drop", Span::new(2, 3)));
        assert_eq!(parser.cursor.pos(), 2);
        assert_eq!(parser.diagnostics().len(), 2);

        parser.rewind(cp);
        assert_eq!(parser.cursor.pos(), 0);
        assert_eq!(parser.diagnostics().len(), 1);
        assert_eq!(parser.diagnostics()[0].message, r"Unknown command: \keep");
    }

    #[test]
    fn with_recursion_enforces_max_recursion_depth() {
        let src = "x";
        let stream = lex_source(src).unwrap();
        let cursor = Cursor::from_stream(&stream, src.len());
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut parser = TokenParser::new(cursor, src, &ctx, &config);

        fn recurse(p: &mut TokenParser<'_, '_, '_>, remaining: usize) -> PResult<()> {
            p.with_recursion(|p| {
                if remaining == 0 {
                    Ok(())
                } else {
                    recurse(p, remaining - 1)
                }
            })
        }

        // Exactly MAX_RECURSION_DEPTH nested frames succeed (0..MAX_RECURSION_DEPTH-1).
        assert!(recurse(&mut parser, MAX_RECURSION_DEPTH - 1).is_ok());
        assert_eq!(parser.recursion_depth(), 0);
        assert!(parser.diagnostics().is_empty());

        // One level deeper trips the guard with a fatal abort.
        let abort = recurse(&mut parser, MAX_RECURSION_DEPTH).unwrap_err();
        assert_eq!(abort.hint, RecoveryHint::Fatal);
        assert_eq!(parser.recursion_depth(), 0);
        assert_eq!(parser.diagnostics().len(), 1);
        assert_eq!(
            parser.diagnostics()[0].kind,
            Some(ParseDiagnosticKind::MaxGroupDepthExceeded)
        );
        assert_eq!(
            parser.diagnostics()[0].message,
            format!("max recursion depth ({MAX_RECURSION_DEPTH}) exceeded")
        );
    }

    #[test]
    fn eof_upgrade_prefers_comment_truncation_then_active_environment() {
        let src = r"\begin{matrix} {a";
        let stream = lex_source(src).unwrap();
        let mut cursor = Cursor::from_stream(&stream, src.len());
        cursor.set_pos(cursor.len()); // EOF
        let ctx = ParseContext::empty();
        let config = ParseConfig::STRICT;
        let mut parser = TokenParser::new(cursor, src, &ctx, &config);

        parser.with_env(make_env("matrix"), false, |p| {
            let upgraded = p.try_emit_eof_upgrade(Span::point(src.len()), Some(15));
            assert!(upgraded);
        });
        assert_eq!(parser.diagnostics().len(), 1);
        assert_eq!(
            parser.diagnostics()[0].kind,
            Some(ParseDiagnosticKind::UnclosedEnvironment)
        );
        assert_eq!(
            parser.diagnostics()[0].message,
            r"Environment matrix missing closing \end{matrix}"
        );
        assert_eq!(parser.diagnostics()[0].expected, vec![r"\end{matrix}"]);
        assert_eq!(parser.diagnostics()[0].found, None);

        // When an unescaped `%` is present, CommentTruncatedArgument wins over UnclosedEnvironment.
        let comment_src = r"\begin{matrix}\frac{%comment";
        let comment_stream = lex_source(comment_src).unwrap();
        let mut comment_cursor = Cursor::from_stream(&comment_stream, comment_src.len());
        comment_cursor.set_pos(comment_cursor.len());
        let mut comment_parser = TokenParser::new(comment_cursor, comment_src, &ctx, &config);
        comment_parser.with_env(make_env("matrix"), false, |p| {
            let upgraded = p.try_emit_eof_upgrade(Span::new(19, 20), Some(19));
            assert!(upgraded);
        });
        assert_eq!(comment_parser.diagnostics().len(), 1);
        assert_eq!(
            comment_parser.diagnostics()[0].kind,
            Some(ParseDiagnosticKind::CommentTruncatedArgument)
        );
    }
}
