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

use crate::context::ParseContext;
use crate::knowledge::{CommandKind, CommandMeta, EnvMeta, KnowledgeBase};
use crate::lexer::Token;
use texform_interface::syntax_node::{ArgumentSlot, ContentMode, Delimiter, GroupKind, SyntaxNode};

use self::arguments::{arguments_parser, fold_items};

/// A value paired with its source byte span.
pub(crate) type Spanned<T> = (T, SimpleSpan);

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

/// Boxed parser producing a list of child nodes (group content).
type ContentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>>;
/// Boxed parser producing a single syntax node.
type NodeParser<'a> = Boxed<'a, 'a, TokenStream<'a>, SyntaxNode, ParserError<'a>>;
/// Boxed parser producing an optional argument slot.
type ArgumentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, ArgumentSlot, ParserError<'a>>;
/// Return type of infix/declarative tail parsers: (command head, right/scope items).
type TailParseOutput = ((String, Vec<ArgumentSlot>), Vec<SyntaxNode>);

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
    kb: &'a KnowledgeBase,
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
        Token::ControlSeq(name) if kb.lookup_delimiter_control(name.as_str()).is_some() => {
            Delimiter::Control(kb.lookup_delimiter_control(name.as_str()).unwrap())
        }
    }
    .labelled("delimiter")
}

/// Parse escaped symbol control sequences into raw `Char` nodes.
fn escaped_symbol<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if matches!(name.as_str(), "%" | "$" | "&" | "#" | "_" | "{" | "}") => {
            SyntaxNode::Char(name.chars().next().unwrap())
        }
    }
    .labelled("escaped symbol")
}

/// Parse the active character `~` into `ActiveSpace`.
fn active_char<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    just(Token::ActiveChar).to(SyntaxNode::ActiveSpace)
}

/// Parse plain math characters.
fn math_char<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => SyntaxNode::Char(c),
        Token::Star => SyntaxNode::Char('*'),
        Token::Alignment => SyntaxNode::Char('&'),
        Token::LBracket => SyntaxNode::Char('['),
        Token::RBracket => SyntaxNode::Char(']'),
    }
    .labelled("math character")
}

/// Parse and coalesce consecutive text characters/whitespace into a single `Text` node.
fn text_chunk<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    content.map(move |children| SyntaxNode::Group {
        mode,
        kind: GroupKind::Implicit,
        children,
    })
}

/// Parse an explicit `{...}` group with the given content parser.
fn braced_group_parser<'a, P>(
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    just(Token::LBrace)
        .ignore_then(content)
        .then_ignore(just(Token::RBrace))
        .map(move |children| SyntaxNode::Group {
            mode,
            kind: GroupKind::Explicit,
            children,
        })
}

/// Parse `\left ... \right` delimited math group.
///
/// Whitespace between `\left`/`\right` and the delimiter is accepted so that
/// `\left ( ... \right )` round-trips correctly through the canonical
/// serializer's `Spaced` command spacing.
fn delimited_group_parser<'a, P>(
    kb: &'a KnowledgeBase,
    math_content: P,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();

    custom(move |input| {
        let group_start = input.cursor();
        input.parse(control_seq("left"))?;
        let _ = input.parse(ws.clone());

        let left_start = input.cursor();
        let left = match input.parse(delimiter(kb)) {
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
        let right = match input.parse(delimiter(kb)) {
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

        Ok(SyntaxNode::Group {
            mode: ContentMode::Math,
            kind: GroupKind::Delimited { left, right },
            children,
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
    Prime(SyntaxNode),
    /// Superscript was set by an explicit `^` token
    Explicit(SyntaxNode),
    /// Prime(s) merged with an explicit superscript
    Mixed(SyntaxNode),
}

/// Intermediate result of script parsing before folding into `SyntaxNode::Scripted`.
#[derive(Debug)]
struct ScriptComponents {
    base: SyntaxNode,
    subscript: Option<SyntaxNode>,
    superscript: Option<SyntaxNode>,
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
    P: Parser<'src, TokenStream<'src>, SyntaxNode, ParserError<'src>> + Clone + 'src,
{
    let ws = insignificant_whitespace();

    let preserve_atom_error = matches!(
        input.peek(),
        Some(Token::ControlSeq(name)) if matches!(name.as_str(), "left" | "begin")
    );
    let base_opt = if preserve_atom_error {
        Some(input.parse(atom_for_scripts.clone())?)
    } else {
        input.parse(atom_for_scripts.clone().or_not())?
    };
    let base = match base_opt {
        Some(base) => base,
        None => match input.peek() {
            Some(Token::Superscript) | Some(Token::Subscript) | Some(Token::Prime(_)) => {
                SyntaxNode::Group {
                    mode: ContentMode::Math,
                    kind: GroupKind::Implicit,
                    children: vec![],
                }
            }
            _ => {
                let cursor = input.cursor();
                return Err(input.err_peek_or_point(&cursor, "expected atom or script marker"));
            }
        },
    };

    let mut sup_state: Option<SupState> = None;
    let mut subscript: Option<SyntaxNode> = None;

    loop {
        let checkpoint = input.save();
        let _ = input.parse(ws.clone());

        let marker_start = input.cursor();

        let marker = match input.peek() {
            Some(Token::Superscript) => {
                input.next();
                let node = input.parse(atom_for_scripts.clone())?;
                Some(("sup", node))
            }
            Some(Token::Subscript) => {
                input.next();
                let node = input.parse(atom_for_scripts.clone())?;
                Some(("sub", node))
            }
            Some(Token::Prime(_)) => {
                let count = match input.next() {
                    Some(Token::Prime(n)) => n,
                    _ => unreachable!("peek ensured prime token"),
                };
                let node = if count == 1 {
                    SyntaxNode::Char('\'')
                } else {
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: (0..count).map(|_| SyntaxNode::Char('\'')).collect(),
                    }
                };
                Some(("prime", node))
            }
            _ => None,
        };

        let Some((kind, node)) = marker else {
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
                subscript = Some(node);
            }
            "sup" => {
                let current = sup_state.take();
                sup_state = match current {
                    None => Some(SupState::Explicit(node)),
                    Some(SupState::Prime(existing)) => Some(SupState::Mixed(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![existing, node],
                    })),
                    Some(SupState::Explicit(_)) | Some(SupState::Mixed(_)) => {
                        return Err(input
                            .err_since(&marker_start, "Double exponent: use braces to clarify"));
                    }
                };
            }
            "prime" => {
                let current = sup_state.take();
                sup_state = match current {
                    None => Some(SupState::Prime(node)),
                    Some(SupState::Prime(existing)) => Some(SupState::Mixed(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![existing, node],
                    })),
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
        SupState::Prime(node) | SupState::Explicit(node) | SupState::Mixed(node) => node,
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
/// (result + diagnostics), use [`ParseContext::parse`](crate::context::ParseContext::parse)
/// instead.
pub fn parse(src: &str, strict: bool) -> Result<Spanned<SyntaxNode>, Vec<Rich<'_, Token>>> {
    let token_stream = build_token_stream(src);
    math_block_parser(ParseContext::all_packages_shared().kb(), strict)
        .map_with(|node, e| (node, e.span()))
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let body = implicit_group_parser(mode, content);
    custom(move |input| {
        let body_start = input.cursor();
        match input.parse(body.clone()) {
            Ok(node) => Ok(node),
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
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> NodeParser<'a> {
    match mode {
        ContentMode::Math => math_item_parser(kb, math_content, text_content, strict).boxed(),
        ContentMode::Text => text_item_parser(kb, math_content, text_content, strict).boxed(),
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

fn normalize_recovery_message(src: &str, message: String) -> String {
    let is_generic_parse_error = matches!(
        message.as_str(),
        "found '}' expected something else" | "found '}' expected something else, or end of input"
    );
    if !is_generic_parse_error {
        return message;
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
    src: &'a str,
    item: P,
    is_hard_stop: fn(&Token) -> bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    custom(move |input| {
        let ws = insignificant_whitespace();
        let metadata_checkpoint = input.save();
        let _ = input.parse(ws.clone());
        let item_start = input.cursor();
        let item_starts_with_left =
            matches!(input.peek().as_ref(), Some(Token::ControlSeq(name)) if name == "left");
        let item_start_index = input.span_from_cursor(&item_start).start;
        let opening_environment = peek_environment_name_at_cursor(input, "begin");
        let outer_environment_stack = opening_environment
            .as_ref()
            .map(|_| scan_environment_stack_before(src, item_start_index))
            .unwrap_or_default();
        input.rewind(metadata_checkpoint);

        let checkpoint = input.save();
        let err = match input.parse(item.clone()) {
            Ok(node) => return Ok(node),
            Err(err) => err,
        };
        let direct_left_group_error = item_starts_with_left
            && matches!(err.reason(), chumsky::error::RichReason::Custom(message) if is_direct_left_group_error(message));
        if direct_left_group_error {
            return Err(err);
        }
        let failure_environment_stack = scan_environment_stack_before(src, err.span().end);
        input.rewind(checkpoint);

        if is_hard_stop_after_whitespace(input, is_hard_stop) {
            return Err(err);
        }

        let message = match err.reason() {
            chumsky::error::RichReason::ExpectedFound { .. } => format!("{err}"),
            chumsky::error::RichReason::Custom(message) => message.clone(),
        };
        let message = normalize_recovery_message(src, message);

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
                Ok(SyntaxNode::Error {
                    message: message.clone(),
                    snippet: slice_snippet(src, span),
                })
            }
        });

        input.parse(item.clone().recover_with(via_parser(recovery_parser)))
    })
}

/// Consume a control-sequence token, look it up in the KB, and validate that
/// it matches the `expected_kind` and is allowed in `current_mode`.
fn command_head_parser<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    kb: &'parse KnowledgeBase,
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

    let meta = match kb.lookup_command(&name) {
        Some(meta) if meta.kind == expected_kind => meta,
        Some(_) => {
            return Err(Rich::custom(
                cmd_span,
                format!("not {}", expected_kind.label()),
            ));
        }
        None => {
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

    if !meta.allowed_mode.allows(current_mode) {
        return Err(Rich::custom(
            cmd_span,
            format!("Command \\{} is not allowed in {} mode", name, current_mode),
        ));
    }

    Ok((name, meta))
}

// ============================================================================
// Content and Argument Parsers
// ============================================================================

/// Lookahead guard that succeeds when the next token is an infix or
/// declarative command. Used as a stop condition for leading-item collection.
fn math_infix_or_decl_guard<'a>(
    kb: &'a KnowledgeBase,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if kb.lookup_command(name.as_str())
                .map(|m| matches!(m.kind, CommandKind::Infix | CommandKind::Declarative))
                .unwrap_or(false) => ()
    }
    .rewind()
}

/// Guard used to stop content parsing before declarative commands.
fn declarative_guard<'a>(
    kb: &'a KnowledgeBase,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if kb.lookup_command(name.as_str())
                .map(|m| m.kind == CommandKind::Declarative)
                .unwrap_or(false) => ()
    }
    .rewind()
}

fn infix_guard<'a>(
    kb: &'a KnowledgeBase,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if kb.lookup_command(name.as_str())
                .map(|m| m.kind == CommandKind::Infix)
                .unwrap_or(false) => ()
    }
    .rewind()
}

/// Parse one math item node (with script handling) without outer spacing policy.
///
/// Callers decide whether to wrap it with padding or stop-guards.
fn math_item_node_parser<'a, P>(
    kb: &'a KnowledgeBase,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let atom = math_atom_parser(kb, group_content, math_content, text_content, strict);
    scripted_atom_parser(atom)
}

/// Parse a single math item in argument contexts.
///
/// This parser does not consume trailing whitespace so the following argument
/// slot can still enforce `no_leading_space`.
fn math_item_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let item = math_item_node_parser(kb, math_content.clone(), math_content, text_content, strict);

    math_infix_or_decl_guard(kb)
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .ignore_then(item)
}

/// Parse a single text item (respecting stop guards).
fn text_item_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let normal_item =
        text_atom_parser(kb, text_content.clone(), math_content, text_content, strict);

    declarative_guard(kb)
        .or(control_seq("end"))
        .not()
        .ignore_then(normal_item)
}

// ============================================================================
// Command and Environment Parsers
// ============================================================================

fn prefix_command_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let (name, meta) =
            match command_head_parser(input, kb, CommandKind::Prefix, current_mode, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.argspec.args,
            strict,
            "command argument",
        ))?;

        Ok(SyntaxNode::Command { name, args })
    })
}

fn unknown_command_parser<'a>(
    kb: &'a KnowledgeBase,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if kb.lookup_command(name.as_str()).is_none() => name
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
            Ok(SyntaxNode::UnknownCommand { name })
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

/// Parse `\begin{name}` plus its arguments, returning metadata.
///
/// Whitespace between `\begin` and `{name}` is accepted so that
/// `\begin {matrix}` round-trips through the serializer's `Spaced`
/// environment name spacing.
fn parse_env_header<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, (String, Vec<ArgumentSlot>, &'a EnvMeta), ParserError<'a>> + Clone
{
    custom(move |input| {
        let ws = insignificant_whitespace();

        input.parse(control_seq("begin"))?;
        let _ = input.parse(ws.clone());

        let name_start = input.cursor();
        let name = input.parse(env_name_parser())?;
        let name_span = input.span_from_cursor(&name_start);

        let meta = match kb.lookup_env(name.as_str()) {
            Some(m) => m,
            None => {
                return Err(Rich::custom(
                    name_span,
                    format!("Unknown environment: {}", name),
                ));
            }
        };

        if !meta.allowed_mode.allows(current_mode) {
            return Err(Rich::custom(
                name_span,
                format!(
                    "Environment {} is not allowed in {} mode",
                    name, current_mode
                ),
            ));
        }

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.argspec.args,
            strict,
            "environment argument",
        ))?;

        Ok((name, args, meta))
    })
}

/// Parse a full environment including body and closing tag.
fn environment_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let ws = insignificant_whitespace();

        let (name, args, meta) = input.parse(parse_env_header(
            kb,
            math_content.clone(),
            text_content.clone(),
            current_mode,
            strict,
        ))?;

        let body_content = match meta.body_mode {
            ContentMode::Math => math_content.clone(),
            ContentMode::Text => text_content.clone(),
        };
        let body_recovery_start = input.save();
        let body = input.parse(env_body_parser(meta.body_mode, body_content))?;

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
                meta.body_mode,
                kb,
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

        Ok(SyntaxNode::Environment {
            name,
            args,
            body: Box::new(body),
        })
    })
}

// ============================================================================
// Mode Parsers (Math and Text)
// ============================================================================

/// Parse a math atom (group/command/env/char) without scripts.
fn math_atom_parser<'a, P>(
    kb: &'a KnowledgeBase,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let explicit_group = braced_group_parser(ContentMode::Math, group_content.clone());
    let delimited_group = delimited_group_parser(kb, math_content.clone());
    let environment = environment_parser(
        kb,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Math,
        strict,
    );
    let prefix_command =
        prefix_command_parser(kb, math_content, text_content, ContentMode::Math, strict);
    let unknown_command = unknown_command_parser(kb, strict);
    let fallback = choice((
        explicit_group,
        environment.clone(),
        escaped_symbol(),
        prefix_command,
        unknown_command,
        active_char(),
        math_char(),
    ));

    custom(move |input| match input.peek() {
        Some(Token::ControlSeq(name)) if name == "left" => input.parse(delimited_group.clone()),
        Some(Token::ControlSeq(name)) if name == "begin" => input.parse(environment.clone()),
        _ => input.parse(fallback.clone()),
    })
}

/// Wrap a base atom with script parsing (`^`, `_`, primes).
///
/// This parser allows leading whitespace before script atoms but does not
/// consume trailing whitespace after the parsed item.
fn scripted_atom_parser<'a, P>(
    atom: P,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();
    let atom_for_scripts = ws.ignore_then(atom.clone());
    custom(move |input| {
        let components = parse_scripted_components(input, atom_for_scripts.clone())?;

        if components.subscript.is_none() && components.superscript.is_none() {
            return Ok(components.base);
        }

        Ok(SyntaxNode::Scripted {
            base: Box::new(components.base),
            subscript: components.subscript.map(Box::new),
            superscript: components.superscript.map(Box::new),
        })
    })
}

/// Parse a text atom (text chunk, inline math, group, command, env).
fn text_atom_parser<'a, P>(
    kb: &'a KnowledgeBase,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let inline_math = just(Token::MathShift)
        .ignore_then(implicit_group_parser(
            ContentMode::Math,
            math_content.clone(),
        ))
        .then_ignore(just(Token::MathShift))
        .map(|node| match node {
            SyntaxNode::Group { mode, children, .. } => SyntaxNode::Group {
                mode,
                kind: GroupKind::InlineMath,
                children,
            },
            other => other,
        });

    let explicit_group = braced_group_parser(ContentMode::Text, group_content);
    let environment = environment_parser(
        kb,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Text,
        strict,
    );
    let prefix_command =
        prefix_command_parser(kb, math_content, text_content, ContentMode::Text, strict);
    let unknown_command = unknown_command_parser(kb, strict);

    choice((
        text_chunk(),
        inline_math,
        explicit_group,
        environment,
        escaped_symbol(),
        prefix_command,
        unknown_command,
        active_char(),
    ))
}

/// Parse the tail after an infix command: the command head plus right operand items.
fn infix_tail_parser<'a, P>(
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let infix_cmd = custom(move |input| {
        let (name, meta) =
            match command_head_parser(input, kb, CommandKind::Infix, ContentMode::Math, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.argspec.args,
            strict,
            "infix command argument",
        ))?;

        Ok((name, args))
    });

    let stop_declarative = declarative_guard(kb);

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
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let decl_cmd = custom(move |input| {
        let (name, meta) =
            match command_head_parser(input, kb, CommandKind::Declarative, current_mode, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.argspec.args,
            strict,
            "declarative command argument",
        ))?;

        Ok((name, args))
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
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();
    let stop_infix_or_decl = math_infix_or_decl_guard(kb);
    let stop_boundary = ws
        .clone()
        .ignore_then(stop_infix_or_decl)
        .or(ws.clone().ignore_then(control_seq("right")))
        .or(ws.clone().ignore_then(control_seq("end")))
        .rewind();
    let guarded_item = stop_boundary.not().ignore_then(normal_item.clone());
    let leading = custom(move |input| {
        let mut items = Vec::new();

        loop {
            let first_item_metadata = input.save();
            let _ = input.parse(ws.clone());
            let preserve_first_item_error = matches!(
                input.peek(),
                Some(Token::ControlSeq(name)) if name == "left"
            );
            input.rewind(first_item_metadata);
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
                    break;
                }
            }
        }

        Ok(items)
    });

    let infix_tail = infix_guard(kb).ignore_then(infix_tail_parser(
        kb,
        normal_item.clone(),
        math_content.clone(),
        text_content.clone(),
        strict,
    ));

    let declarative_tail = declarative_guard(kb).ignore_then(declarative_tail_parser(
        kb,
        normal_item,
        math_content,
        text_content,
        ContentMode::Math,
        strict,
    ));

    leading
        .then(infix_tail.or_not())
        .then(declarative_tail.or_not())
        .try_map(|((leading, infix_tail), declarative_tail), span| {
            if let Some((infix_info, right_items)) = infix_tail {
                if leading.is_empty() {
                    return Err(Rich::custom(
                        span,
                        "Infix command requires non-empty left operand",
                    ));
                }

                let (name, args) = infix_info;
                let left = fold_items(ContentMode::Math, leading);
                let right = fold_items(ContentMode::Math, right_items);

                let infix_node = SyntaxNode::Infix {
                    name,
                    args,
                    left: Box::new(left),
                    right: Box::new(right),
                };

                let mut nodes = vec![infix_node];
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_args) = decl_info;
                    let scope = fold_items(ContentMode::Math, scope_items);
                    nodes.push(SyntaxNode::Declarative {
                        name: decl_name,
                        args: decl_args,
                        scope: Box::new(scope),
                    });
                }
                Ok(nodes)
            } else {
                let mut items = leading;
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_args) = decl_info;
                    let scope = fold_items(ContentMode::Math, scope_items);
                    items.push(SyntaxNode::Declarative {
                        name: decl_name,
                        args: decl_args,
                        scope: Box::new(scope),
                    });
                }
                Ok(items)
            }
        })
}

/// Build text-mode group content (leading items + optional declarative tail).
fn text_group_content_parser<'a, P>(
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let stop_declarative = declarative_guard(kb);
    let ws = insignificant_whitespace();

    let stop_boundary = ws
        .clone()
        .ignore_then(stop_declarative)
        .or(ws.ignore_then(control_seq("end")))
        .rewind();

    let guarded_item = stop_boundary.not().ignore_then(normal_item.clone());
    let leading = guarded_item.repeated().collect::<Vec<_>>();

    let declarative_tail = declarative_guard(kb).ignore_then(declarative_tail_parser(
        kb,
        normal_item,
        math_content,
        text_content,
        ContentMode::Text,
        strict,
    ));

    leading
        .then(declarative_tail.or_not())
        .map(|(mut leading, declarative_tail)| {
            if let Some((decl_info, scope_items)) = declarative_tail {
                let (decl_name, decl_args) = decl_info;
                let scope = fold_items(ContentMode::Text, scope_items);
                leading.push(SyntaxNode::Declarative {
                    name: decl_name,
                    args: decl_args,
                    scope: Box::new(scope),
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
    kb: &'a KnowledgeBase,
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
            kb,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        )
        .padded_by(ws.clone());
        math_group_content_parser(kb, normal_item, math_content, text_content, strict).padded_by(ws)
    }));

    let math_for_text = math.clone();
    let text_for_text = text.clone();
    text.define(recursive(move |group_content| {
        let math_content = math_for_text.clone().boxed();
        let text_content = text_for_text.clone().boxed();
        let normal_item = text_atom_parser(
            kb,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        );
        text_group_content_parser(kb, normal_item, math_content, text_content, strict)
    }));

    (math.boxed(), text.boxed())
}

fn mode_content_parsers_with_source<'a>(
    kb: &'a KnowledgeBase,
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
            kb,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        )
        .padded_by(ws.clone());
        let normal_item = if strict {
            base_item.boxed()
        } else {
            recoverable_content_item_parser(src, base_item, is_math_hard_stop).boxed()
        };
        math_group_content_parser(kb, normal_item, math_content, text_content, strict).padded_by(ws)
    }));

    let math_for_text = math.clone();
    let text_for_text = text.clone();
    text.define(recursive(move |group_content| {
        let math_content = math_for_text.clone().boxed();
        let text_content = text_for_text.clone().boxed();
        let base_item = text_atom_parser(
            kb,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        );
        let normal_item = if strict {
            base_item.boxed()
        } else {
            recoverable_content_item_parser(src, base_item, is_text_hard_stop).boxed()
        };
        text_group_content_parser(kb, normal_item, math_content, text_content, strict)
    }));

    (math.boxed(), text.boxed())
}

/// Construct top-level math/text group parsers from content parsers.
fn mode_group_parsers<'a>(kb: &'a KnowledgeBase, strict: bool) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers(kb, strict);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

fn mode_group_parsers_with_source<'a>(
    kb: &'a KnowledgeBase,
    strict: bool,
    src: &'a str,
) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers_with_source(kb, strict, src);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

/// Build the top-level math-mode parser.
///
/// Returns a boxed parser that produces an implicit math-mode group
/// wrapping all parsed items. This is the parser used by
/// [`ParseContext::parse`](crate::context::ParseContext::parse).
pub(crate) fn math_block_parser<'a>(kb: &'a KnowledgeBase, strict: bool) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers(kb, strict);
    math_parser
}

pub(crate) fn math_block_parser_with_source<'a>(
    kb: &'a KnowledgeBase,
    strict: bool,
    src: &'a str,
) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers_with_source(kb, strict, src);
    math_parser
}

/// Entry point parser for text mode.
#[allow(dead_code)] // Text entry point is unused; expose when direct text parsing is needed
fn text_block_parser<'a>(kb: &'a KnowledgeBase, strict: bool) -> NodeParser<'a> {
    let (_, text_parser) = mode_group_parsers(kb, strict);
    text_parser
}

#[allow(dead_code)] // Source-aware text entry point is reserved for future direct text parsing
fn text_block_parser_with_source<'a>(
    kb: &'a KnowledgeBase,
    strict: bool,
    src: &'a str,
) -> NodeParser<'a> {
    let (_, text_parser) = mode_group_parsers_with_source(kb, strict, src);
    text_parser
}
