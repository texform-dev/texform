//! Chumsky-based parser for LaTeX formulas.
//!
//! The parser is structured as a hierarchy of combinator parsers that mirror
//! the LaTeX grammar. At the top level, the math block parser produces an
//! implicit math-mode group wrapping all top-level items.
//!
//! # Parser layers (bottom-up)
//!
//! 1. **Atoms** — characters, escaped symbols, `~`, braced groups, delimited
//!    groups (`\left…\right`), prefix commands, environments, unknown commands.
//! 2. **Scripted atoms** — atoms wrapped with subscript / superscript / prime
//!    handling.
//! 3. **Group content** — a sequence of items optionally followed by infix and
//!    declarative command tails. Math and text modes each have their own
//!    content parser.
//! 4. **Mode entry** — the math/text block parsers wrap the corresponding
//!    content parser in an implicit group.
//!
//! Math and text content parsers are mutually recursive (a math-mode command
//! may take a text-mode argument and vice versa). The recursion is resolved
//! through [`chumsky::recursive`].

#[path = "parser_arguments.rs"]
mod arguments;

use chumsky::{
    input::{Cursor, InputRef, Stream},
    label::LabelError,
    prelude::*,
    recovery::via_parser,
};
use logos::Logos;

use crate::knowledge::{CommandKind, CommandMeta};
use crate::lexer::Token;
use crate::parse::ParseContext;
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
const SCOPE: &str = "scope";
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
}

impl TrackedNode {
    /// Wrap a syntax node with no descendant records.
    pub(crate) fn leaf(node: SyntaxNode, span: SimpleSpan) -> Self {
        Self {
            node,
            span,
            records: Vec::new(),
        }
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
        }
    }

    /// Finalize into the root-prefixed record list consumed by `ParseResult`.
    pub(crate) fn finish_root(self) -> (SyntaxNode, SimpleSpan, Vec<RelativeSpanEntry>) {
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
        (self.node, self.span, records)
    }

    /// Extract syntax nodes and `child.N` records from tracked children.
    fn decompose_children(children: Vec<TrackedNode>) -> (Vec<SyntaxNode>, Vec<RelativeSpanEntry>) {
        let mut records = Vec::new();
        let mut nodes = Vec::with_capacity(children.len());
        for (index, child) in children.iter().enumerate() {
            records.extend(prefix_records(&format!("{CHILD}.{index}"), child));
        }
        for child in children {
            nodes.push(child.node);
        }
        (nodes, records)
    }

    /// Extract argument slots and `arg.N` / `arg.N.content` records.
    fn decompose_args(
        slots: Vec<TrackedArgumentSlot>,
    ) -> (Vec<ArgumentSlot>, Vec<RelativeSpanEntry>) {
        let mut records = Vec::new();
        for (index, arg) in slots.iter().enumerate() {
            if let Some(arg_span) = arg.span {
                let arg_path = format!("{ARG}.{index}");
                records.push(RelativeSpanEntry {
                    path: arg_path.clone(),
                    span: arg_span,
                });
                if let Some(content) = &arg.content {
                    records.extend(prefix_records(&format!("{arg_path}.{CONTENT}"), content));
                }
            }
        }
        let slots = slots.into_iter().map(|a| a.slot).collect();
        (slots, records)
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
                let (nodes, records) = TrackedNode::decompose_children(items);
                TrackedNode {
                    node: SyntaxNode::Group {
                        mode,
                        kind: GroupKind::Implicit,
                        children: nodes,
                    },
                    span,
                    records,
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
/// Return type of infix/declarative tail parsers.
///
/// The tuple is `((name, args, cmd_start_byte), right_or_scope_items)`.
/// `cmd_start_byte` is the byte offset where the command control sequence
/// begins, so callers can compute the full span from the command head
/// through the end of the right/scope items.
type TailParseOutput = ((String, Vec<TrackedArgumentSlot>, usize), Vec<TrackedNode>);

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
        Token::Char('.') => Delimiter::None,
        Token::Char(c) if matches!(c, '(' | ')' | '[' | ']' | '|' | '<' | '>' | '/' | '\\')
            => Delimiter::Char(c),
        // Raw square brackets are tokenized separately so optional arguments
        // can be recognized without backtracking, but they still need to work
        // as plain delimiters after \left / \right.
        Token::LBracket => Delimiter::Char('['),
        Token::RBracket => Delimiter::Char(']'),
        Token::ControlSeq(name) if ctx.lookup_delimiter_control(name.as_str()).is_some() => {
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

/// Parse and coalesce consecutive text characters/whitespace into a single `Text` node.
fn text_chunk<'a>() -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => c,
        Token::Whitespaces => ' ',
        Token::LBracket => '[',
        Token::RBracket => ']',
    }
    .repeated()
    .at_least(1)
    .collect::<Vec<_>>()
    .labelled("text")
    .map(|chars| {
        let mut buf = String::new();
        let mut last_was_space = false;
        for ch in chars {
            if ch == ' ' {
                if !last_was_space {
                    buf.push(' ');
                    last_was_space = true;
                }
            } else {
                buf.push(ch);
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

/// Build an implicit group from a content parser.
fn implicit_group_parser<'a, P>(
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    content.map_with(move |children, e| {
        let (nodes, records) = TrackedNode::decompose_children(children);
        TrackedNode {
            node: SyntaxNode::Group {
                mode,
                kind: GroupKind::Implicit,
                children: nodes,
            },
            span: e.span(),
            records,
        }
    })
}

/// Parse an explicit `{...}` group with the given content parser.
fn braced_group_parser<'a, P>(
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    just(Token::LBrace)
        .ignore_then(content)
        .then_ignore(just(Token::RBrace))
        .map_with(move |children, e| {
            let (nodes, records) = TrackedNode::decompose_children(children);
            TrackedNode {
                node: SyntaxNode::Group {
                    mode,
                    kind: GroupKind::Explicit,
                    children: nodes,
                },
                span: e.span(),
                records,
            }
        })
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
                return Err(err);
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
            return Err(err);
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
                return Err(err);
            }
        };

        let span = input.span_from_cursor(&group_start);
        let (nodes, records) = TrackedNode::decompose_children(children);
        Ok(TrackedNode {
            node: SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Delimited { left, right },
                children: nodes,
            },
            span,
            records,
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
            Some(Token::Superscript) | Some(Token::Subscript) | Some(Token::Prime(_)) => {
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
                let tracked = input.parse(atom_for_scripts.clone())?;
                let span = input.span_from_cursor(&marker_start);
                Some(("sup", TrackedNode { span, ..tracked }))
            }
            Some(Token::Subscript) => {
                input.next();
                let tracked = input.parse(atom_for_scripts.clone())?;
                let span = input.span_from_cursor(&marker_start);
                Some(("sub", TrackedNode { span, ..tracked }))
            }
            Some(Token::Prime(_)) => {
                let count = match input.next() {
                    Some(Token::Prime(n)) => n,
                    _ => unreachable!("peek ensured prime token"),
                };
                let prime_span = input.span_from_cursor(&marker_start);
                let prime_node = if count == 1 {
                    SyntaxNode::Char('\'')
                } else {
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: (0..count).map(|_| SyntaxNode::Char('\'')).collect(),
                    }
                };
                Some(("prime", TrackedNode::leaf(prime_node, prime_span)))
            }
            _ => None,
        };

        let Some((kind, tracked)) = marker else {
            input.rewind(checkpoint);
            break;
        };

        match kind {
            "sub" => {
                if subscript.is_some() {
                    return Err(
                        input.err_since(&marker_start, "Double subscripts: use braces to clarify")
                    );
                }
                subscript = Some(tracked);
            }
            "sup" => {
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
            "prime" => {
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
            _ => unreachable!("unsupported script kind"),
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

/// Parse a LaTeX math formula using the default all-packages knowledge base.
///
/// Returns the root [`SyntaxNode`] together with its byte span on success,
/// or a list of rich diagnostics on failure. For partial-parse semantics
/// (result + diagnostics), use [`ParseContext::parse`](crate::parse::ParseContext::parse)
/// instead.
pub fn parse(src: &str, strict: bool) -> Result<Spanned<SyntaxNode>, Vec<Rich<'_, Token>>> {
    let token_stream = build_token_stream(src);
    math_block_parser(ParseContext::all_packages_shared(), strict)
        .map_with(|tracked, e| (tracked.node, e.span()))
        .then_ignore(end())
        .parse(token_stream)
        .into_result()
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
    ctx: &'a ParseContext,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> NodeParser<'a> {
    match mode {
        ContentMode::Math => math_item_parser(ctx, math_content, text_content, strict).boxed(),
        ContentMode::Text => text_item_parser(ctx, math_content, text_content, strict).boxed(),
    }
}

enum LookupResult<'a, T> {
    Active(&'a T),
    KnownButDisallowed,
    Unknown,
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
) -> LookupResult<'a, CommandMeta> {
    if let Some(meta) = ctx.lookup_command(name, mode) {
        LookupResult::Active(meta)
    } else if ctx.lookup_command(name, other_mode(mode)).is_some() {
        LookupResult::KnownButDisallowed
    } else {
        LookupResult::Unknown
    }
}

fn lookup_env_for_parse<'a>(
    ctx: &'a ParseContext,
    name: &str,
    mode: ContentMode,
) -> LookupResult<'a, texform_specs::specs::EnvMeta> {
    if let Some(meta) = ctx.lookup_env(name, mode) {
        LookupResult::Active(meta)
    } else if ctx.lookup_env(name, other_mode(mode)).is_some() {
        LookupResult::KnownButDisallowed
    } else {
        LookupResult::Unknown
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

fn is_text_hard_stop(token: &Token) -> bool {
    matches!(token, Token::RBrace | Token::MathShift)
        || matches!(token, Token::ControlSeq(name) if name == "end")
}

fn is_direct_left_group_error(message: &str) -> bool {
    matches!(
        message,
        "invalid \\left delimiter"
            | "missing \\right for \\left-delimited group"
            | "invalid \\right delimiter"
    )
}

fn is_direct_environment_header_error(message: &str) -> bool {
    message.starts_with("Unknown environment: ")
        || (message.starts_with("Environment ") && message.contains(" is not allowed in "))
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
) -> String {
    let is_generic_parse_error = matches!(
        message.as_str(),
        "found '}' expected something else" | "found '}' expected something else, or end of input"
    );
    if !is_generic_parse_error {
        return message;
    }

    if let Some(env_name) = leading_begin_environment_name(src)
        && ctx.lookup_env(env_name.as_str(), current_mode).is_none()
        && ctx
            .lookup_env(env_name.as_str(), other_mode(current_mode))
            .is_some()
    {
        return format!(
            "Environment {} is not allowed in {} mode",
            env_name, current_mode
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
                return format!(
                    "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
                    expected, env_name
                );
            }
        }

        index += 1;
    }

    message
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

fn recoverable_content_item_parser<'a, P>(
    ctx: &'a ParseContext,
    current_mode: ContentMode,
    src: &'a str,
    item: P,
    is_hard_stop: fn(&Token) -> bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    custom(move |input| {
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
        let direct_left_group_error = item_starts_with_left
            && matches!(err.reason(), chumsky::error::RichReason::Custom(message) if is_direct_left_group_error(message));
        let direct_environment_header_error = item_starts_with_begin
            && matches!(err.reason(), chumsky::error::RichReason::Custom(message) if is_direct_environment_header_error(message));
        if direct_left_group_error || direct_environment_header_error {
            return Err(err);
        }
        let failure_environment_stack = scan_environment_stack_before(src, err.span().end);
        input.rewind(checkpoint);

        if is_hard_stop_after_whitespace(input, is_hard_stop) {
            return Err(err);
        }

        let message = opening_environment
            .as_ref()
            .and_then(|env_name| {
                if ctx.lookup_env(env_name.as_str(), current_mode).is_none()
                    && ctx
                        .lookup_env(env_name.as_str(), other_mode(current_mode))
                        .is_some()
                {
                    Some(format!(
                        "Environment {} is not allowed in {} mode",
                        env_name, current_mode
                    ))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| match err.reason() {
                chumsky::error::RichReason::ExpectedFound { .. } => format!("{err}"),
                chumsky::error::RichReason::Custom(message) => message.clone(),
            });
        let recovery_src = src.get(item_start_index..).unwrap_or(src);
        let message = normalize_recovery_message(ctx, current_mode, recovery_src, message);

        let recovery_parser = custom({
            let message = message.clone();
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

                    if message.starts_with("Command \\") && message.contains(" is not allowed in ")
                    {
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

                    if let Some(open_name) = opening_environment.as_ref() {
                        if let Some(end_name) = peek_environment_name_at_cursor(input, "end") {
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

        input.parse(item.clone().recover_with(via_parser(recovery_parser)))
    })
}

/// Consume a control-sequence token, look it up in the KB, and validate that
/// it matches the `expected_kind` and is allowed in `current_mode`.
fn command_head_parser<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    ctx: &'parse ParseContext,
    expected_kind: CommandKind,
    current_mode: ContentMode,
    strict: bool,
) -> Result<(String, &'parse CommandMeta), Rich<'src, Token>> {
    let cmd_start = input.cursor();
    let token = input.next();
    let name = match token {
        Some(Token::ControlSeq(name)) => name,
        Some(_) => return Err(input.err_since(&cmd_start, "not a command")),
        None => return Err(input.err_since(&cmd_start, "not a command")),
    };

    let cmd_span = input.span_from_cursor(&cmd_start);

    let meta = match lookup_command_for_parse(ctx, &name, current_mode) {
        LookupResult::Active(meta) if meta.kind == expected_kind => meta,
        LookupResult::Active(_) => {
            return Err(Rich::custom(
                cmd_span,
                format!("not {}", expected_kind.label()),
            ));
        }
        LookupResult::KnownButDisallowed => {
            return Err(Rich::custom(
                cmd_span,
                format!("Command \\{} is not allowed in {} mode", name, current_mode),
            ));
        }
        LookupResult::Unknown => {
            if strict {
                return Err(Rich::custom(
                    cmd_span,
                    format!("Unknown command: \\{}", name),
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

/// Lookahead guard that succeeds when the next token is an infix or
/// declarative command. Used as a stop condition for leading-item collection.
fn math_infix_or_decl_guard<'a>(
    ctx: &'a ParseContext,
    current_mode: ContentMode,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if matches!(lookup_command_for_parse(ctx, name.as_str(), current_mode),
                LookupResult::Active(meta) if matches!(meta.kind, CommandKind::Infix | CommandKind::Declarative))
                || ctx.lookup_command(name.as_str(), other_mode(current_mode))
                    .map(|meta| matches!(meta.kind, CommandKind::Infix | CommandKind::Declarative))
                    .unwrap_or(false) => ()
    }
    .rewind()
}

/// Guard used to stop content parsing before declarative commands.
fn declarative_guard<'a>(
    ctx: &'a ParseContext,
    current_mode: ContentMode,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if matches!(lookup_command_for_parse(ctx, name.as_str(), current_mode),
                LookupResult::Active(meta) if meta.kind == CommandKind::Declarative)
                || ctx.lookup_command(name.as_str(), other_mode(current_mode))
                    .map(|meta| meta.kind == CommandKind::Declarative)
                    .unwrap_or(false) => ()
    }
    .rewind()
}

fn infix_guard<'a>(
    ctx: &'a ParseContext,
    current_mode: ContentMode,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if matches!(lookup_command_for_parse(ctx, name.as_str(), current_mode),
                LookupResult::Active(meta) if meta.kind == CommandKind::Infix)
                || ctx.lookup_command(name.as_str(), other_mode(current_mode))
                    .map(|meta| meta.kind == CommandKind::Infix)
                    .unwrap_or(false) => ()
    }
    .rewind()
}

/// Parse one math item node (with script handling) without outer spacing policy.
///
/// Callers decide whether to wrap it with padding or stop-guards.
fn math_item_node_parser<'a, P>(
    ctx: &'a ParseContext,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    let atom = math_atom_parser(ctx, group_content, math_content, text_content, strict);
    scripted_atom_parser(atom)
}

/// Parse a single math item in argument contexts.
///
/// This parser does not consume trailing whitespace so the following argument
/// slot can still enforce `no_leading_space`.
fn math_item_parser<'a>(
    ctx: &'a ParseContext,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    let item = math_item_node_parser(
        ctx,
        math_content.clone(),
        math_content,
        text_content,
        strict,
    );

    math_infix_or_decl_guard(ctx, ContentMode::Math)
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .ignore_then(item)
}

/// Parse a single text item (respecting stop guards).
fn text_item_parser<'a>(
    ctx: &'a ParseContext,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    let normal_item = text_atom_parser(
        ctx,
        text_content.clone(),
        math_content,
        text_content,
        strict,
    );

    declarative_guard(ctx, ContentMode::Text)
        .or(control_seq("end"))
        .not()
        .ignore_then(normal_item)
}

// ============================================================================
// Command and Environment Parsers
// ============================================================================

fn prefix_command_parser<'a>(
    ctx: &'a ParseContext,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let cmd_start = input.cursor();
        let (name, meta) =
            match command_head_parser(input, ctx, CommandKind::Prefix, current_mode, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let mut cmd_args: Vec<TrackedArgumentSlot> = Vec::with_capacity(meta.argspec.args.len());
        for spec in meta.argspec.args {
            if !spec.no_leading_space {
                let _ = input.parse(insignificant_whitespace());
            }

            let arg_checkpoint = input.save();
            let arg_start = input.cursor();
            let parser = argument_parser(
                ctx,
                math_content.clone(),
                text_content.clone(),
                spec,
                strict,
            );
            let arg = match input.parse(parser) {
                Ok(arg) => arg,
                Err(err) => {
                    input.rewind(arg_checkpoint.clone());
                    let arg_span = match input.peek() {
                        Some(Token::LBracket) => {
                            let _ = collect_optional_bracketed_tokens(input, false);
                            input.span_from_cursor(&arg_start)
                        }
                        Some(Token::LBrace) => {
                            let _ = collect_braced_tokens(input, true);
                            input.span_from_cursor(&arg_start)
                        }
                        Some(_) => {
                            let _ = input.next();
                            input.span_from_cursor(&arg_start)
                        }
                        None => input.span_from_cursor(&arg_start),
                    };
                    return match err.reason() {
                        chumsky::error::RichReason::Custom(message) => {
                            let mut err = Rich::custom(arg_span, message.clone());
                            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                                &mut err,
                                "command argument",
                                arg_span,
                            );
                            Err(err)
                        }
                        chumsky::error::RichReason::ExpectedFound { .. } => {
                            let mut err = err;
                            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                                &mut err,
                                "command argument",
                                arg_span,
                            );
                            Err(err)
                        }
                    };
                }
            };
            cmd_args.push(arg);
        }

        let span = input.span_from_cursor(&cmd_start);
        let (args, records) = TrackedNode::decompose_args(cmd_args);
        Ok(TrackedNode {
            node: SyntaxNode::Command {
                name,
                args,
                known: true,
            },
            span,
            records,
        })
    })
}

fn unknown_command_parser<'a>(
    ctx: &'a ParseContext,
    strict: bool,
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

        if strict {
            Err(Rich::custom(span, format!("Unknown command: \\{}", name)))
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
    ctx: &'a ParseContext,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let env_start = input.cursor();
        let ws = insignificant_whitespace();

        input.parse(control_seq("begin"))?;
        let _ = input.parse(ws.clone());

        let name_start = input.cursor();
        let name = input.parse(env_name_parser())?;
        let name_span = input.span_from_cursor(&name_start);

        let (cmd_args, known, body_mode) =
            match lookup_env_for_parse(ctx, name.as_str(), current_mode) {
                LookupResult::Active(meta) => {
                    let mut cmd_args: Vec<TrackedArgumentSlot> =
                        Vec::with_capacity(meta.argspec.args.len());
                    for spec in meta.argspec.args {
                        if !spec.no_leading_space {
                            let _ = input.parse(insignificant_whitespace());
                        }

                        let arg_start = input.cursor();
                        let parser = argument_parser(
                            ctx,
                            math_content.clone(),
                            text_content.clone(),
                            spec,
                            strict,
                        );
                        let arg = match input.parse(parser) {
                            Ok(arg) => arg,
                            Err(err) => {
                                let arg_span = err
                                    .contexts()
                                    .next()
                                    .map(|(_, span)| *span)
                                    .unwrap_or_else(|| input.span_from_cursor(&arg_start));
                                return match err.reason() {
                                    chumsky::error::RichReason::Custom(message) => {
                                        let mut err = Rich::custom(arg_span, message.clone());
                                        <Rich<'a, Token> as LabelError<
                                            'a,
                                            TokenStream<'a>,
                                            &str,
                                        >>::in_context(
                                            &mut err, "environment argument", arg_span
                                        );
                                        Err(err)
                                    }
                                    chumsky::error::RichReason::ExpectedFound { .. } => {
                                        let mut err = err;
                                        <Rich<'a, Token> as LabelError<
                                            'a,
                                            TokenStream<'a>,
                                            &str,
                                        >>::in_context(
                                            &mut err, "environment argument", arg_span
                                        );
                                        Err(err)
                                    }
                                };
                            }
                        };
                        cmd_args.push(arg);
                    }

                    (cmd_args, true, meta.body_mode)
                }
                LookupResult::KnownButDisallowed => {
                    return Err(Rich::custom(
                        name_span,
                        format!(
                            "Environment {} is not allowed in {} mode",
                            name, current_mode
                        ),
                    ));
                }
                LookupResult::Unknown => {
                    if strict {
                        return Err(Rich::custom(
                            name_span,
                            format!("Unknown environment: {}", name),
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
            let probe = mode_item_parser(
                body_mode,
                ctx,
                math_content.clone(),
                text_content.clone(),
                strict,
            )
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
            return Err(Rich::custom(
                end_span,
                format!(
                    "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
                    expected_end, end_name
                ),
            ));
        }

        let span = input.span_from_cursor(&env_start);
        let (args, mut records) = TrackedNode::decompose_args(cmd_args);
        records.extend(prefix_records(BODY, &body));
        Ok(TrackedNode {
            node: SyntaxNode::Environment {
                name,
                args,
                known,
                body: Box::new(body.node),
            },
            span,
            records,
        })
    })
}

// ============================================================================
// Mode Parsers (Math and Text)
// ============================================================================

/// Parse a math atom (group/command/env/char) without scripts.
fn math_atom_parser<'a, P>(
    ctx: &'a ParseContext,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    let explicit_group = braced_group_parser(ContentMode::Math, group_content.clone());
    let delimited_group = delimited_group_parser(ctx, math_content.clone());
    let environment = environment_parser(
        ctx,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Math,
        strict,
    );
    let prefix_command =
        prefix_command_parser(ctx, math_content, text_content, ContentMode::Math, strict);
    let unknown_command = unknown_command_parser(ctx, strict);
    let fallback = choice((
        explicit_group,
        environment.clone(),
        escaped_symbol(),
        prefix_command.clone(),
        unknown_command,
        active_char(),
        math_char(),
    ));

    custom(move |input| match input.peek() {
        Some(Token::ControlSeq(name)) if name == "left" => input.parse(delimited_group.clone()),
        Some(Token::ControlSeq(name)) if name == "begin" => input.parse(environment.clone()),
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
        records.extend(prefix_records(BASE, &components.base));
        if let Some(sub) = &components.subscript {
            records.extend(prefix_records(SUB, sub));
        }
        if let Some(sup_node) = &components.superscript {
            records.extend(prefix_records(SUP, sup_node));
        }
        Ok(TrackedNode {
            node: SyntaxNode::Scripted {
                base: Box::new(components.base.node),
                subscript: components.subscript.map(|t| Box::new(t.node)),
                superscript: components.superscript.map(|t| Box::new(t.node)),
            },
            span,
            records,
        })
    })
}

/// Parse a text atom (text chunk, inline math, group, command, env).
fn text_atom_parser<'a, P>(
    ctx: &'a ParseContext,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone + 'a,
{
    let inline_math = just(Token::MathShift)
        .ignore_then(implicit_group_parser(
            ContentMode::Math,
            math_content.clone(),
        ))
        .then_ignore(just(Token::MathShift))
        .map_with(|tracked, e| {
            // Reclassify the implicit group as inline-math; preserve span records.
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
                span: e.span(),
                records: tracked.records,
            }
        });

    let explicit_group = braced_group_parser(ContentMode::Text, group_content);
    let environment = environment_parser(
        ctx,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Text,
        strict,
    );
    let prefix_command =
        prefix_command_parser(ctx, math_content, text_content, ContentMode::Text, strict);
    let unknown_command = unknown_command_parser(ctx, strict);

    let fallback = choice((
        text_chunk(),
        inline_math,
        explicit_group,
        environment.clone(),
        escaped_symbol(),
        prefix_command.clone(),
        unknown_command,
        active_char(),
    ));

    custom(move |input| match input.peek() {
        Some(Token::ControlSeq(name)) if name == "begin" => input.parse(environment.clone()),
        Some(Token::ControlSeq(name))
            if matches!(
                ctx.lookup_command(name.as_str(), ContentMode::Text),
                Some(meta)
                    if meta.kind == CommandKind::Prefix
            ) =>
        {
            input.parse(prefix_command.clone())
        }
        _ => input.parse(fallback.clone()),
    })
}

/// Parse the tail after an infix command: the command head plus right operand items.
fn infix_tail_parser<'a, P>(
    ctx: &'a ParseContext,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    let infix_cmd = custom(move |input| {
        let cmd_start = input.cursor();
        let cmd_start_byte = input.span_from_cursor(&cmd_start).start;
        let (name, meta) =
            match command_head_parser(input, ctx, CommandKind::Infix, ContentMode::Math, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let mut args: Vec<TrackedArgumentSlot> = Vec::with_capacity(meta.argspec.args.len());
        for spec in meta.argspec.args {
            if !spec.no_leading_space {
                let _ = input.parse(insignificant_whitespace());
            }

            let arg_start = input.cursor();
            let parser = argument_parser(
                ctx,
                math_content.clone(),
                text_content.clone(),
                spec,
                strict,
            );
            let arg = match input.parse(parser) {
                Ok(arg) => arg,
                Err(err) => {
                    let arg_span = err
                        .contexts()
                        .next()
                        .map(|(_, span)| *span)
                        .unwrap_or_else(|| input.span_from_cursor(&arg_start));
                    return match err.reason() {
                        chumsky::error::RichReason::Custom(message) => {
                            let mut err = Rich::custom(arg_span, message.clone());
                            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                                &mut err,
                                "infix command argument",
                                arg_span,
                            );
                            Err(err)
                        }
                        chumsky::error::RichReason::ExpectedFound { .. } => {
                            let mut err = err;
                            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                                &mut err,
                                "infix command argument",
                                arg_span,
                            );
                            Err(err)
                        }
                    };
                }
            };
            args.push(arg);
        }

        Ok((name, args, cmd_start_byte))
    });

    let stop_declarative = declarative_guard(ctx, ContentMode::Math);

    let guarded_item = stop_declarative
        .not()
        .then(normal_item)
        .map(|(_, item)| item);
    let right_items = guarded_item
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .labelled("infix right operand")
        .as_context();

    infix_cmd.then(right_items)
}

/// Parse the tail of a declarative command: command head plus scoped items.
fn declarative_tail_parser<'a, P>(
    ctx: &'a ParseContext,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    let decl_cmd = custom(move |input| {
        let cmd_start = input.cursor();
        let cmd_start_byte = input.span_from_cursor(&cmd_start).start;
        let (name, meta) =
            match command_head_parser(input, ctx, CommandKind::Declarative, current_mode, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let mut args: Vec<TrackedArgumentSlot> = Vec::with_capacity(meta.argspec.args.len());
        for spec in meta.argspec.args {
            if !spec.no_leading_space {
                let _ = input.parse(insignificant_whitespace());
            }

            let arg_start = input.cursor();
            let parser = argument_parser(
                ctx,
                math_content.clone(),
                text_content.clone(),
                spec,
                strict,
            );
            let arg = match input.parse(parser) {
                Ok(arg) => arg,
                Err(err) => {
                    let arg_span = err
                        .contexts()
                        .next()
                        .map(|(_, span)| *span)
                        .unwrap_or_else(|| input.span_from_cursor(&arg_start));
                    return match err.reason() {
                        chumsky::error::RichReason::Custom(message) => {
                            let mut err = Rich::custom(arg_span, message.clone());
                            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                                &mut err,
                                "declarative command argument",
                                arg_span,
                            );
                            Err(err)
                        }
                        chumsky::error::RichReason::ExpectedFound { .. } => {
                            let mut err = err;
                            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                                &mut err,
                                "declarative command argument",
                                arg_span,
                            );
                            Err(err)
                        }
                    };
                }
            };
            args.push(arg);
        }

        Ok((name, args, cmd_start_byte))
    });

    let scope_items = normal_item
        .repeated()
        .collect::<Vec<_>>()
        .labelled("declarative scope")
        .as_context();
    decl_cmd.then(scope_items)
}

/// Build math-mode group content (leading items + optional infix/declarative tails).
fn math_group_content_parser<'a, P>(
    ctx: &'a ParseContext,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();
    let stop_infix_or_decl = math_infix_or_decl_guard(ctx, ContentMode::Math);
    let stop_boundary = ws
        .clone()
        .ignore_then(stop_infix_or_decl)
        .or(ws.clone().ignore_then(control_seq("right")))
        .or(ws.clone().ignore_then(control_seq("end")))
        .rewind();
    let guarded_item = stop_boundary.clone().not().ignore_then(normal_item.clone());
    let leading = custom(move |input| {
        let mut items = Vec::new();

        loop {
            let first_item_metadata = input.save();
            let _ = input.parse(ws.clone());
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
                    if preserve_first_item_error
                        && matches!(err.reason(), chumsky::error::RichReason::Custom(message) if is_direct_left_group_error(message))
                    {
                        return Err(err);
                    }
                    input.rewind(checkpoint);
                    return Err(err);
                }
            }
        }

        Ok(items)
    });

    let infix_tail = infix_guard(ctx, ContentMode::Math).ignore_then(infix_tail_parser(
        ctx,
        normal_item.clone(),
        math_content.clone(),
        text_content.clone(),
        strict,
    ));

    let declarative_tail =
        declarative_guard(ctx, ContentMode::Math).ignore_then(declarative_tail_parser(
            ctx,
            normal_item,
            math_content,
            text_content,
            ContentMode::Math,
            strict,
        ));

    leading
        .then(infix_tail.or_not())
        .then(declarative_tail.or_not())
        .try_map(|((leading, infix_tail), declarative_tail), content_span| {
            if let Some((infix_info, right_items)) = infix_tail {
                if leading.is_empty() {
                    return Err(Rich::custom(
                        content_span,
                        "Infix command requires non-empty left operand",
                    ));
                }

                let (name, args, _cmd_start) = infix_info;

                let left_span = items_span(&leading, content_span.start);
                let left = TrackedNode::fold(ContentMode::Math, leading, left_span);
                let right_span = items_span(&right_items, content_span.end);
                let right = TrackedNode::fold(ContentMode::Math, right_items, right_span);

                let (args, mut records) = TrackedNode::decompose_args(args);
                records.extend(prefix_records(LEFT, &left));
                records.extend(prefix_records(RIGHT, &right));

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
                };

                let mut nodes = vec![infix_node];
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_args, decl_cmd_start) = decl_info;
                    let scope_span = items_span(&scope_items, content_span.end);
                    let scope = TrackedNode::fold(ContentMode::Math, scope_items, scope_span);
                    let decl_span = SimpleSpan::new((), decl_cmd_start..scope.span.end);
                    let (decl_args, mut decl_records) = TrackedNode::decompose_args(decl_args);
                    decl_records.extend(prefix_records(SCOPE, &scope));
                    nodes.push(TrackedNode {
                        node: SyntaxNode::Declarative {
                            name: decl_name,
                            args: decl_args,
                            scope: Box::new(scope.node),
                        },
                        span: decl_span,
                        records: decl_records,
                    });
                }
                Ok(nodes)
            } else {
                let mut items = leading;
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_args, decl_cmd_start) = decl_info;
                    let scope_span = items_span(&scope_items, content_span.end);
                    let scope = TrackedNode::fold(ContentMode::Math, scope_items, scope_span);
                    let decl_span = SimpleSpan::new((), decl_cmd_start..scope.span.end);
                    let (decl_args, mut decl_records) = TrackedNode::decompose_args(decl_args);
                    decl_records.extend(prefix_records(SCOPE, &scope));
                    items.push(TrackedNode {
                        node: SyntaxNode::Declarative {
                            name: decl_name,
                            args: decl_args,
                            scope: Box::new(scope.node),
                        },
                        span: decl_span,
                        records: decl_records,
                    });
                }
                Ok(items)
            }
        })
}

/// Build text-mode group content (leading items + optional declarative tail).
fn text_group_content_parser<'a, P>(
    ctx: &'a ParseContext,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, Vec<TrackedNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, TrackedNode, ParserError<'a>> + Clone + 'a,
{
    let stop_declarative = declarative_guard(ctx, ContentMode::Text);
    let ws = insignificant_whitespace();

    let stop_boundary = ws
        .clone()
        .ignore_then(stop_declarative)
        .or(ws.clone().ignore_then(control_seq("end")))
        .rewind();

    let guarded_item = stop_boundary.clone().not().ignore_then(normal_item.clone());
    let leading = custom(move |input| {
        let mut items = Vec::new();

        loop {
            let checkpoint = input.save();
            let _ = input.parse(ws.clone());
            let natural_end = matches!(input.peek().as_ref(), None | Some(Token::RBrace));
            input.rewind(checkpoint.clone());
            if natural_end || input.parse(stop_boundary.clone()).is_ok() {
                break;
            }

            match input.parse(guarded_item.clone()) {
                Ok(item) => items.push(item),
                Err(err) => {
                    input.rewind(checkpoint);
                    return Err(err);
                }
            }
        }

        Ok(items)
    });

    let declarative_tail =
        declarative_guard(ctx, ContentMode::Text).ignore_then(declarative_tail_parser(
            ctx,
            normal_item,
            math_content,
            text_content,
            ContentMode::Text,
            strict,
        ));

    leading
        .then(declarative_tail.or_not())
        .map_with(|(mut leading, declarative_tail), e| {
            if let Some((decl_info, scope_items)) = declarative_tail {
                let (decl_name, decl_args, decl_cmd_start) = decl_info;
                let content_span = e.span();
                let scope_span = items_span(&scope_items, content_span.end);
                let scope = TrackedNode::fold(ContentMode::Text, scope_items, scope_span);
                let decl_span = SimpleSpan::new((), decl_cmd_start..scope.span.end);
                let (decl_args, mut decl_records) = TrackedNode::decompose_args(decl_args);
                decl_records.extend(prefix_records(SCOPE, &scope));
                leading.push(TrackedNode {
                    node: SyntaxNode::Declarative {
                        name: decl_name,
                        args: decl_args,
                        scope: Box::new(scope.node),
                    },
                    span: decl_span,
                    records: decl_records,
                });
            }
            leading
        })
}

/// Construct mutually recursive math/text content parsers.
///
/// Math content may embed text content (via `\text`-family commands) and
/// vice versa (via inline math `$...$`). Both parsers are declared with
/// [`chumsky::recursive`] and wired to each other before being returned
/// as boxed parsers.
fn mode_content_parsers<'a>(
    ctx: &'a ParseContext,
    strict: bool,
) -> (ContentParser<'a>, ContentParser<'a>) {
    let mut math = Recursive::declare();
    let mut text = Recursive::declare();

    let math_for_math = math.clone();
    let text_for_math = text.clone();
    math.define(recursive(move |group_content| {
        let ws = insignificant_whitespace();
        let math_content = math_for_math.clone().boxed();
        let text_content = text_for_math.clone().boxed();
        let normal_item = math_item_node_parser(
            ctx,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        )
        .padded_by(ws.clone());
        math_group_content_parser(ctx, normal_item, math_content, text_content, strict)
            .padded_by(ws)
    }));

    let math_for_text = math.clone();
    let text_for_text = text.clone();
    text.define(recursive(move |group_content| {
        let math_content = math_for_text.clone().boxed();
        let text_content = text_for_text.clone().boxed();
        let normal_item = text_atom_parser(
            ctx,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        );
        text_group_content_parser(ctx, normal_item, math_content, text_content, strict)
    }));

    (math.boxed(), text.boxed())
}

fn mode_content_parsers_with_source<'a>(
    ctx: &'a ParseContext,
    strict: bool,
    src: &'a str,
) -> (ContentParser<'a>, ContentParser<'a>) {
    let mut math = Recursive::declare();
    let mut text = Recursive::declare();

    let math_for_math = math.clone();
    let text_for_math = text.clone();
    math.define(recursive(move |group_content| {
        let ws = insignificant_whitespace();
        let math_content = math_for_math.clone().boxed();
        let text_content = text_for_math.clone().boxed();
        let base_item = math_item_node_parser(
            ctx,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        )
        .padded_by(ws.clone());
        let normal_item = if strict {
            base_item.boxed()
        } else {
            recoverable_content_item_parser(
                ctx,
                ContentMode::Math,
                src,
                base_item,
                is_math_hard_stop,
            )
            .boxed()
        };
        math_group_content_parser(ctx, normal_item, math_content, text_content, strict)
            .padded_by(ws)
    }));

    let math_for_text = math.clone();
    let text_for_text = text.clone();
    text.define(recursive(move |group_content| {
        let math_content = math_for_text.clone().boxed();
        let text_content = text_for_text.clone().boxed();
        let base_item = text_atom_parser(
            ctx,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        );
        let normal_item = if strict {
            base_item.boxed()
        } else {
            recoverable_content_item_parser(
                ctx,
                ContentMode::Text,
                src,
                base_item,
                is_text_hard_stop,
            )
            .boxed()
        };
        text_group_content_parser(ctx, normal_item, math_content, text_content, strict)
    }));

    (math.boxed(), text.boxed())
}

/// Construct top-level math/text group parsers from content parsers.
fn mode_group_parsers<'a>(ctx: &'a ParseContext, strict: bool) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers(ctx, strict);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

fn mode_group_parsers_with_source<'a>(
    ctx: &'a ParseContext,
    strict: bool,
    src: &'a str,
) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers_with_source(ctx, strict, src);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

/// Build the top-level math-mode parser.
///
/// Returns a boxed parser that produces an implicit math-mode group
/// wrapping all parsed items. This is the parser used by
/// [`ParseContext::parse`](crate::parse::ParseContext::parse).
pub(crate) fn math_block_parser<'a>(ctx: &'a ParseContext, strict: bool) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers(ctx, strict);
    math_parser
}

pub(crate) fn math_block_parser_with_source<'a>(
    ctx: &'a ParseContext,
    strict: bool,
    src: &'a str,
) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers_with_source(ctx, strict, src);
    math_parser
}

/// Entry point parser for text mode.
#[allow(dead_code)] // Text entry point is unused; expose when direct text parsing is needed
fn text_block_parser<'a>(ctx: &'a ParseContext, strict: bool) -> NodeParser<'a> {
    let (_, text_parser) = mode_group_parsers(ctx, strict);
    text_parser
}

#[allow(dead_code)] // Source-aware text entry point is reserved for future direct text parsing
fn text_block_parser_with_source<'a>(
    ctx: &'a ParseContext,
    strict: bool,
    src: &'a str,
) -> NodeParser<'a> {
    let (_, text_parser) = mode_group_parsers_with_source(ctx, strict, src);
    text_parser
}
