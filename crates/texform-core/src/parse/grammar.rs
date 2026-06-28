//! Chumsky-based parser for LaTeX formulas.
//!
//! This module builds the syntax parser around [`SyntaxNode`] while carrying
//! parser-private span and diagnostic metadata in its internal tracked-node
//! wrapper. Top-level math/text block parsers first produce implicit content
//! groups, then promote those groups to [`SyntaxNode::Root`] before returning
//! results to callers.
//!
//! # Parser layers
//!
//! 1. **Leaf and atom parsers** — math/text characters, escaped symbols,
//!    active characters, prime atoms, explicit groups, `\left...\right` groups,
//!    prefix/declarative/delimiter-control commands, environments, and unknown
//!    commands.
//! 2. **Script handling** — math atoms are wrapped with `_`, `^`, and prime
//!    handling, including empty-base scripts and prime/exponent merge rules.
//! 3. **Argument parsing** — command/environment argument slots are parsed in
//!    `arguments.rs`; content arguments re-enter the math or text content
//!    parsers.
//! 4. **Content parsing** — math content parses item sequences with optional
//!    infix tails; text content parses ordinary text-mode item sequences.
//! 5. **Mode entry** — math/text block parsers wrap content in implicit groups,
//!    and root promotion turns the outer group into [`SyntaxNode::Root`].
//!
//! Math and text content parsers are mutually recursive: math commands may
//! contain text arguments, and text content may contain inline math. The
//! recursion is resolved with [`chumsky::recursive`].

#[path = "arguments.rs"]
mod arguments;

use chumsky::{
    input::{Cursor, InputRef, Stream},
    label::LabelError,
    prelude::*,
};
use logos::Logos;

use crate::knowledge::{ActiveCommandRecord, ActiveEnvironmentRecord, ArgSpec, CommandKind};
use crate::lexer::Token;
use crate::parse::{ParseConfig, ParseContext, ParseDiagnosticKind, ParserState};
use texform_interface::syntax_node::{ArgumentSlot, ContentMode, Delimiter, GroupKind, SyntaxNode};

use self::arguments::{
    TrackedArgumentSlot, argument_parser, collect_braced_tokens, collect_optional_bracketed_tokens,
};

/// A value paired with its source byte span.
pub(crate) type Spanned<T> = (T, SimpleSpan);

// ============================================================================
// Node Span Tracking Infrastructure
// ============================================================================

// Path segment constants for node span IDs.
const CHILD: &str = "child";
const ARG: &str = "arg";
const CONTENT: &str = "content";
const LEFT: &str = "left";
const RIGHT: &str = "right";
const BODY: &str = "body";
const BASE: &str = "base";
const SUB: &str = "sub";
const SUP: &str = "sup";

/// Descendant span keyed by a path relative to the current tracked node.
#[derive(Debug, Clone)]
pub(crate) struct RelativeSpanEntry {
    pub path: String,
    pub span: SimpleSpan,
}

/// Parser-private wrapper: a syntax node bundled with its span and descendant
/// path records. This is the only tracking structure in the parser; it is NOT
/// a mirror of `SyntaxNode` — the node is constructed normally, and the
/// records are assembled via a handful of decompose/prefix helpers.
#[derive(Debug, Clone)]
pub(crate) struct TrackedNode {
    pub node: SyntaxNode,
    pub span: SimpleSpan,
    pub records: Vec<RelativeSpanEntry>,
    diagnostics: Vec<Rich<'static, Token>>,
}

impl TrackedNode {
    /// Wrap a syntax node with no descendant records.
    pub(crate) fn leaf(node: SyntaxNode, span: SimpleSpan) -> Self {
        Self {
            node,
            span,
            records: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn with_diagnostics(mut self, diagnostics: Vec<Rich<'static, Token>>) -> Self {
        self.diagnostics.extend(diagnostics);
        self
    }

    /// Shift all spans by `offset` bytes. Used when content was re-parsed from
    /// a token sub-stream whose positions start at 0.
    pub(crate) fn offset(self, offset: usize) -> Self {
        TrackedNode {
            node: self.node,
            span: SimpleSpan::new((), self.span.start + offset..self.span.end + offset),
            records: self
                .records
                .into_iter()
                .map(|e| RelativeSpanEntry {
                    path: e.path,
                    span: SimpleSpan::new((), e.span.start + offset..e.span.end + offset),
                })
                .collect(),
            diagnostics: self
                .diagnostics
                .into_iter()
                .map(|err| shift_owned_rich_span(err, offset))
                .collect(),
        }
    }

    /// Finalize into the root-prefixed record list consumed by `ParseResult`.
    ///
    /// Promotes the tracked top-level implicit group into a real
    /// `SyntaxNode::Root` so downstream consumers never see the root as a
    /// regular group. Span paths continue to start with `root` and
    /// `root.child.N` before they are attached to `Document` node handles.
    pub(crate) fn finish_root(
        self,
    ) -> (
        SyntaxNode,
        SimpleSpan,
        Vec<RelativeSpanEntry>,
        Vec<Rich<'static, Token>>,
    ) {
        let root_node = match self.node {
            node @ SyntaxNode::Root { .. } => node,
            SyntaxNode::Group {
                mode,
                kind: GroupKind::Implicit,
                children,
            } => SyntaxNode::Root { mode, children },
            other => panic!(
                "top-level parser must finish as implicit group or root, got {:?}",
                other
            ),
        };

        let mut records = vec![RelativeSpanEntry {
            path: "root".to_string(),
            span: self.span,
        }];
        for entry in self.records {
            records.push(RelativeSpanEntry {
                path: format!("root.{}", entry.path),
                span: entry.span,
            });
        }
        (root_node, self.span, records, self.diagnostics)
    }

    /// Extract syntax nodes and `child.N` records from tracked children.
    fn decompose_children(
        children: Vec<TrackedNode>,
    ) -> (
        Vec<SyntaxNode>,
        Vec<RelativeSpanEntry>,
        Vec<Rich<'static, Token>>,
    ) {
        let mut records = Vec::new();
        let mut diagnostics = Vec::new();
        let mut nodes = Vec::with_capacity(children.len());
        for (index, child) in children.iter().enumerate() {
            records.extend(prefix_records(&format!("{CHILD}.{index}"), child));
            extend_unique_diagnostics(&mut diagnostics, child.diagnostics.iter().cloned());
        }
        for child in children {
            nodes.push(child.node);
        }
        (nodes, records, diagnostics)
    }

    /// Extract argument slots and `arg.N` / `arg.N.content` records.
    fn decompose_args(
        slots: Vec<TrackedArgumentSlot>,
    ) -> (
        Vec<ArgumentSlot>,
        Vec<RelativeSpanEntry>,
        Vec<Rich<'static, Token>>,
    ) {
        let mut records = Vec::new();
        let mut diagnostics = Vec::new();
        for (index, arg) in slots.iter().enumerate() {
            if let Some(arg_span) = arg.span {
                let arg_path = format!("{ARG}.{index}");
                records.push(RelativeSpanEntry {
                    path: arg_path.clone(),
                    span: arg_span,
                });
                if let Some(content) = &arg.content {
                    records.extend(prefix_records(&format!("{arg_path}.{CONTENT}"), content));
                    extend_unique_diagnostics(
                        &mut diagnostics,
                        content.diagnostics.iter().cloned(),
                    );
                }
            }
        }
        let slots = slots.into_iter().map(|a| a.slot).collect();
        (slots, records, diagnostics)
    }

    /// `fold_items` equivalent that preserves span records.
    ///
    /// - 0 items → empty implicit group
    /// - 1 item  → unwrap (reuse child's records, override span)
    /// - N items → implicit group with `child.N` records
    fn fold(mode: ContentMode, items: Vec<TrackedNode>, span: SimpleSpan) -> Self {
        match items.len() {
            0 => TrackedNode::leaf(
                SyntaxNode::Group {
                    mode,
                    kind: GroupKind::Implicit,
                    children: vec![],
                },
                span,
            ),
            1 => {
                let child = items.into_iter().next().unwrap();
                TrackedNode { span, ..child }
            }
            _ => {
                let (nodes, records, diagnostics) = TrackedNode::decompose_children(items);
                TrackedNode {
                    node: SyntaxNode::Group {
                        mode,
                        kind: GroupKind::Implicit,
                        children: nodes,
                    },
                    span,
                    records,
                    diagnostics,
                }
            }
        }
    }
}

/// Compute the aggregate span covering all items (first.start .. last.end).
/// Returns a zero-width span at `fallback` when the slice is empty.
fn items_span(items: &[TrackedNode], fallback: usize) -> SimpleSpan {
    match (items.first(), items.last()) {
        (Some(first), Some(last)) => SimpleSpan::new((), first.span.start..last.span.end),
        _ => SimpleSpan::new((), fallback..fallback),
    }
}

/// Create records for a child node: one entry for the child itself, plus all
/// its descendants prefixed under the given path. Takes a reference to avoid
/// unnecessary cloning at callsites.
fn prefix_records(prefix: &str, child: &TrackedNode) -> Vec<RelativeSpanEntry> {
    let mut records = vec![RelativeSpanEntry {
        path: prefix.to_string(),
        span: child.span,
    }];
    for entry in &child.records {
        records.push(RelativeSpanEntry {
            path: format!("{prefix}.{}", entry.path),
            span: entry.span,
        });
    }
    records
}

// Nested content reparses can surface the same inner diagnostic at multiple wrapper levels.
fn extend_unique_diagnostics(
    diagnostics: &mut Vec<Rich<'static, Token>>,
    incoming: impl IntoIterator<Item = Rich<'static, Token>>,
) {
    for candidate in incoming {
        if diagnostics
            .iter()
            .any(|existing| rich_diagnostics_match(existing, &candidate))
        {
            continue;
        }
        diagnostics.push(candidate);
    }
}

fn rich_diagnostics_match(left: &Rich<'static, Token>, right: &Rich<'static, Token>) -> bool {
    left.span() == right.span()
        && rich_reason_key(left) == rich_reason_key(right)
        && rich_contexts_key(left) == rich_contexts_key(right)
}

fn rich_reason_key(err: &Rich<'static, Token>) -> (Option<String>, Vec<String>, Option<String>) {
    match err.reason() {
        chumsky::error::RichReason::Custom(message) => (
            Some(
                ParseDiagnosticKind::split_message(message.as_str())
                    .1
                    .to_string(),
            ),
            Vec::new(),
            None,
        ),
        chumsky::error::RichReason::ExpectedFound { expected, found } => (
            None,
            expected.iter().map(|pattern| pattern.to_string()).collect(),
            found.as_ref().map(|token| token.to_string()),
        ),
    }
}

fn rich_contexts_key(err: &Rich<'static, Token>) -> Vec<(String, SimpleSpan)> {
    err.contexts()
        .map(|(label, span)| (label.to_string(), *span))
        .collect()
}

pub(crate) fn diagnostic_kind(err: &Rich<'_, Token>) -> Option<ParseDiagnosticKind> {
    match err.reason() {
        chumsky::error::RichReason::Custom(message) => {
            ParseDiagnosticKind::split_message(message.as_str()).0
        }
        chumsky::error::RichReason::ExpectedFound { .. } => None,
    }
    .or_else(|| {
        err.contexts()
            .find_map(|(label, _)| ParseDiagnosticKind::from_context_label(&label.to_string()))
    })
}

fn add_contexts_to_error<'a>(
    mut err: Rich<'a, Token>,
    contexts: Vec<(String, SimpleSpan)>,
) -> Rich<'a, Token> {
    for (label, span) in contexts {
        <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, String>>::in_context(
            &mut err, label, span,
        );
    }
    err
}

fn error_contexts(err: &Rich<'_, Token>) -> Vec<(String, SimpleSpan)> {
    err.contexts()
        .map(|(label, span)| (label.to_string(), *span))
        .collect()
}

fn has_diagnostic_kind(err: &Rich<'_, Token>, kind: ParseDiagnosticKind) -> bool {
    diagnostic_kind(err) == Some(kind)
}

pub(crate) fn custom_error<'a>(
    span: SimpleSpan,
    msg: impl ToString,
    kind: ParseDiagnosticKind,
) -> Rich<'a, Token> {
    let mut err = Rich::custom(span, kind.wrap_message(msg.to_string()));
    attach_diagnostic_kind(&mut err, kind, span);
    err
}

pub(crate) fn with_default_diagnostic_kind<'a>(
    err: Rich<'a, Token>,
    kind: ParseDiagnosticKind,
) -> Rich<'a, Token> {
    if diagnostic_kind(&err).is_some() {
        err
    } else {
        with_diagnostic_kind(err, kind)
    }
}

pub(crate) fn with_diagnostic_kind<'a>(
    err: Rich<'a, Token>,
    kind: ParseDiagnosticKind,
) -> Rich<'a, Token> {
    let span = *err.span();
    let contexts = error_contexts(&err);
    if let chumsky::error::RichReason::Custom(message) = err.reason() {
        let (_, public_message) = ParseDiagnosticKind::split_message(message.as_str());
        let mut rebuilt = Rich::custom(span, kind.wrap_message(public_message));
        attach_diagnostic_kind(&mut rebuilt, kind, span);
        return add_contexts_to_error(rebuilt, contexts);
    }

    let mut err = err;
    attach_diagnostic_kind(&mut err, kind, span);
    err
}

fn attach_diagnostic_kind<'a>(
    err: &mut Rich<'a, Token>,
    kind: ParseDiagnosticKind,
    span: SimpleSpan,
) {
    <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, String>>::in_context(
        err,
        kind.context_label(),
        span,
    );
}

pub(crate) fn shift_owned_rich_span(
    err: Rich<'static, Token>,
    offset: usize,
) -> Rich<'static, Token> {
    let shift = |span: SimpleSpan| SimpleSpan::new((), span.start + offset..span.end + offset);
    let original_span = err.span();

    let mut rebuilt = match err.reason() {
        chumsky::error::RichReason::Custom(message) => {
            Rich::custom(shift(*original_span), message.clone())
        }
        chumsky::error::RichReason::ExpectedFound { expected, found } => {
            <Rich<'static, Token> as LabelError<
                'static,
                TokenStream<'static>,
                chumsky::error::RichPattern<'static, Token>,
            >>::expected_found(
                expected.iter().cloned(),
                found.clone(),
                shift(*original_span),
            )
        }
    };

    for (label, span) in err.contexts() {
        <Rich<'static, Token> as LabelError<'static, TokenStream<'static>, String>>::in_context(
            &mut rebuilt,
            label.to_string(),
            shift(*span),
        );
    }

    rebuilt
}

// Keep the original error location while attaching the outer argument wrapper as context.
fn with_argument_context<'a>(
    err: Rich<'a, Token>,
    label: &'static str,
    span: SimpleSpan,
) -> Rich<'a, Token> {
    let mut err = clone_rich_error(&err);
    <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(&mut err, label, span);
    err
}

fn parse_argument_slots<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    state: &'src ParserState<'src>,
    math_content: ContentParser<'src>,
    text_content: ContentParser<'src>,
    args: &'static [ArgSpec],
    context_label: &'static str,
) -> Result<Vec<TrackedArgumentSlot>, Rich<'src, Token>> {
    let ws = insignificant_whitespace();
    let mut slots = Vec::with_capacity(args.len());

    for spec in args {
        if !spec.no_leading_space {
            let _ = input.parse(ws.clone());
        }

        let arg_start = input.cursor();
        let parser = argument_parser(state, math_content.clone(), text_content.clone(), spec);
        let mut arg = input.parse(parser).map_err(|err| {
            let original_kind = diagnostic_kind(&err);
            let arg_span = err
                .contexts()
                .next()
                .map(|(_, span)| *span)
                .unwrap_or_else(|| input.span_from_cursor(&arg_start));
            let err = with_argument_context(err, context_label, arg_span);
            if let Some(kind) = original_kind {
                with_diagnostic_kind(err, kind)
            } else {
                with_default_diagnostic_kind(err, ParseDiagnosticKind::ArgumentValidation)
            }
        })?;
        if let Some(argument) = arg.slot.as_mut() {
            argument.no_leading_space = spec.no_leading_space;
        }
        slots.push(arg);
    }

    Ok(slots)
}

fn clone_rich_error<'a>(err: &Rich<'a, Token>) -> Rich<'a, Token> {
    let mut cloned = match err.reason() {
        chumsky::error::RichReason::Custom(message) => Rich::custom(*err.span(), message.clone()),
        chumsky::error::RichReason::ExpectedFound { expected, found } => {
            <Rich<'a, Token> as LabelError<
                'a,
                TokenStream<'a>,
                chumsky::error::RichPattern<'a, Token>,
            >>::expected_found(expected.iter().cloned(), found.clone(), *err.span())
        }
    };

    for (label, span) in err.contexts() {
        <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, String>>::in_context(
            &mut cloned,
            label.to_string(),
            *span,
        );
    }

    cloned
}

/// Extension trait: convert any `Parser<..., SyntaxNode, ...>` into one that
/// produces `TrackedNode` by capturing the span. Use on leaf parsers only.
trait ParserTrackedExt<'a>: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Sized {
    fn tracked(self) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
    where
        Self: Clone + 'a,
    {
        self.map_with(|node, e| TrackedNode::leaf(node, e.span()))
    }
}

impl<'a, P> ParserTrackedExt<'a> for P where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>>
{
}

/// The concrete mapped input type fed to all chumsky parsers in this module.
pub(crate) type TokenStream<'a> = chumsky::input::MappedInput<
    Token,
    SimpleSpan,
    Stream<std::vec::IntoIter<(Token, SimpleSpan)>>,
    fn((Token, SimpleSpan)) -> (Token, SimpleSpan),
>;

/// Chumsky error extra carrying rich diagnostics.
pub(crate) type ParserError<'a> = extra::Err<Rich<'a, Token>>;

/// Mutable reference to the input stream used by imperative (`custom`) parsers.
pub(crate) type ParserInput<'src, 'parse> =
    InputRef<'src, 'parse, TokenStream<'src>, ParserError<'src>>;

/// Lex a source string and wrap the result as a chumsky [`TokenStream`].
///
/// # Panics
///
/// Panics if the lexer encounters an unrecognizable byte (catcode 9/15).
pub(crate) fn build_token_stream(src: &str) -> TokenStream<'_> {
    let tokens: Vec<(Token, SimpleSpan)> = Token::lexer(src)
        .spanned()
        .map(|(tok, span)| {
            let tok = tok.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (tok, SimpleSpan::from(span))
        })
        .collect();
    let eoi: SimpleSpan = SimpleSpan::new((), src.len()..src.len());
    let stream = Stream::from_iter(tokens);

    fn identity(pair: (Token, SimpleSpan)) -> (Token, SimpleSpan) {
        pair
    }

    stream.map(
        eoi,
        identity as fn((Token, SimpleSpan)) -> (Token, SimpleSpan),
    )
}

/// Ergonomic helpers for building spans and custom errors in imperative
/// (`custom`) parsers that work through [`ParserInput`].
pub(crate) trait ParserInputExt<'src, 'parse> {
    /// Compute the byte span from `start` cursor to the current position.
    fn span_from_cursor(&mut self, start: &Cursor<'src, 'parse, TokenStream<'src>>) -> SimpleSpan;

    /// Build a custom error spanning from `start` to the current position.
    fn err_since(
        &mut self,
        start: &Cursor<'src, 'parse, TokenStream<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token>;

    /// Build an error for the next token without consuming it.
    ///
    /// Falls back to a point span at EOF.
    fn err_peek_or_point(
        &mut self,
        start: &Cursor<'src, 'parse, TokenStream<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token>;
}

impl<'src, 'parse> ParserInputExt<'src, 'parse> for ParserInput<'src, 'parse> {
    #[inline]
    fn span_from_cursor(&mut self, start: &Cursor<'src, 'parse, TokenStream<'src>>) -> SimpleSpan {
        self.span_since(start)
    }

    #[inline]
    fn err_since(
        &mut self,
        start: &Cursor<'src, 'parse, TokenStream<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token> {
        Rich::custom(self.span_from_cursor(start), msg)
    }

    #[inline]
    fn err_peek_or_point(
        &mut self,
        start: &Cursor<'src, 'parse, TokenStream<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token> {
        let span = self.span_since(start);
        Rich::custom(span, msg)
    }
}

/// Boxed parser producing a list of tracked child nodes (group content).
type ContentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>>;
/// Boxed parser producing a single tracked syntax node.
type NodeParser<'a> = Boxed<'a, 'a, TokenStream<'a>, TrackedNode, ParserError<'a>>;
/// Boxed parser producing a tracked argument slot.
type ArgumentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, TrackedArgumentSlot, ParserError<'a>>;
/// Consume insignificant whitespace tokens and produce no output.
fn insignificant_whitespace<'a>() -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! { Token::Whitespaces => () }.repeated().ignored()
}

/// Wrap a parser to accept either `{...}` or inline input.
fn maybe_braced<'a, T, P>(inner: P) -> impl Parser<'a, TokenStream<'a>, T, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, T, ParserError<'a>> + Clone + 'a,
    T: 'a,
{
    let ws = insignificant_whitespace();
    let braced = just(Token::LBrace)
        .ignore_then(ws.clone())
        .ignore_then(inner.clone())
        .then_ignore(ws)
        .then_ignore(just(Token::RBrace));

    choice((braced, inner))
}

/// Wrap a parser to accept `{...}`, allowing the braced form to be empty.
fn maybe_braced_or_empty<'a, T, P>(
    inner: P,
    empty_value: T,
) -> impl Parser<'a, TokenStream<'a>, T, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, T, ParserError<'a>> + Clone + 'a,
    T: Clone + 'a,
{
    let ws = insignificant_whitespace();
    let braced_empty_value = empty_value.clone();
    let braced = just(Token::LBrace)
        .ignore_then(ws.clone())
        .ignore_then(inner.clone().or_not())
        .then_ignore(ws)
        .then_ignore(just(Token::RBrace))
        .map(move |value| value.unwrap_or_else(|| braced_empty_value.clone()));

    choice((braced, inner))
}

/// Wrap a parser to accept an optional `[...]` argument.
fn optional_bracketed<'a, T, P>(
    inner: P,
) -> impl Parser<'a, TokenStream<'a>, Option<T>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, T, ParserError<'a>> + Clone + 'a,
    T: 'a,
{
    let ws = insignificant_whitespace();
    just(Token::LBracket)
        .ignore_then(ws.clone())
        .ignore_then(inner)
        .then_ignore(ws)
        .then_ignore(just(Token::RBracket))
        .or_not()
}

/// Wrap a parser to accept an optional `[...]` argument whose content may be empty.
fn optional_bracketed_or_empty<'a, T, P>(
    inner: P,
    empty_value: T,
) -> impl Parser<'a, TokenStream<'a>, Option<T>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, T, ParserError<'a>> + Clone + 'a,
    T: Clone + 'a,
{
    let ws = insignificant_whitespace();
    just(Token::LBracket)
        .ignore_then(ws.clone())
        .ignore_then(inner.or_not())
        .then_ignore(ws)
        .then_ignore(just(Token::RBracket))
        .map(move |value| value.unwrap_or_else(|| empty_value.clone()))
        .or_not()
}

/// Parse a math delimiter token into a typed `Delimiter`.
fn delimiter<'a>(
    ctx: &'a ParseContext,
) -> impl Parser<'a, TokenStream<'a>, Delimiter, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) if ctx.lookup_delimiter(c.to_string().as_str(), false, ContentMode::Math).is_some() => {
            let delimiter = ctx.lookup_delimiter(c.to_string().as_str(), false, ContentMode::Math).unwrap();
            if delimiter.name == "." && delimiter.unicode_value.is_empty() {
                Delimiter::None
            } else {
                Delimiter::Char(c)
            }
        },
        // Raw square brackets are tokenized separately so optional arguments
        // can be recognized without backtracking, but they still need to work
        // as plain delimiters after \left / \right.
        Token::LBracket if ctx.lookup_delimiter("[", false, ContentMode::Math).is_some() => Delimiter::Char('['),
        Token::RBracket if ctx.lookup_delimiter("]", false, ContentMode::Math).is_some() => Delimiter::Char(']'),
        Token::ControlSeq(name) if ctx.lookup_delimiter(name.as_str(), true, ContentMode::Math).is_some() => {
            Delimiter::Control(ctx.lookup_delimiter_control(name.as_str()).unwrap())
        }
    }
    .labelled("delimiter")
}

/// Parse escaped symbol control sequences into raw `Char` nodes.
fn escaped_symbol<'a>() -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if matches!(name.as_str(), "%" | "$" | "&" | "#" | "_" | "{" | "}") => {
            SyntaxNode::Char(name.chars().next().unwrap())
        }
    }
    .labelled("escaped symbol")
    .tracked()
}

/// Parse delimiter controls as visible math commands outside delimiter positions.
fn delimiter_control_command_parser<'a>(
    ctx: &'a ParseContext,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if ctx.lookup_delimiter(name.as_str(), true, ContentMode::Math).is_some() => {
            SyntaxNode::Command {
                name,
                args: vec![],
                known: true,
            }
        }
    }
    .labelled("bare delimiter control")
    .tracked()
}

/// Parse the active character `~` into `ActiveSpace`.
fn active_char<'a>() -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    just(Token::ActiveChar)
        .to(SyntaxNode::ActiveSpace)
        .tracked()
}

/// Parse plain math characters.
fn math_char<'a>() -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => SyntaxNode::Char(c),
        Token::Star => SyntaxNode::Char('*'),
        Token::Alignment => SyntaxNode::Char('&'),
        Token::LBracket => SyntaxNode::Char('['),
        Token::RBracket => SyntaxNode::Char(']'),
    }
    .labelled("math character")
    .tracked()
}

/// Parse bare math prime shorthand as a standalone atom.
fn math_prime<'a>() -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    select! {
        Token::Prime(count) => SyntaxNode::Prime { count },
    }
    .labelled("math prime")
    .tracked()
}

/// Parse and coalesce consecutive text characters/whitespace into a single `Text` node.
fn text_chunk<'a>() -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => c.to_string(),
        Token::Whitespaces => " ".to_string(),
        Token::Star => "*".to_string(),
        Token::LBracket => "[".to_string(),
        Token::RBracket => "]".to_string(),
        Token::Prime(n) => "'".repeat(n),
    }
    .repeated()
    .at_least(1)
    .collect::<Vec<_>>()
    .labelled("text")
    .map(|parts| {
        let mut buf = String::new();
        let mut last_was_space = false;
        for part in parts {
            if part == " " {
                if !last_was_space {
                    buf.push(' ');
                    last_was_space = true;
                }
            } else {
                buf.push_str(&part);
                last_was_space = false;
            }
        }
        SyntaxNode::Text(buf)
    })
    .tracked()
}

/// Match an exact control sequence.
fn control_seq<'a>(
    target: &'static str,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if name == target => (),
    }
    .labelled(target)
}

fn optional_control_seq<'a>(
    target: Option<&'static str>,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if target.is_some_and(|target| name == target) => (),
    }
    .labelled(target.unwrap_or("control stop"))
}

/// Build an implicit group from a content parser.
fn implicit_group_parser<'a, P>(
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    content.map_with(move |children, e| {
        let (nodes, records, diagnostics) = TrackedNode::decompose_children(children);
        TrackedNode {
            node: SyntaxNode::Group {
                mode,
                kind: GroupKind::Implicit,
                children: nodes,
            },
            span: e.span(),
            records,
            diagnostics,
        }
    })
}

/// Parse an explicit `{...}` group with the given content parser.
fn braced_group_parser<'a, P>(
    state: &'a ParserState<'a>,
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    custom(move |input| {
        let group_start = input.cursor();
        input.parse(just(Token::LBrace))?;

        let Some(_guard) = state.enter_group() else {
            skip_to_matching_rbrace(input);
            let span = input.span_from_cursor(&group_start);
            let message = format!(
                "max group depth ({}) exceeded",
                state.config.max_group_depth
            );
            let diagnostic = custom_error(
                span,
                message.clone(),
                ParseDiagnosticKind::MaxGroupDepthExceeded,
            )
            .into_owned();
            return Ok(TrackedNode::leaf(
                SyntaxNode::Error {
                    message,
                    snippet: slice_snippet(state.src, span),
                },
                span,
            )
            .with_diagnostics(vec![diagnostic]));
        };

        let result = input.parse(content.clone().then_ignore(just(Token::RBrace)));
        // _guard drops here: depth is restored on both Ok and Err paths.

        result.map(move |children| {
            let span = input.span_from_cursor(&group_start);
            let (nodes, records, diagnostics) = TrackedNode::decompose_children(children);
            TrackedNode {
                node: SyntaxNode::Group {
                    mode,
                    kind: GroupKind::Explicit,
                    children: nodes,
                },
                span,
                records,
                diagnostics,
            }
        })
    })
}

fn skip_to_matching_rbrace<'src, 'parse>(input: &mut ParserInput<'src, 'parse>) {
    let mut depth = 1usize;
    while depth > 0 {
        match input.next() {
            Some(Token::LBrace) => depth = depth.saturating_add(1),
            Some(Token::RBrace) => depth -= 1,
            Some(_) => {}
            None => return,
        }
    }
}

/// Parse `\left ... \right` delimited math group.
///
/// Whitespace between `\left`/`\right` and the delimiter is accepted so that
/// `\left ( ... \right )` round-trips correctly through the canonical
/// serializer's `Spaced` command spacing.
fn delimited_group_parser<'a, P>(
    ctx: &'a ParseContext,
    math_content: P,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();

    custom(move |input| {
        let group_start = input.cursor();
        input.parse(control_seq("left"))?;
        let _ = input.parse(ws.clone());

        let left_start = input.cursor();
        let left = match input.parse(delimiter(ctx)) {
            Ok(left) => left,
            Err(_) => {
                let mut err = Rich::custom(
                    input.span_from_cursor(&left_start),
                    "invalid \\left delimiter",
                );
                <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                    &mut err,
                    "left-delimited group",
                    input.span_from_cursor(&group_start),
                );
                return Err(with_diagnostic_kind(
                    err,
                    ParseDiagnosticKind::LeftRightDelimiter,
                ));
            }
        };

        let children = input.parse(math_content.clone())?;

        if input.parse(control_seq("right")).is_err() {
            let mut err = Rich::custom(
                input.span_from_cursor(&group_start),
                "missing \\right for \\left-delimited group",
            );
            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                &mut err,
                "left-delimited group",
                input.span_from_cursor(&group_start),
            );
            return Err(with_diagnostic_kind(
                err,
                ParseDiagnosticKind::LeftRightDelimiter,
            ));
        }
        let _ = input.parse(ws.clone());

        let delimiter_start = input.cursor();
        let right = match input.parse(delimiter(ctx)) {
            Ok(right) => right,
            Err(_) => {
                let mut err = Rich::custom(
                    input.span_from_cursor(&delimiter_start),
                    "invalid \\right delimiter",
                );
                <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                    &mut err,
                    "left-delimited group",
                    input.span_from_cursor(&group_start),
                );
                return Err(with_diagnostic_kind(
                    err,
                    ParseDiagnosticKind::LeftRightDelimiter,
                ));
            }
        };

        let span = input.span_from_cursor(&group_start);
        let (nodes, records, diagnostics) = TrackedNode::decompose_children(children);
        Ok(TrackedNode {
            node: SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Delimited { left, right },
                children: nodes,
            },
            span,
            records,
            diagnostics,
        })
    })
}

/// Tracks how the superscript was built so we can detect double-exponent errors.
///
/// Primes (e.g. `f'`) and explicit carets (`^`) can be mixed once
/// (`f'^2` → implicit group `[', 2]`), but a second explicit caret or
/// a second prime run after an explicit caret is an error.
#[derive(Clone, Debug)]
enum SupState {
    /// Superscript is composed entirely of prime marks
    Prime(TrackedNode),
    /// Superscript was set by an explicit `^` token
    Explicit(TrackedNode),
    /// Prime(s) merged with an explicit superscript
    Mixed(TrackedNode),
}

/// Intermediate result of script parsing before folding into `SyntaxNode::Scripted`.
#[derive(Debug)]
struct ScriptComponents {
    base: TrackedNode,
    subscript: Option<TrackedNode>,
    superscript: Option<TrackedNode>,
}

#[derive(Clone, Copy)]
enum ScriptMarker {
    Sub,
    Sup,
    Prime,
}

fn braced_prime_group<'src, 'parse>(input: &mut ParserInput<'src, 'parse>) -> Option<TrackedNode> {
    let checkpoint = input.save();
    let ws = insignificant_whitespace();
    let start = input.cursor();

    let _ = input.parse(ws.clone());
    if !matches!(input.peek(), Some(Token::LBrace)) {
        input.rewind(checkpoint);
        return None;
    }
    input.next();

    let _ = input.parse(ws.clone());
    let count = match input.peek() {
        Some(Token::Prime(n)) => {
            input.next();
            n
        }
        _ => {
            input.rewind(checkpoint);
            return None;
        }
    };

    let _ = input.parse(ws.clone());
    if !matches!(input.peek(), Some(Token::RBrace)) {
        input.rewind(checkpoint);
        return None;
    }
    input.next();

    let span = input.span_from_cursor(&start);
    Some(TrackedNode::leaf(
        SyntaxNode::Group {
            mode: ContentMode::Math,
            kind: GroupKind::Explicit,
            children: vec![SyntaxNode::Prime { count }],
        },
        span,
    ))
}

/// Imperative parser that greedily collects `^`, `_`, and prime tokens after
/// an atom, producing [`ScriptComponents`].
///
/// Handles the TeX rules for combining primes with explicit superscripts:
/// `f'^2` is valid (merged), but `f^a^b` is a "double exponent" error.
/// An empty base is allowed when the first token is already a script marker,
/// producing an empty implicit group as the base.
fn parse_scripted_components<'src, 'parse, P>(
    input: &mut ParserInput<'src, 'parse>,
    atom_for_scripts: P,
) -> Result<ScriptComponents, Rich<'src, Token>>
where
    P: Parser<'src, TokenStream<'src>, TrackedNode, ParserError<'src>> + Clone + 'src,
{
    let ws = insignificant_whitespace();

    let preserve_atom_error = matches!(input.peek(), Some(Token::ControlSeq(_)));
    let base_opt = if preserve_atom_error {
        Some(input.parse(atom_for_scripts.clone())?)
    } else {
        input.parse(atom_for_scripts.clone().or_not())?
    };
    let base = match base_opt {
        Some(base) => base,
        None => match input.peek() {
            Some(Token::Superscript) | Some(Token::Subscript) => {
                // Empty implicit group as base; use zero-width span at current byte position.
                // Note: span_from_cursor with the same cursor is unreliable for zero-width spans
                // in chumsky's MappedInput — the cursor's end field defaults to eoi.
                let pos_cursor = input.cursor();
                let byte_pos = input.span_from_cursor(&pos_cursor).start;
                let span = SimpleSpan::new((), byte_pos..byte_pos);
                TrackedNode::leaf(
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![],
                    },
                    span,
                )
            }
            _ => {
                let cursor = input.cursor();
                return Err(input.err_peek_or_point(&cursor, "expected atom or script marker"));
            }
        },
    };

    let mut sup_state: Option<SupState> = None;
    let mut subscript: Option<TrackedNode> = None;

    loop {
        let checkpoint = input.save();
        let _ = input.parse(ws.clone());

        let marker_start = input.cursor();

        let marker = match input.peek() {
            Some(Token::Superscript) => {
                input.next();
                let tracked = match braced_prime_group(input) {
                    Some(group) => group,
                    None => input.parse(atom_for_scripts.clone())?,
                };
                let span = input.span_from_cursor(&marker_start);
                Some((ScriptMarker::Sup, TrackedNode { span, ..tracked }))
            }
            Some(Token::Subscript) => {
                input.next();
                let tracked = input.parse(atom_for_scripts.clone())?;
                let span = input.span_from_cursor(&marker_start);
                Some((ScriptMarker::Sub, TrackedNode { span, ..tracked }))
            }
            Some(Token::Prime(_)) => {
                let count = match input.next() {
                    Some(Token::Prime(n)) => n,
                    _ => unreachable!("peek ensured prime token"),
                };
                let prime_span = input.span_from_cursor(&marker_start);
                Some((
                    ScriptMarker::Prime,
                    TrackedNode::leaf(SyntaxNode::Prime { count }, prime_span),
                ))
            }
            _ => None,
        };

        let Some((kind, tracked)) = marker else {
            input.rewind(checkpoint);
            break;
        };

        match kind {
            ScriptMarker::Sub => {
                if subscript.is_some() {
                    return Err(
                        input.err_since(&marker_start, "Double subscripts: use braces to clarify")
                    );
                }
                subscript = Some(tracked);
            }
            ScriptMarker::Sup => {
                let current = sup_state.take();
                sup_state = match current {
                    None => Some(SupState::Explicit(tracked)),
                    Some(SupState::Prime(existing)) => {
                        // Merge prime and explicit superscript into an implicit group.
                        let merged_span =
                            SimpleSpan::new((), existing.span.start..tracked.span.end);
                        let merged = TrackedNode::leaf(
                            SyntaxNode::Group {
                                mode: ContentMode::Math,
                                kind: GroupKind::Implicit,
                                children: vec![existing.node, tracked.node],
                            },
                            merged_span,
                        );
                        Some(SupState::Mixed(merged))
                    }
                    Some(SupState::Explicit(_)) | Some(SupState::Mixed(_)) => {
                        return Err(input
                            .err_since(&marker_start, "Double exponent: use braces to clarify"));
                    }
                };
            }
            ScriptMarker::Prime => {
                let current = sup_state.take();
                sup_state = match current {
                    None => Some(SupState::Prime(tracked)),
                    Some(SupState::Prime(existing)) => {
                        // Merge consecutive prime runs into an implicit group.
                        let merged_span =
                            SimpleSpan::new((), existing.span.start..tracked.span.end);
                        let merged = TrackedNode::leaf(
                            SyntaxNode::Group {
                                mode: ContentMode::Math,
                                kind: GroupKind::Implicit,
                                children: vec![existing.node, tracked.node],
                            },
                            merged_span,
                        );
                        Some(SupState::Mixed(merged))
                    }
                    Some(SupState::Explicit(_)) | Some(SupState::Mixed(_)) => {
                        return Err(input.err_since(
                            &marker_start,
                            "Prime causes double exponent: use braces to clarify",
                        ));
                    }
                };
            }
        }
    }

    let superscript = sup_state.map(|state| match state {
        SupState::Prime(t) | SupState::Explicit(t) | SupState::Mixed(t) => t,
    });

    Ok(ScriptComponents {
        base,
        subscript,
        superscript,
    })
}

// ============================================================================
// Public Interface
// ============================================================================

/// Parse a LaTeX math formula using the default package knowledge base.
///
/// Returns the root [`SyntaxNode`] together with its byte span on success,
/// or a list of rich diagnostics on failure. For partial-parse semantics
/// (result + diagnostics), use [`ParseContext::parse`](crate::parse::ParseContext::parse)
/// instead.
pub fn parse(
    src: &str,
    reject_unknown: bool,
) -> Result<Spanned<SyntaxNode>, Vec<Rich<'static, Token>>> {
    let token_stream = build_token_stream(src);
    let config = if reject_unknown {
        ParseConfig {
            reject_unknown: true,
            ..Default::default()
        }
    } else {
        ParseConfig::LENIENT
    };
    let state = ParserState::new(ParseContext::shared(), &config, src);
    let parser = math_block_parser(&state)
        .map_with(|tracked, e| (promote_to_root(tracked.node), e.span()))
        .then_ignore(end());
    parser
        .parse(token_stream)
        .into_result()
        .map_err(|errors| errors.into_iter().map(Rich::into_owned).collect())
}

/// Promote the top-level implicit group produced by `math_block_parser` /
/// `text_block_parser` into a proper `SyntaxNode::Root`.
fn promote_to_root(node: SyntaxNode) -> SyntaxNode {
    match node {
        node @ SyntaxNode::Root { .. } => node,
        SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children,
        } => SyntaxNode::Root { mode, children },
        other => panic!(
            "top-level parser must finish as implicit group or root, got {:?}",
            other
        ),
    }
}

// ============================================================================
// Environment Body Parser
// ============================================================================

/// Pick the correct implicit group parser for an environment body.
fn env_body_parser<'a>(
    mode: ContentMode,
    content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    let body = implicit_group_parser(mode, content);
    custom(move |input| {
        let body_start = input.cursor();
        match input.parse(body.clone()) {
            Ok(tracked) => Ok(tracked),
            Err(mut err) => {
                <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                    &mut err,
                    "environment body",
                    input.span_from_cursor(&body_start),
                );
                Err(err)
            }
        }
    })
}

fn mode_item_parser<'a>(
    mode: ContentMode,
    state: &'a ParserState<'a>,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> NodeParser<'a> {
    match mode {
        ContentMode::Math => math_item_parser(state, math_content, text_content).boxed(),
        ContentMode::Text => text_item_parser(state, math_content, text_content).boxed(),
    }
}

enum ModeLookup<'a, T> {
    Found(&'a T),
    WrongMode,
    NotFound,
}

fn other_mode(mode: ContentMode) -> ContentMode {
    match mode {
        ContentMode::Math => ContentMode::Text,
        ContentMode::Text => ContentMode::Math,
    }
}

fn lookup_command_for_parse<'a>(
    ctx: &'a ParseContext,
    name: &str,
    mode: ContentMode,
) -> ModeLookup<'a, ActiveCommandRecord> {
    if let Some(meta) = ctx.lookup_command(name, mode) {
        ModeLookup::Found(meta)
    } else if ctx.lookup_command(name, other_mode(mode)).is_some() {
        ModeLookup::WrongMode
    } else {
        ModeLookup::NotFound
    }
}

fn lookup_env_for_parse<'a>(
    ctx: &'a ParseContext,
    name: &str,
    mode: ContentMode,
) -> ModeLookup<'a, ActiveEnvironmentRecord> {
    if let Some(meta) = ctx.lookup_env(name, mode) {
        ModeLookup::Found(meta)
    } else if ctx.lookup_env(name, other_mode(mode)).is_some() {
        ModeLookup::WrongMode
    } else {
        ModeLookup::NotFound
    }
}

fn is_outer_closing_boundary(token: Option<&Token>) -> bool {
    match token {
        None | Some(Token::RBrace) | Some(Token::RBracket) | Some(Token::MathShift) => true,
        Some(Token::ControlSeq(name)) => matches!(name.as_str(), "right" | "end"),
        _ => false,
    }
}

fn slice_snippet(src: &str, span: SimpleSpan) -> String {
    src.get(span.start..span.end).unwrap_or("").to_string()
}

fn is_math_hard_stop(token: &Token) -> bool {
    matches!(token, Token::RBrace | Token::MathShift)
        || matches!(token, Token::ControlSeq(name) if matches!(name.as_str(), "right" | "end"))
}

fn is_math_control_paren_hard_stop(token: &Token) -> bool {
    is_math_hard_stop(token) || matches!(token, Token::ControlSeq(name) if name == ")")
}

fn is_text_hard_stop(token: &Token) -> bool {
    matches!(token, Token::RBrace | Token::MathShift)
        || matches!(token, Token::ControlSeq(name) if name == "end")
}

fn is_direct_left_group_error(err: &Rich<'_, Token>) -> bool {
    has_diagnostic_kind(err, ParseDiagnosticKind::LeftRightDelimiter)
}

fn is_direct_environment_header_error(err: &Rich<'_, Token>) -> bool {
    matches!(
        diagnostic_kind(err),
        Some(ParseDiagnosticKind::UnknownEnvironment | ParseDiagnosticKind::EnvironmentModeError)
    )
}

fn scan_environment_stack_before(src: &str, limit: usize) -> Vec<String> {
    let mut stack = Vec::new();
    let tokens: Vec<(Token, std::ops::Range<usize>)> = Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (token, span)
        })
        .take_while(|(_, span)| span.start < limit)
        .collect();

    let mut index = 0;
    while index < tokens.len() {
        let Some((Token::ControlSeq(name), _)) = tokens.get(index) else {
            index += 1;
            continue;
        };

        if !matches!(name.as_str(), "begin" | "end") {
            index += 1;
            continue;
        }

        let mut next = index + 1;
        while matches!(tokens.get(next), Some((Token::Whitespaces, _))) {
            next += 1;
        }
        if !matches!(tokens.get(next), Some((Token::LBrace, _))) {
            index += 1;
            continue;
        }
        next += 1;

        let mut env_name = String::new();
        while let Some((token, _)) = tokens.get(next) {
            match token {
                Token::Char(c) => env_name.push(*c),
                Token::Star => env_name.push('*'),
                Token::RBrace => break,
                _ => {
                    env_name.clear();
                    break;
                }
            }
            next += 1;
        }

        if env_name.is_empty() {
            index += 1;
            continue;
        }

        if name == "begin" {
            stack.push(env_name);
        } else if let Some(pos) = stack.iter().rposition(|open| open == &env_name) {
            stack.truncate(pos);
        }

        index += 1;
    }

    stack
}

fn peek_environment_name_at_cursor<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    head: &'static str,
) -> Option<String> {
    let checkpoint = input.save();
    let ws = insignificant_whitespace();
    let result = (|| {
        input.parse(control_seq(head)).ok()?;
        let _ = input.parse(ws.clone());
        input.parse(env_name_parser()).ok()
    })();
    input.rewind(checkpoint);
    result
}

fn consume_environment_end<'src, 'parse>(input: &mut ParserInput<'src, 'parse>) -> bool {
    consume_environment_marker(input, "end")
}

fn consume_environment_marker<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    head: &'static str,
) -> bool {
    let checkpoint = input.save();
    let ws = insignificant_whitespace();
    if input.parse(control_seq(head)).is_err() {
        input.rewind(checkpoint);
        return false;
    }
    let _ = input.parse(ws.clone());
    if input.parse(env_name_parser()).is_err() {
        input.rewind(checkpoint);
        return false;
    }
    true
}

fn leading_begin_environment_name(src: &str) -> Option<String> {
    let rest = src.trim_start();
    let rest = rest.strip_prefix("\\begin")?.trim_start();
    let rest = rest.strip_prefix('{')?;
    let end = rest.find('}')?;
    let env_name = &rest[..end];
    if env_name.is_empty()
        || !env_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '*')
    {
        None
    } else {
        Some(env_name.to_string())
    }
}

fn normalize_recovery_message(
    ctx: &ParseContext,
    current_mode: ContentMode,
    src: &str,
    message: String,
    kind: Option<ParseDiagnosticKind>,
) -> (String, Option<ParseDiagnosticKind>) {
    if kind != Some(ParseDiagnosticKind::RawExpectedFound) {
        return (message, kind);
    }

    if let Some(env_name) = leading_begin_environment_name(src)
        && ctx.lookup_env(env_name.as_str(), current_mode).is_none()
        && ctx
            .lookup_env(env_name.as_str(), other_mode(current_mode))
            .is_some()
    {
        return (
            format!(
                "Environment {} is not allowed in {} mode",
                env_name, current_mode
            ),
            Some(ParseDiagnosticKind::EnvironmentModeError),
        );
    }

    let tokens: Vec<Token> = Token::lexer(src)
        .map(|token| {
            token.unwrap_or_else(|()| panic!("Lexer error while normalizing recovery message"))
        })
        .collect();
    let mut stack = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        let Token::ControlSeq(head) = &tokens[index] else {
            index += 1;
            continue;
        };

        if !matches!(head.as_str(), "begin" | "end") {
            index += 1;
            continue;
        }

        let mut next = index + 1;
        while matches!(tokens.get(next), Some(Token::Whitespaces)) {
            next += 1;
        }
        if !matches!(tokens.get(next), Some(Token::LBrace)) {
            index += 1;
            continue;
        }
        next += 1;

        let mut env_name = String::new();
        while let Some(token) = tokens.get(next) {
            match token {
                Token::Char(c) => env_name.push(*c),
                Token::Star => env_name.push('*'),
                Token::RBrace => break,
                _ => {
                    env_name.clear();
                    break;
                }
            }
            next += 1;
        }

        if env_name.is_empty() {
            index += 1;
            continue;
        }

        if head == "begin" {
            stack.push(env_name);
        } else if let Some(expected) = stack.last() {
            if expected == &env_name {
                stack.pop();
            } else {
                return (
                    format!(
                        "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
                        expected, env_name
                    ),
                    Some(ParseDiagnosticKind::EnvironmentNameMismatch),
                );
            }
        }

        index += 1;
    }

    (message, kind)
}

fn is_hard_stop_after_whitespace<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    is_hard_stop: fn(&Token) -> bool,
) -> bool {
    let checkpoint = input.save();
    let _ = input.parse(insignificant_whitespace());
    let result = matches!(input.peek().as_ref(), Some(token) if is_hard_stop(token));
    input.rewind(checkpoint);
    result
}

fn recovery_diagnostic_from_error(
    err: &Rich<'_, Token>,
    message: &str,
    kind: Option<ParseDiagnosticKind>,
) -> Rich<'static, Token> {
    let Some(kind) = kind else {
        return clone_rich_error(err).into_owned();
    };

    match err.reason() {
        chumsky::error::RichReason::ExpectedFound { .. }
            if matches!(
                kind,
                ParseDiagnosticKind::RawExpectedFound
                    | ParseDiagnosticKind::EnvironmentNameMismatch
                    | ParseDiagnosticKind::UnclosedInlineMath
            ) =>
        {
            with_diagnostic_kind(clone_rich_error(err), kind).into_owned()
        }
        chumsky::error::RichReason::ExpectedFound { .. } => {
            custom_error(*err.span(), message, kind).into_owned()
        }
        chumsky::error::RichReason::Custom(original_message) => {
            let (_, public_message) = ParseDiagnosticKind::split_message(original_message.as_str());
            if public_message == message {
                with_diagnostic_kind(clone_rich_error(err), kind).into_owned()
            } else {
                custom_error(*err.span(), message, kind).into_owned()
            }
        }
    }
}

fn invalid_left_recovery_diagnostic(
    ctx: &ParseContext,
    src: &str,
    search_start: usize,
    search_end: usize,
) -> Option<Rich<'static, Token>> {
    let tokens: Vec<(Token, SimpleSpan)> = Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error while scanning recoverable \\left diagnostic")
            });
            (token, SimpleSpan::from(span))
        })
        .collect();

    let mut index = 0;
    while index < tokens.len() {
        let (token, span) = &tokens[index];
        if span.start > search_end {
            break;
        }
        if span.start < search_start {
            index += 1;
            continue;
        }

        if !matches!(token, Token::ControlSeq(name) if name == "left") {
            index += 1;
            continue;
        }

        let mut next = index + 1;
        while matches!(tokens.get(next), Some((Token::Whitespaces, _))) {
            next += 1;
        }

        let Some((delimiter_token, delimiter_span)) = tokens.get(next) else {
            return Some(
                custom_error(
                    *span,
                    "invalid \\left delimiter",
                    ParseDiagnosticKind::LeftRightDelimiter,
                )
                .into_owned(),
            );
        };

        if delimiter_span.start > search_end {
            return None;
        }

        let valid = match delimiter_token {
            Token::Char(c) => ctx
                .lookup_delimiter(c.to_string().as_str(), false, ContentMode::Math)
                .is_some(),
            Token::LBracket => ctx
                .lookup_delimiter("[", false, ContentMode::Math)
                .is_some(),
            Token::RBracket => ctx
                .lookup_delimiter("]", false, ContentMode::Math)
                .is_some(),
            Token::ControlSeq(name) => ctx
                .lookup_delimiter(name.as_str(), true, ContentMode::Math)
                .is_some(),
            _ => false,
        };

        if !valid {
            return Some(
                custom_error(
                    *delimiter_span,
                    "invalid \\left delimiter",
                    ParseDiagnosticKind::LeftRightDelimiter,
                )
                .into_owned(),
            );
        }

        index += 1;
    }

    None
}

fn expected_found_control_sequence(err: &Rich<'_, Token>, expected_name: &str) -> bool {
    match err.reason() {
        chumsky::error::RichReason::ExpectedFound {
            found: Some(found), ..
        } => matches!(&**found, Token::ControlSeq(name) if name == expected_name),
        chumsky::error::RichReason::ExpectedFound { found: None, .. }
        | chumsky::error::RichReason::Custom(_) => false,
    }
}

fn recoverable_content_item_parser<'a, P>(
    state: &'a ParserState<'a>,
    current_mode: ContentMode,
    src: &'a str,
    item: P,
    is_hard_stop: fn(&Token) -> bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    custom(move |input| {
        let ctx = state.ctx;
        let ws = insignificant_whitespace();
        let metadata_checkpoint = input.save();
        let _ = input.parse(ws.clone());
        let item_start = input.cursor();
        let item_starts_with_left =
            matches!(input.peek().as_ref(), Some(Token::ControlSeq(name)) if name == "left");
        let item_starts_with_begin =
            matches!(input.peek().as_ref(), Some(Token::ControlSeq(name)) if name == "begin");
        let item_start_index = input.span_from_cursor(&item_start).start;
        let opening_environment = peek_environment_name_at_cursor(input, "begin");
        let outer_environment_stack = opening_environment
            .as_ref()
            .map(|_| scan_environment_stack_before(src, item_start_index))
            .unwrap_or_default();
        input.rewind(metadata_checkpoint);

        let checkpoint = input.save();
        let err = match input.parse(item.clone()) {
            Ok(tracked) => return Ok(tracked),
            Err(err) => err,
        };
        let direct_left_group_error = item_starts_with_left && is_direct_left_group_error(&err);
        let direct_environment_header_error =
            item_starts_with_begin && is_direct_environment_header_error(&err);
        if direct_left_group_error || direct_environment_header_error {
            return Err(err);
        }
        let failure_environment_stack = scan_environment_stack_before(src, err.span().end);
        input.rewind(checkpoint);

        if is_hard_stop_after_whitespace(input, is_hard_stop) {
            return Err(err);
        }

        let err_kind = diagnostic_kind(&err).or_else(|| {
            matches!(
                err.reason(),
                chumsky::error::RichReason::ExpectedFound { .. }
            )
            .then_some(ParseDiagnosticKind::RawExpectedFound)
        });
        let (message, kind) = opening_environment
            .as_ref()
            .and_then(|env_name| {
                if ctx.lookup_env(env_name.as_str(), current_mode).is_none()
                    && ctx
                        .lookup_env(env_name.as_str(), other_mode(current_mode))
                        .is_some()
                {
                    Some((
                        format!(
                            "Environment {} is not allowed in {} mode",
                            env_name, current_mode
                        ),
                        Some(ParseDiagnosticKind::EnvironmentModeError),
                    ))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| match err.reason() {
                chumsky::error::RichReason::ExpectedFound { .. } => (format!("{err}"), err_kind),
                chumsky::error::RichReason::Custom(message) => (
                    ParseDiagnosticKind::split_message(message.as_str())
                        .1
                        .to_string(),
                    err_kind,
                ),
            });
        let recovery_src = src.get(item_start_index..).unwrap_or(src);
        let (message, kind) =
            normalize_recovery_message(ctx, current_mode, recovery_src, message, kind);

        let recovery_parser = custom({
            let message = message.clone();
            let should_consume_command_args = kind == Some(ParseDiagnosticKind::CommandModeError);
            let opening_environment = opening_environment.clone();
            let outer_environment_stack = outer_environment_stack.clone();
            move |input| {
                let start = input.cursor();
                let ws = insignificant_whitespace();
                let failure_environment_stack = failure_environment_stack.clone();
                let mut consumed = false;

                loop {
                    let boundary_checkpoint = input.save();
                    let _ = input.parse(ws.clone());

                    if should_consume_command_args {
                        if matches!(input.peek(), Some(Token::LBracket)) {
                            let _ = collect_optional_bracketed_tokens(input, true);
                            consumed = true;
                            continue;
                        }
                        if matches!(input.peek(), Some(Token::LBrace)) {
                            let _ = collect_braced_tokens(input, true);
                            consumed = true;
                            continue;
                        }
                    }

                    if peek_environment_name_at_cursor(input, "begin").is_some() {
                        if !consume_environment_marker(input, "begin") {
                            break;
                        }
                        consumed = true;
                        continue;
                    }

                    if let Some(open_name) = opening_environment.as_ref()
                        && let Some(end_name) = peek_environment_name_at_cursor(input, "end")
                    {
                        if end_name == *open_name {
                            if failure_environment_stack.last() != Some(open_name) {
                                break;
                            }
                            if !consume_environment_end(input) {
                                break;
                            }
                            consumed = true;
                            break;
                        }

                        if outer_environment_stack.iter().any(|name| name == &end_name) {
                            break;
                        }

                        if !consume_environment_end(input) {
                            break;
                        }
                        consumed = true;
                        break;
                    }

                    match input.peek().as_ref() {
                        Some(Token::ControlSeq(name)) if name == "right" => {
                            break;
                        }
                        Some(token) if consumed && is_hard_stop(token) => {
                            break;
                        }
                        _ => input.rewind(boundary_checkpoint),
                    }

                    match input.peek().as_ref() {
                        Some(_) => {
                            let _ = input.next();
                            consumed = true;
                        }
                        None => break,
                    }
                }

                if !consumed {
                    return Err(
                        input.err_since(&start, "content recovery must consume at least one token")
                    );
                }

                let span = input.span_from_cursor(&start);
                Ok(TrackedNode::leaf(
                    SyntaxNode::Error {
                        message: message.clone(),
                        snippet: slice_snippet(src, span),
                    },
                    span,
                ))
            }
        });

        let invalid_left_diagnostic = (expected_found_control_sequence(&err, "right")
            || expected_found_control_sequence(&err, "end"))
        .then(|| invalid_left_recovery_diagnostic(ctx, src, item_start_index, err.span().end))
        .flatten();
        let diagnostic = invalid_left_diagnostic
            .unwrap_or_else(|| recovery_diagnostic_from_error(&err, message.as_str(), kind));
        state.push_recovery_diagnostic(diagnostic);
        input.parse(recovery_parser)
    })
}

/// Consume a control-sequence token, look it up in the KB, and validate that
/// it matches the `expected_kind` and is allowed in `current_mode`.
fn command_head_parser<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    ctx: &'parse ParseContext,
    expected_kind: CommandKind,
    current_mode: ContentMode,
    reject_unknown: bool,
) -> Result<(String, &'parse ActiveCommandRecord), Rich<'src, Token>> {
    let cmd_start = input.cursor();
    let token = input.next();
    let name = match token {
        Some(Token::ControlSeq(name)) => name,
        Some(_) => return Err(input.err_since(&cmd_start, "not a command")),
        None => return Err(input.err_since(&cmd_start, "not a command")),
    };

    let cmd_span = input.span_from_cursor(&cmd_start);

    let meta = match lookup_command_for_parse(ctx, &name, current_mode) {
        ModeLookup::Found(meta) if meta.kind == expected_kind => meta,
        ModeLookup::Found(_) => {
            return Err(Rich::custom(
                cmd_span,
                format!("not {}", expected_kind.label()),
            ));
        }
        ModeLookup::WrongMode => {
            return Err(custom_error(
                cmd_span,
                format!("Command \\{} is not allowed in {} mode", name, current_mode),
                ParseDiagnosticKind::CommandModeError,
            ));
        }
        ModeLookup::NotFound => {
            if reject_unknown {
                return Err(custom_error(
                    cmd_span,
                    format!("Unknown command: \\{}", name),
                    ParseDiagnosticKind::UnknownCommand,
                ));
            } else {
                return Err(Rich::custom(cmd_span, "unknown"));
            }
        }
    };

    Ok((name, meta))
}

// ============================================================================
// Content and Argument Parsers
// ============================================================================

fn infix_guard<'a>(
    ctx: &'a ParseContext,
    current_mode: ContentMode,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if matches!(lookup_command_for_parse(ctx, name.as_str(), current_mode),
                ModeLookup::Found(meta) if meta.kind == CommandKind::Infix)
                || ctx.lookup_command(name.as_str(), other_mode(current_mode))
                    .map(|meta| meta.kind == CommandKind::Infix)
                    .unwrap_or(false) => ()
    }
    .rewind()
}

fn source_has_buildrel_tail_before_over(src: &str, over_start: usize) -> bool {
    let Some(prefix) = src.get(..over_start) else {
        return false;
    };
    let Some(buildrel_index) = prefix.rfind(r"\buildrel") else {
        return false;
    };
    let previous_infix_index = ["\\over", "\\choose", "\\atop", "\\above"]
        .into_iter()
        .filter_map(|needle| prefix.rfind(needle))
        .max();

    previous_infix_index.is_none_or(|index| buildrel_index > index)
}

/// Parse one math item node (with script handling) without outer spacing policy.
///
/// Callers decide whether to wrap it with padding or stop-guards.
fn math_item_node_parser<'a, P>(
    state: &'a ParserState<'a>,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    let atom = math_atom_parser(state, group_content, math_content, text_content);
    scripted_atom_parser(atom)
}

/// Parse a single math item in argument contexts.
///
/// This parser does not consume trailing whitespace so the following argument
/// slot can still enforce `no_leading_space`.
fn math_item_parser<'a>(
    state: &'a ParserState<'a>,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    let item = math_item_node_parser(state, math_content.clone(), math_content, text_content);

    infix_guard(state.ctx, ContentMode::Math)
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .ignore_then(item)
}

/// Parse a single math atom in argument shorthand contexts.
///
/// This keeps following `_`, `^`, and prime tokens available to the outer
/// scripted parser instead of folding them into a mandatory `m` argument.
/// If the argument itself begins with `_` or `^`, parse it as an empty-base
/// scripted atom so legacy shorthands such as `\mod _{n}` remain valid.
fn math_atom_argument_parser<'a>(
    state: &'a ParserState<'a>,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    let atom = math_atom_parser(state, math_content.clone(), math_content, text_content);
    let scripted = scripted_atom_parser(atom.clone());
    let leading_script_marker = insignificant_whitespace()
        .ignore_then(select! {
            Token::Superscript => (),
            Token::Subscript => (),
        })
        .rewind();

    infix_guard(state.ctx, ContentMode::Math)
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .ignore_then(choice((leading_script_marker.ignore_then(scripted), atom)))
}

/// Parse a single text item (respecting stop guards).
fn text_item_parser<'a>(
    state: &'a ParserState<'a>,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    let control_inline_math_content =
        math_content_parser_with_extra_control_stop(state, text_content.clone(), None, ")");
    let normal_item = text_atom_parser(
        state,
        text_content.clone(),
        math_content,
        control_inline_math_content,
        text_content,
    );

    control_seq("end").not().ignore_then(normal_item)
}

// ============================================================================
// Command and Environment Parsers
// ============================================================================

fn prefix_command_parser<'a>(
    state: &'a ParserState<'a>,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let ctx = state.ctx;
        let reject_unknown = state.config.reject_unknown;
        let cmd_start = input.cursor();
        let (name, meta) = match command_head_parser(
            input,
            ctx,
            CommandKind::Prefix,
            current_mode,
            reject_unknown,
        ) {
            Ok(data) => data,
            Err(err) => return Err(err),
        };

        let cmd_args = parse_argument_slots(
            input,
            state,
            math_content.clone(),
            text_content.clone(),
            meta.argspec.args,
            "command argument",
        )?;

        let span = input.span_from_cursor(&cmd_start);
        let (args, records, diagnostics) = TrackedNode::decompose_args(cmd_args);
        Ok(TrackedNode {
            node: SyntaxNode::Command {
                name,
                args,
                known: true,
            },
            span,
            records,
            diagnostics,
        })
    })
}

fn declarative_command_parser<'a>(
    state: &'a ParserState<'a>,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let ctx = state.ctx;
        let reject_unknown = state.config.reject_unknown;
        let cmd_start = input.cursor();
        let (name, meta) = match command_head_parser(
            input,
            ctx,
            CommandKind::Declarative,
            current_mode,
            reject_unknown,
        ) {
            Ok(data) => data,
            Err(err) => return Err(err),
        };

        let cmd_args = parse_argument_slots(
            input,
            state,
            math_content.clone(),
            text_content.clone(),
            meta.argspec.args,
            "declarative command argument",
        )?;

        let span = input.span_from_cursor(&cmd_start);
        let (args, records, diagnostics) = TrackedNode::decompose_args(cmd_args);
        Ok(TrackedNode {
            node: SyntaxNode::Declarative { name, args },
            span,
            records,
            diagnostics,
        })
    })
}

fn unknown_command_parser<'a>(
    ctx: &'a ParseContext,
    reject_unknown: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if !ctx.knows_command_name(name.as_str()) => name
    }
    .try_map(move |name, span| {
        if matches!(name.as_str(), "begin" | "end") {
            return Err(Rich::custom(
                span,
                format!("Reserved environment delimiter: \\{}", name),
            ));
        }

        if reject_unknown {
            Err(custom_error(
                span,
                format!("Unknown command: \\{}", name),
                ParseDiagnosticKind::UnknownCommand,
            ))
        } else {
            Ok(TrackedNode::leaf(
                SyntaxNode::Command {
                    name,
                    args: vec![],
                    known: false,
                },
                span,
            ))
        }
    })
    .labelled("unknown command")
}

/// Parse `{name}` inside environment delimiters.
///
/// Environment names are parsed as a single token sequence and may include `*`
/// as part of the name (e.g. `align*`).
fn env_name_parser<'a>() -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => c,
        Token::Star => '*',
    }
    .repeated()
    .at_least(1)
    .collect::<String>()
    .delimited_by(just(Token::LBrace), just(Token::RBrace))
    .labelled("environment name")
}

/// Parse a full environment including body and closing tag.
fn environment_parser<'a>(
    state: &'a ParserState<'a>,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let ctx = state.ctx;
        let reject_unknown = state.config.reject_unknown;
        let env_start = input.cursor();
        let ws = insignificant_whitespace();

        input.parse(control_seq("begin"))?;
        let _ = input.parse(ws.clone());

        let name_start = input.cursor();
        let name = input.parse(env_name_parser())?;
        let name_span = input.span_from_cursor(&name_start);

        let (cmd_args, known, body_mode) =
            match lookup_env_for_parse(ctx, name.as_str(), current_mode) {
                ModeLookup::Found(meta) => {
                    let cmd_args = parse_argument_slots(
                        input,
                        state,
                        math_content.clone(),
                        text_content.clone(),
                        meta.argspec.args,
                        "environment argument",
                    )?;

                    (cmd_args, true, meta.body_mode)
                }
                ModeLookup::WrongMode => {
                    return Err(custom_error(
                        name_span,
                        format!(
                            "Environment {} is not allowed in {} mode",
                            name, current_mode
                        ),
                        ParseDiagnosticKind::EnvironmentModeError,
                    ));
                }
                ModeLookup::NotFound => {
                    if reject_unknown {
                        return Err(custom_error(
                            name_span,
                            format!("Unknown environment: {}", name),
                            ParseDiagnosticKind::UnknownEnvironment,
                        ));
                    }

                    (vec![], false, current_mode)
                }
            };

        let body_content = match body_mode {
            ContentMode::Math => math_content.clone(),
            ContentMode::Text => text_content.clone(),
        };
        let body_recovery_start = input.save();
        let body = input.parse(env_body_parser(body_mode, body_content))?;

        let expected_end = name.clone();
        let missing_end_message = format!(
            "Environment {} missing closing \\end{{{}}}",
            expected_end, expected_end
        );

        let end_start = input.cursor();

        if input.parse(control_seq("end")).is_err() {
            if is_outer_closing_boundary(input.peek().as_ref()) {
                return Err(Rich::custom(
                    input.span_from_cursor(&end_start),
                    missing_end_message.clone(),
                ));
            }

            let checkpoint = input.save();
            let probe =
                mode_item_parser(body_mode, state, math_content.clone(), text_content.clone())
                    .padded_by(ws.clone());
            let probe_result = input.parse(probe);
            input.rewind(checkpoint);

            return match probe_result {
                Ok(_) => Err(Rich::custom(
                    input.span_from_cursor(&end_start),
                    missing_end_message.clone(),
                )),
                Err(err) => Err(err),
            };
        }
        let _ = input.parse(ws.clone());

        let end_name = input.parse(env_name_parser()).map_err(|_| {
            input.rewind(body_recovery_start.clone());
            Rich::custom(input.span_from_cursor(&end_start), missing_end_message)
        })?;
        let end_span = input.span_from_cursor(&end_start);

        if end_name != name {
            input.rewind(body_recovery_start);
            return Err(custom_error(
                end_span,
                format!(
                    "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
                    expected_end, end_name
                ),
                ParseDiagnosticKind::EnvironmentNameMismatch,
            ));
        }

        let span = input.span_from_cursor(&env_start);
        let (args, mut records, mut diagnostics) = TrackedNode::decompose_args(cmd_args);
        records.extend(prefix_records(BODY, &body));
        diagnostics.extend(body.diagnostics.clone());
        Ok(TrackedNode {
            node: SyntaxNode::Environment {
                name,
                args,
                known,
                body: Box::new(body.node),
            },
            span,
            records,
            diagnostics,
        })
    })
}

// ============================================================================
// Mode Parsers (Math and Text)
// ============================================================================

/// Parse a math atom (group/command/env/char) without scripts.
fn math_atom_parser<'a, P>(
    state: &'a ParserState<'a>,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    let ctx = state.ctx;
    let reject_unknown = state.config.reject_unknown;
    let explicit_group = braced_group_parser(state, ContentMode::Math, group_content.clone());
    let delimited_group = delimited_group_parser(ctx, math_content.clone());
    let environment = environment_parser(
        state,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Math,
    );
    let prefix_command = prefix_command_parser(
        state,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Math,
    );
    let declarative_command =
        declarative_command_parser(state, math_content, text_content, ContentMode::Math);
    let delimiter_control_command = delimiter_control_command_parser(ctx);
    let unknown_command = unknown_command_parser(ctx, reject_unknown);
    let fallback = choice((
        explicit_group,
        environment.clone(),
        escaped_symbol(),
        declarative_command.clone(),
        prefix_command.clone(),
        delimiter_control_command,
        unknown_command,
        active_char(),
        math_prime(),
        math_char(),
    ));

    custom(move |input| match input.peek() {
        Some(Token::ControlSeq(name)) if name == "left" => input.parse(delimited_group.clone()),
        Some(Token::ControlSeq(name)) if name == "begin" => input.parse(environment.clone()),
        Some(Token::ControlSeq(name))
            if matches!(
                ctx.lookup_command(name.as_str(), ContentMode::Math),
                Some(meta)
                    if meta.kind == CommandKind::Declarative
            ) =>
        {
            input.parse(declarative_command.clone())
        }
        Some(Token::ControlSeq(name))
            if matches!(
                ctx.lookup_command(name.as_str(), ContentMode::Math),
                Some(meta)
                    if meta.kind == CommandKind::Prefix
            ) =>
        {
            input.parse(prefix_command.clone())
        }
        _ => input.parse(fallback.clone()),
    })
}

/// Wrap a base atom with script parsing (`^`, `_`, primes).
///
/// This parser allows leading whitespace before script atoms but does not
/// consume trailing whitespace after the parsed item.
fn scripted_atom_parser<'a, P>(
    atom: P,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();
    let atom_for_scripts = ws.ignore_then(atom.clone());
    custom(move |input| {
        let start = input.cursor();
        let components = parse_scripted_components(input, atom_for_scripts.clone())?;

        if components.subscript.is_none() && components.superscript.is_none() {
            return Ok(components.base);
        }

        let span = input.span_from_cursor(&start);
        let mut records = Vec::new();
        let mut diagnostics = Vec::new();
        records.extend(prefix_records(BASE, &components.base));
        diagnostics.extend(components.base.diagnostics.clone());
        if let Some(sub) = &components.subscript {
            records.extend(prefix_records(SUB, sub));
            diagnostics.extend(sub.diagnostics.clone());
        }
        if let Some(sup_node) = &components.superscript {
            records.extend(prefix_records(SUP, sup_node));
            diagnostics.extend(sup_node.diagnostics.clone());
        }
        Ok(TrackedNode {
            node: SyntaxNode::Scripted {
                base: Box::new(components.base.node),
                subscript: components.subscript.map(|t| Box::new(t.node)),
                superscript: components.superscript.map(|t| Box::new(t.node)),
            },
            span,
            records,
            diagnostics,
        })
    })
}

fn inline_math_group_from_tracked(tracked: TrackedNode, span: SimpleSpan) -> TrackedNode {
    let node = match tracked.node {
        SyntaxNode::Group { mode, children, .. } => SyntaxNode::Group {
            mode,
            kind: GroupKind::InlineMath,
            children,
        },
        other => other,
    };
    TrackedNode {
        node,
        span,
        records: tracked.records,
        diagnostics: tracked.diagnostics,
    }
}

fn math_content_parser_with_extra_control_stop<'a>(
    state: &'a ParserState<'a>,
    text_content: ContentParser<'a>,
    recoverable_src: Option<&'a str>,
    extra_control_stop: &'static str,
) -> ContentParser<'a> {
    recursive(move |group_content| {
        let ws = insignificant_whitespace();
        let math_content = group_content.clone().boxed();
        let base_item = math_item_node_parser(
            state,
            group_content,
            math_content.clone(),
            text_content.clone(),
        );
        let normal_item = match recoverable_src {
            Some(src) if !state.config.abort_on_error => recoverable_content_item_parser(
                state,
                ContentMode::Math,
                src,
                base_item.padded_by(ws.clone()),
                is_math_control_paren_hard_stop,
            )
            .boxed(),
            _ => base_item.padded_by(ws.clone()).boxed(),
        };
        math_group_content_parser(
            state,
            normal_item,
            math_content,
            text_content.clone(),
            Some(extra_control_stop),
        )
        .padded_by(ws)
    })
    .boxed()
}

/// Parse a text atom (text chunk, inline math, group, command, env).
fn text_atom_parser<'a, P>(
    state: &'a ParserState<'a>,
    group_content: P,
    math_content: ContentParser<'a>,
    control_inline_math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    let ctx = state.ctx;
    let reject_unknown = state.config.reject_unknown;
    let dollar_inline_math = just(Token::MathShift)
        .ignore_then(implicit_group_parser(
            ContentMode::Math,
            math_content.clone(),
        ))
        .then_ignore(just(Token::MathShift))
        .map_with(|tracked, e| inline_math_group_from_tracked(tracked, e.span()));
    let control_sequence_inline_math = control_seq("(")
        .ignore_then(implicit_group_parser(
            ContentMode::Math,
            control_inline_math_content,
        ))
        .then_ignore(control_seq(")"))
        .map_with(|tracked, e| inline_math_group_from_tracked(tracked, e.span()));
    let inline_math = choice((dollar_inline_math, control_sequence_inline_math));

    let scripted_marker = select! {
        Token::Superscript => (),
        Token::Subscript => (),
    }
    .try_map(|_, span| {
        Err(custom_error(
            span,
            "Scripted syntax is not allowed in Text mode",
            ParseDiagnosticKind::TextScriptError,
        ))
    });

    let explicit_group = braced_group_parser(state, ContentMode::Text, group_content);
    let environment = environment_parser(
        state,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Text,
    );
    let prefix_command = prefix_command_parser(
        state,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Text,
    );
    let declarative_command =
        declarative_command_parser(state, math_content, text_content, ContentMode::Text);
    let unknown_command = unknown_command_parser(ctx, reject_unknown);

    let control_seq_fallback = choice((
        inline_math.clone(),
        environment.clone(),
        escaped_symbol(),
        declarative_command.clone(),
        prefix_command.clone(),
        unknown_command,
    ));

    let fallback = choice((
        text_chunk(),
        inline_math,
        explicit_group,
        scripted_marker,
        active_char(),
    ));

    custom(move |input| match input.peek() {
        Some(Token::ControlSeq(_)) => input.parse(control_seq_fallback.clone()),
        _ => input.parse(fallback.clone()),
    })
}

/// Build math-mode group content (leading items + optional infix tail).
fn math_group_content_parser<'a, P>(
    state: &'a ParserState<'a>,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    extra_control_stop: Option<&'static str>,
) -> impl Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    let ctx = state.ctx;
    let reject_unknown = state.config.reject_unknown;
    let ws = insignificant_whitespace();
    let stop_infix = infix_guard(ctx, ContentMode::Math);
    let stop_boundary = ws
        .clone()
        .ignore_then(stop_infix)
        .or(ws.clone().ignore_then(control_seq("right")))
        .or(ws.clone().ignore_then(control_seq("end")))
        .or(ws
            .clone()
            .ignore_then(optional_control_seq(extra_control_stop)))
        .rewind();
    let guarded_item = stop_boundary.clone().not().ignore_then(normal_item.clone());
    let ws_for_leading = ws.clone();
    let leading = custom(move |input| {
        let mut items = Vec::new();

        loop {
            let first_item_metadata = input.save();
            let _ = input.parse(ws_for_leading.clone());
            let preserve_first_item_error = matches!(
                input.peek(),
                Some(Token::ControlSeq(name)) if name == "left"
            );
            let natural_end = matches!(
                input.peek().as_ref(),
                None | Some(Token::RBrace) | Some(Token::MathShift)
            );
            input.rewind(first_item_metadata);
            if natural_end || input.parse(stop_boundary.clone()).is_ok() {
                break;
            }

            let checkpoint = input.save();
            match input.parse(guarded_item.clone()) {
                Ok(item) => items.push(item),
                Err(err) => {
                    if preserve_first_item_error && is_direct_left_group_error(&err) {
                        return Err(err);
                    }
                    input.rewind(checkpoint);
                    return Err(err);
                }
            }
        }

        Ok(items)
    });

    let ws_for_optional_infix = ws.clone();
    let math_content_for_infix = math_content.clone();
    let text_content_for_infix = text_content.clone();
    let optional_infix_tail = custom(move |input| {
        let checkpoint = input.save();
        let _ = input.parse(ws_for_optional_infix.clone());
        let has_infix = matches!(input.peek(),
        Some(Token::ControlSeq(name))
            if matches!(
                lookup_command_for_parse(ctx, name.as_str(), ContentMode::Math),
                ModeLookup::Found(meta) if meta.kind == CommandKind::Infix
            ));
        input.rewind(checkpoint);

        if has_infix {
            let ws = insignificant_whitespace();
            let cmd_start = input.cursor();
            let cmd_start_byte = input.span_from_cursor(&cmd_start).start;
            let (name, meta) = command_head_parser(
                input,
                ctx,
                CommandKind::Infix,
                ContentMode::Math,
                reject_unknown,
            )?;

            let args = parse_argument_slots(
                input,
                state,
                math_content_for_infix.clone(),
                text_content_for_infix.clone(),
                meta.argspec.args,
                "infix command argument",
            )?;

            let normal_item = math_item_parser(
                state,
                math_content_for_infix.clone(),
                text_content_for_infix.clone(),
            )
            .padded_by(ws.clone())
            .boxed();
            let stop_boundary = ws
                .clone()
                .ignore_then(control_seq("right"))
                .or(ws.clone().ignore_then(control_seq("end")))
                .or(ws
                    .clone()
                    .ignore_then(optional_control_seq(extra_control_stop)))
                .rewind();
            let guarded_item = stop_boundary.clone().not().ignore_then(normal_item.clone());
            let buildrel_over_tail =
                name == "over" && source_has_buildrel_tail_before_over(state.src, cmd_start_byte);
            let mut right_items = Vec::new();
            let mut trailing_items = Vec::new();

            if buildrel_over_tail {
                let checkpoint = input.save();
                let _ = input.parse(ws.clone());
                let natural_end = matches!(
                    input.peek().as_ref(),
                    None | Some(Token::RBrace) | Some(Token::MathShift)
                );
                if natural_end || input.parse(stop_boundary.clone()).is_ok() {
                    input.rewind(checkpoint.clone());
                }

                input.rewind(checkpoint.clone());
                match input.parse(guarded_item.clone()) {
                    Ok(item) => right_items.push(item),
                    Err(err) => {
                        input.rewind(checkpoint);
                        return Err(err);
                    }
                }

                let checkpoint = input.save();
                let _ = input.parse(ws.clone());
                let natural_end = matches!(
                    input.peek().as_ref(),
                    None | Some(Token::RBrace) | Some(Token::MathShift)
                );
                if natural_end || input.parse(stop_boundary.clone()).is_ok() {
                    input.rewind(checkpoint);
                } else {
                    input.rewind(checkpoint);
                    trailing_items = input.parse(math_content_for_infix.clone())?;
                }
            } else {
                loop {
                    let checkpoint = input.save();
                    let _ = input.parse(ws.clone());
                    let token_start = input.cursor();
                    let natural_end = matches!(
                        input.peek().as_ref(),
                        None | Some(Token::RBrace) | Some(Token::MathShift)
                    );
                    if natural_end {
                        input.rewind(checkpoint);
                        break;
                    }

                    if input.parse(stop_boundary.clone()).is_ok() {
                        break;
                    }

                    let ambiguous_infix = match input.peek() {
                        Some(Token::ControlSeq(name))
                            if input.parse(infix_guard(ctx, ContentMode::Math)).is_ok() =>
                        {
                            Some(name.clone())
                        }
                        _ => None,
                    };

                    if let Some(name) = ambiguous_infix {
                        return Err(custom_error(
                            input.span_from_cursor(&token_start),
                            format!("Ambiguous use of \\{}", name),
                            ParseDiagnosticKind::AmbiguousInfix,
                        ));
                    }

                    input.rewind(checkpoint.clone());

                    match input.parse(guarded_item.clone()) {
                        Ok(item) => right_items.push(item),
                        Err(err) => {
                            input.rewind(checkpoint);
                            return Err(err);
                        }
                    }
                }
            }

            Ok(Some((
                (name, args, cmd_start_byte),
                right_items,
                trailing_items,
            )))
        } else {
            Ok(None)
        }
    });

    leading
        .then(optional_infix_tail)
        .try_map(|(leading, infix_tail), content_span| {
            if let Some((infix_info, right_items, trailing_items)) = infix_tail {
                let (name, args, _cmd_start) = infix_info;

                let left_span = items_span(&leading, content_span.start);
                let left = TrackedNode::fold(ContentMode::Math, leading, left_span);
                let right_span = items_span(&right_items, content_span.end);
                let right = TrackedNode::fold(ContentMode::Math, right_items, right_span);

                let (args, mut records, mut diagnostics) = TrackedNode::decompose_args(args);
                records.extend(prefix_records(LEFT, &left));
                records.extend(prefix_records(RIGHT, &right));
                diagnostics.extend(left.diagnostics.clone());
                diagnostics.extend(right.diagnostics.clone());

                // Span covers from left start to right end.
                let infix_span = SimpleSpan::new((), left.span.start..right.span.end);
                let infix_node = TrackedNode {
                    node: SyntaxNode::Infix {
                        name,
                        args,
                        left: Box::new(left.node),
                        right: Box::new(right.node),
                    },
                    span: infix_span,
                    records,
                    diagnostics,
                };

                let mut items = vec![infix_node];
                items.extend(trailing_items);
                Ok(items)
            } else {
                Ok(leading)
            }
        })
}

/// Build text-mode group content as an ordinary item sequence.
fn text_group_content_parser<'a, P>(
    normal_item: P,
) -> impl Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();

    let stop_boundary = ws.clone().ignore_then(control_seq("end"));

    custom(move |input| {
        let mut items = Vec::new();

        loop {
            let checkpoint = input.save();
            let _ = input.parse(ws.clone());
            let natural_end = matches!(input.peek().as_ref(), None | Some(Token::RBrace));
            input.rewind(checkpoint.clone());
            let at_stop = input.parse(stop_boundary.clone()).is_ok();
            input.rewind(checkpoint.clone());
            if natural_end || at_stop {
                break;
            }

            match input.parse(normal_item.clone()) {
                Ok(item) => items.push(item),
                Err(err) => {
                    input.rewind(checkpoint);
                    return Err(err);
                }
            }
        }

        Ok(items)
    })
}

/// Construct mutually recursive math/text content parsers.
///
/// Math content may embed text content (via `\text`-family commands) and
/// vice versa (via inline math `$...$`). Both parsers are declared with
/// [`chumsky::recursive`] and wired to each other before being returned
/// as boxed parsers. Avoid [`Recursive::declare`] here: capturing declared
/// parser clones inside their own definitions forms strong `Rc` cycles in
/// chumsky 0.11 and leaks one parser graph per parse call.
fn mode_content_parsers<'a>(state: &'a ParserState<'a>) -> (ContentParser<'a>, ContentParser<'a>) {
    let math = recursive(move |group_content| {
        let ws = insignificant_whitespace();
        let math_content = group_content.clone().boxed();
        let text_content = recursive({
            let math_content = group_content.clone().boxed();
            move |group_content| {
                let text_content = group_content.clone().boxed();
                let control_inline_math_content = math_content_parser_with_extra_control_stop(
                    state,
                    text_content.clone(),
                    None,
                    ")",
                );
                let normal_item = text_atom_parser(
                    state,
                    group_content,
                    math_content.clone(),
                    control_inline_math_content,
                    text_content.clone(),
                );
                text_group_content_parser(normal_item)
            }
        })
        .boxed();
        let base_item = math_item_node_parser(
            state,
            group_content,
            math_content.clone(),
            text_content.clone(),
        );
        let normal_item = base_item.padded_by(ws.clone()).boxed();
        math_group_content_parser(state, normal_item, math_content, text_content, None)
            .padded_by(ws)
    })
    .boxed();

    let text = recursive({
        let math_content = math.clone();
        move |group_content| {
            let text_content = group_content.clone().boxed();
            let control_inline_math_content =
                math_content_parser_with_extra_control_stop(state, text_content.clone(), None, ")");
            let normal_item = text_atom_parser(
                state,
                group_content,
                math_content.clone(),
                control_inline_math_content,
                text_content.clone(),
            );
            text_group_content_parser(normal_item)
        }
    })
    .boxed();

    (math, text)
}

fn mode_content_parsers_with_source<'a>(
    state: &'a ParserState<'a>,
    src: &'a str,
) -> (ContentParser<'a>, ContentParser<'a>) {
    let math = recursive(move |group_content| {
        let ws = insignificant_whitespace();
        let math_content = group_content.clone().boxed();
        let text_content = recursive({
            let math_content = group_content.clone().boxed();
            move |group_content| {
                let text_content = group_content.clone().boxed();
                let control_inline_math_content = math_content_parser_with_extra_control_stop(
                    state,
                    text_content.clone(),
                    Some(src),
                    ")",
                );
                let base_item = text_atom_parser(
                    state,
                    group_content,
                    math_content.clone(),
                    control_inline_math_content,
                    text_content.clone(),
                );
                let normal_item = if state.config.abort_on_error {
                    base_item.boxed()
                } else {
                    recoverable_content_item_parser(
                        state,
                        ContentMode::Text,
                        src,
                        base_item,
                        is_text_hard_stop,
                    )
                    .boxed()
                };
                text_group_content_parser(normal_item)
            }
        })
        .boxed();
        let base_item = math_item_node_parser(
            state,
            group_content,
            math_content.clone(),
            text_content.clone(),
        );
        let normal_item = if state.config.abort_on_error {
            // Math items still need per-item whitespace so infix/declarative tails see the command head.
            base_item.padded_by(ws.clone()).boxed()
        } else {
            recoverable_content_item_parser(
                state,
                ContentMode::Math,
                src,
                base_item.padded_by(ws.clone()),
                is_math_hard_stop,
            )
            .boxed()
        };
        math_group_content_parser(state, normal_item, math_content, text_content, None)
            .padded_by(ws)
    })
    .boxed();

    let text = recursive({
        let math_content = math.clone();
        move |group_content| {
            let text_content = group_content.clone().boxed();
            let control_inline_math_content = math_content_parser_with_extra_control_stop(
                state,
                text_content.clone(),
                Some(src),
                ")",
            );
            let base_item = text_atom_parser(
                state,
                group_content,
                math_content.clone(),
                control_inline_math_content,
                text_content.clone(),
            );
            let normal_item = if state.config.abort_on_error {
                base_item.boxed()
            } else {
                recoverable_content_item_parser(
                    state,
                    ContentMode::Text,
                    src,
                    base_item,
                    is_text_hard_stop,
                )
                .boxed()
            };
            text_group_content_parser(normal_item)
        }
    })
    .boxed();

    (math, text)
}

/// Construct top-level math/text group parsers from content parsers.
fn mode_group_parsers<'a>(state: &'a ParserState<'a>) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers(state);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

fn mode_group_parsers_with_source<'a>(
    state: &'a ParserState<'a>,
    src: &'a str,
) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers_with_source(state, src);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

/// Build the top-level math-mode parser.
///
/// Returns a boxed parser that produces an implicit math-mode group
/// wrapping all parsed items. This is the parser used by
/// [`ParseContext::parse`](crate::parse::ParseContext::parse).
pub(crate) fn math_block_parser<'a>(state: &'a ParserState<'a>) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers(state);
    math_parser
}

pub(crate) fn math_block_parser_with_source<'a>(
    state: &'a ParserState<'a>,
    src: &'a str,
) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers_with_source(state, src);
    math_parser
}

pub(crate) fn content_block_parser_with_source<'a>(
    mode: ContentMode,
    state: &'a ParserState<'a>,
    src: &'a str,
) -> NodeParser<'a> {
    match mode {
        ContentMode::Math => math_block_parser_with_source(state, src),
        ContentMode::Text => {
            let (_, text_parser) = mode_group_parsers_with_source(state, src);
            text_parser
        }
    }
}
