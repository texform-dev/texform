//! Parser utilities and base parsers.
//!
//! This module hosts:
//! - Type aliases and extension traits for parser input handling
//! - Base parsers: delimiters, escapes, characters, whitespace, control sequences
//! - Group builders: implicit/braced/bracket groups
//! - State machines for script parsing and token-level argument parsing
//! - Helper functions for AST normalization

use chumsky::{
    input::{Cursor, InputRef, Stream},
    prelude::*,
};
use logos::Logos;

use crate::column_parser::parse_column_template;
use crate::knowledge::KnowledgeBase;
use crate::lexer::Token;
use texform_interface::syntax_node::{ContentMode, Delimiter, GroupKind, SyntaxNode};

// ============================================================================
// Type Aliases
// ============================================================================

/// A value paired with its source byte span.
pub type Spanned<T> = (T, SimpleSpan);

/// The concrete mapped input type produced by `build_token_stream`.
/// `Stream<...>.map(eoi, |(tok, span)| (tok, span))` yields this.
pub type TokenStream<'a> = chumsky::input::MappedInput<
    Token,
    SimpleSpan,
    Stream<std::vec::IntoIter<(Token, SimpleSpan)>>,
    fn((Token, SimpleSpan)) -> (Token, SimpleSpan),
>;

pub type ParserError<'a> = extra::Err<Rich<'a, Token>>;
pub type ParserInput<'src, 'parse> = InputRef<'src, 'parse, TokenStream<'src>, ParserError<'src>>;

/// Build a chumsky token stream from a source string.
///
/// Performs lexing via logos and wraps the result in a `Stream` + `MappedInput`
/// so that chumsky automatically propagates byte-level spans.
pub fn build_token_stream(src: &str) -> TokenStream<'_> {
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

// ============================================================================
// Parser Input Extensions
// ============================================================================

/// Ergonomic helpers for building spans and custom errors in imperative parsers.
pub trait ParserInputExt<'src, 'parse> {
    fn span_from_cursor(&mut self, start: &Cursor<'src, 'parse, TokenStream<'src>>) -> SimpleSpan;

    fn err_since(
        &mut self,
        start: &Cursor<'src, 'parse, TokenStream<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token>;

    /// Build an error for the next token without consuming it. Falls back to a
    /// point span at EOF.
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
        // Use span_since to get a byte-level span from the cursor position.
        // If there's a next token, the span covers it; otherwise it's zero-width.
        let span = self.span_since(start);
        Rich::custom(span, msg)
    }
}

// ============================================================================
// Base Parsers
// ============================================================================

/// Consume insignificant whitespace tokens and produce no output.
pub fn insignificant_whitespace<'a>()
-> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! { Token::Whitespaces => () }.repeated().ignored()
}

/// Wrap a parser to accept either `{...}` or inline input.
pub fn maybe_braced<'a, T, P>(
    inner: P,
) -> impl Parser<'a, TokenStream<'a>, T, ParserError<'a>> + Clone
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
pub fn maybe_braced_or_empty<'a, T, P>(
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
pub fn optional_bracketed<'a, T, P>(
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
pub fn optional_bracketed_or_empty<'a, T, P>(
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
///
/// Supports:
/// - '.' => Delimiter::None
/// - '(', ')', '[', ']', '|' etc => Delimiter::Char
/// - \langle, \rangle etc => Delimiter::Control
pub fn delimiter<'a>(
    kb: &'a KnowledgeBase,
) -> impl Parser<'a, TokenStream<'a>, Delimiter, ParserError<'a>> + Clone {
    select! {
        Token::Char('.') => Delimiter::None,
        Token::Char(c) if matches!(c, '(' | ')' | '[' | ']' | '|' | '<' | '>' | '/' | '\\')
            => Delimiter::Char(c),
        Token::ControlSeq(name) if kb.lookup_delimiter_control(name.as_str()).is_some() => {
            Delimiter::Control(kb.lookup_delimiter_control(name.as_str()).unwrap())
        }
    }
    .labelled("delimiter")
}

/// Parse escaped symbol control sequences into raw `Char` nodes.
///
pub fn escaped_symbol<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
{
    select! {
        Token::ControlSeq(name) if matches!(name.as_str(), "%" | "$" | "&" | "#" | "_" | "{" | "}") => {
            SyntaxNode::Char(name.chars().next().unwrap())
        }
    }
    .labelled("escaped symbol")
}

/// Parse the active character `~` into `ActiveSpace`.
pub fn active_char<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    just(Token::ActiveChar).to(SyntaxNode::ActiveSpace)
}

/// Parse plain math characters (including `*` and `&` tokens).
pub fn math_char<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
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
pub fn text_chunk<'a>() -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
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
pub fn control_seq<'a>(
    target: &'static str,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if name == target => (),
    }
    .labelled(target)
}

// ============================================================================
// Group Builders
// ============================================================================

/// Build an implicit group from a content parser.
pub fn implicit_group_parser<'a, P>(
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
pub fn braced_group_parser<'a, P>(
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
pub fn delimited_group_parser<'a, P>(
    kb: &'a KnowledgeBase,
    math_content: P,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    control_seq("left")
        .ignore_then(delimiter(kb))
        .then(math_content)
        .then_ignore(control_seq("right"))
        .then(delimiter(kb))
        .map(|((left, children), right)| SyntaxNode::Group {
            mode: ContentMode::Math,
            kind: GroupKind::Delimited { left, right },
            children,
        })
}

// ============================================================================
// Script Parsing
// ============================================================================

/// Superscript state while scanning a script sequence.
///
/// We distinguish three cases so we can implement MathJax-compatible
/// error semantics:
/// - `Prime`: only prime markers contributed to the superscript so far
/// - `Explicit`: a normal superscript atom (no primes)
/// - `Mixed`: combination of primes and normal superscripts
#[derive(Clone)]
pub enum SupState {
    Prime(SyntaxNode),
    Explicit(SyntaxNode),
    Mixed(SyntaxNode),
}

/// Result of parsing scripted atom components.
///
/// This is the minimal data needed to build a `SyntaxNode::Scripted`:
/// we keep the base node and optional subscript/superscript nodes.
pub struct ScriptComponents {
    pub base: SyntaxNode,
    pub subscript: Option<SyntaxNode>,
    pub superscript: Option<SyntaxNode>,
}

/// Parse a scripted atom sequence starting from an optional base.
///
/// Responsibilities:
/// - Accept optional base atom, or synthesize an empty base when the
///   first token is a script marker (`^`, `_`, or prime)
/// - Scan an arbitrary-length sequence of script markers and their
///   arguments, folding primes and explicit superscripts into a single
///   superscript state
/// - Enforce MathJax-like error conditions:
///   - "Double exponent: use braces to clarify"
///   - "Double subscripts: use braces to clarify"
///   - "Prime causes double exponent: use braces to clarify"
///
/// On success this returns a `ScriptComponents` that the caller can wrap
/// into a `SyntaxNode::Scripted`.
pub fn parse_scripted_components<'src, 'parse, P>(
    input: &mut ParserInput<'src, 'parse>,
    atom_for_scripts: P,
) -> Result<ScriptComponents, Rich<'src, Token>>
where
    P: Parser<'src, TokenStream<'src>, SyntaxNode, ParserError<'src>> + Clone + 'src,
{
    let ws = insignificant_whitespace();

    // Optional base: either an atom or an empty implicit group when
    // the sequence starts with a script marker.
    let base_opt = input.parse(atom_for_scripts.clone().or_not())?;
    let base = match base_opt {
        Some(b) => b,
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

    let superscript = sup_state.map(|s| match s {
        SupState::Prime(n) | SupState::Explicit(n) | SupState::Mixed(n) => n,
    });

    Ok(ScriptComponents {
        base,
        subscript,
        superscript,
    })
}

// ============================================================================
// Token-Level Argument Parsers
// ============================================================================

/// Parse an optional bracketed argument and return its raw tokens.
pub(crate) fn collect_optional_bracketed_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    match_brackets: bool,
) -> Result<Option<Vec<Token>>, Rich<'src, Token>> {
    if !matches!(input.peek(), Some(Token::LBracket)) {
        return Ok(None);
    }

    let start = input.cursor();
    input.next();

    let mut tokens = Vec::new();
    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;

    // Track nested braces; optionally track top-level brackets.
    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed bracket argument")),
        };

        match token {
            Token::LBracket => {
                if match_brackets && brace_depth == 0 {
                    bracket_depth += 1;
                }
                tokens.push(Token::LBracket);
            }
            Token::RBracket => {
                if brace_depth == 0 {
                    if match_brackets && bracket_depth > 0 {
                        bracket_depth -= 1;
                        tokens.push(Token::RBracket);
                        continue;
                    }
                    break;
                }
                tokens.push(Token::RBracket);
            }
            Token::LBrace => {
                brace_depth += 1;
                tokens.push(Token::LBrace);
            }
            Token::RBrace => {
                if brace_depth == 0 {
                    return Err(input.err_since(&start, "unbalanced brace in bracket argument"));
                }
                brace_depth -= 1;
                tokens.push(Token::RBrace);
            }
            other => tokens.push(other),
        }
    }

    Ok(Some(tokens))
}

/// Parse a brace-delimited token list, optionally allowing nested braces.
pub(crate) fn collect_braced_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    allow_nested: bool,
) -> Result<Vec<Token>, Rich<'src, Token>> {
    let start = input.cursor();
    match input.next() {
        Some(Token::LBrace) => {}
        _ => return Err(input.err_since(&start, "expected '{'")),
    }

    let mut tokens = Vec::new();
    let mut depth = 0usize;

    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed brace argument")),
        };

        match token {
            Token::LBrace => {
                if !allow_nested {
                    return Err(input.err_since(&start, "nested braces not allowed"));
                }
                depth += 1;
                tokens.push(Token::LBrace);
            }
            Token::RBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(Token::RBrace);
            }
            other => tokens.push(other),
        }
    }

    Ok(tokens)
}

/// Convert tokens into a raw string representation.
pub(crate) fn tokens_to_string(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::ControlSeq(name) => {
                out.push('\\');
                out.push_str(name);
            }
            Token::Char(c) => out.push(*c),
            Token::Star => out.push('*'),
            Token::Alignment => out.push('&'),
            Token::MathShift => out.push('$'),
            Token::Parameter => out.push('#'),
            Token::Superscript => out.push('^'),
            Token::Subscript => out.push('_'),
            Token::Prime(count) => {
                for _ in 0..*count {
                    out.push('\'');
                }
            }
            Token::ActiveChar => out.push('~'),
            Token::LBracket => out.push('['),
            Token::RBracket => out.push(']'),
            Token::LBrace => out.push('{'),
            Token::RBrace => out.push('}'),
            Token::Whitespaces => out.push(' '),
            Token::Comment => {}
        }
    }
    out
}

// ============================================================================
// String Validation Helpers
// ============================================================================

/// Validate a key=value list at top level.
pub(crate) fn validate_keyval(raw: &str) -> Result<(), &'static str> {
    let mut key = String::new();
    let mut value = String::new();
    let mut in_value = false;
    let mut brace_depth = 0usize;

    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                let target = if in_value { &mut value } else { &mut key };
                target.push('\\');
                if let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphabetic() {
                        while let Some(c) = chars.peek().copied() {
                            if c.is_ascii_alphabetic() {
                                target.push(c);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    } else {
                        target.push(next);
                        chars.next();
                    }
                }
            }
            '{' => {
                brace_depth += 1;
                if in_value {
                    value.push('{');
                } else {
                    key.push('{');
                }
            }
            '}' => {
                if brace_depth == 0 {
                    return Err("unbalanced brace in keyval");
                }
                brace_depth -= 1;
                if in_value {
                    value.push('}');
                } else {
                    key.push('}');
                }
            }
            '=' if brace_depth == 0 && !in_value => {
                if key.trim().is_empty() {
                    return Err("keyval missing key");
                }
                in_value = true;
            }
            ',' if brace_depth == 0 && in_value => {
                if value.trim().is_empty() {
                    return Err("keyval missing value");
                }
                key.clear();
                value.clear();
                in_value = false;
            }
            ',' if brace_depth == 0 && !in_value => {
                return Err("keyval missing value");
            }
            '=' if brace_depth == 0 && in_value => {
                value.push('=');
            }
            other => {
                if in_value {
                    value.push(other);
                } else {
                    key.push(other);
                }
            }
        }
    }

    if brace_depth != 0 {
        return Err("unbalanced brace in keyval");
    }

    if !in_value {
        return Err("keyval missing value");
    }

    if value.trim().is_empty() {
        return Err("keyval missing value");
    }

    Ok(())
}

/// Normalize a valid keyval string for storage.
pub(crate) fn normalize_keyval_string(raw: &str) -> String {
    raw.trim().to_string()
}

// ============================================================================
// Inline Value Combinators
// ============================================================================

/// Pure combinator: `sign? digits+` → normalized integer string.
pub fn integer<'a>() -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    let sign = select! { Token::Char(c @ ('+' | '-')) => c }.or_not();
    let digit = select! { Token::Char(c) if c.is_ascii_digit() => c };

    sign.then(digit.repeated().at_least(1).collect::<Vec<char>>())
        .map(|(sign, digits)| {
            let mut out = String::with_capacity(digits.len() + 1);
            if let Some(s) = sign {
                out.push(s);
            }
            for d in digits {
                out.push(d);
            }
            out
        })
        .labelled("integer")
}

/// Pure combinator: `sign? (digits frac? | frac) ws? unit` → normalized dimension string.
pub fn dimension<'a>() -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    let sign = select! { Token::Char(c @ ('+' | '-')) => c }.or_not();
    let digit = select! { Token::Char(c) if c.is_ascii_digit() => c };
    let sep = select! { Token::Char(c @ ('.' | ',')) => c };
    let ws = insignificant_whitespace();
    let alpha = select! { Token::Char(c) if c.is_ascii_alphabetic() => c };
    let unit = alpha.repeated().at_least(1).collect::<Vec<char>>();

    let int_digits = digit.repeated().collect::<Vec<char>>();
    let frac = sep.then(digit.repeated().collect::<Vec<char>>());

    sign.then(int_digits)
        .then(frac.or_not())
        .then_ignore(ws)
        .then(unit)
        .try_map(|(((sign, int_digits), frac), unit_chars), span| {
            let has_int = !int_digits.is_empty();
            let has_frac = frac.as_ref().is_some_and(|(_, ds)| !ds.is_empty());
            if !has_int && !has_frac {
                return Err(Rich::custom(span, "invalid dimension"));
            }
            let unit: String = unit_chars.into_iter().collect();
            if !is_valid_dimension_unit(&unit) {
                return Err(Rich::custom(span, "unsupported dimension unit"));
            }
            let mut value = String::new();
            if let Some(s) = sign {
                value.push(s);
            }
            for d in &int_digits {
                value.push(*d);
            }
            if let Some((_, frac_digits)) = frac {
                value.push('.'); // normalize comma to dot
                for d in &frac_digits {
                    value.push(*d);
                }
            }
            Ok(format!("{}{}", value, unit))
        })
        .labelled("dimension")
}

/// Parse a keyval value argument (required or optional).
///
/// - Required: must be `{...}` form
/// - Optional: accepts `[...]` or returns empty string
pub fn keyval_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let raw = if required {
            let start = input.cursor();
            if !matches!(input.peek(), Some(Token::LBrace)) {
                return Err(input.err_since(&start, "expected keyval argument"));
            }
            let tokens = collect_braced_tokens(input, true)?;
            tokens_to_string(&tokens)
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            return Ok(String::new());
        };

        validate_keyval(&raw).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg)
        })?;

        Ok(normalize_keyval_string(&raw))
    })
}

/// Parse a column template argument (required or optional) into a normalized string.
///
/// - Required: must be `{...}` form
/// - Optional: accepts `[...]` or returns an empty string
///
/// The parser validates the template by attempting to parse it with ColumnParser,
/// but stores the trimmed raw string in syntax node arguments.
pub fn column_spec_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let raw = if required {
            let start = input.cursor();
            if !matches!(input.peek(), Some(Token::LBrace)) {
                return Err(input.err_since(&start, "expected column argument"));
            }
            let tokens = collect_braced_tokens(input, true)?;
            tokens_to_string(&tokens)
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            String::new()
        };
        let normalized = raw.trim().to_string();

        parse_column_template(&normalized).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg.to_string())
        })?;

        Ok(normalized)
    })
}

/// Return true if the unit is in the supported set.
fn is_valid_dimension_unit(unit: &str) -> bool {
    matches!(
        unit,
        "em" | "ex" | "pt" | "pc" | "px" | "in" | "cm" | "mm" | "mu"
    )
}

// ============================================================================
// AST Helpers
// ============================================================================

/// Normalize argument value by collapsing groups to a single node when possible.
///
/// This ensures `\frac{a}{b}` and `\frac ab` produce identical AST:
/// - Single-element explicit groups are unwrapped to the element itself
/// - Multi-element groups become implicit groups
/// - Empty groups remain as empty implicit groups
pub fn normalize_argument_value(mode: ContentMode, node: SyntaxNode) -> SyntaxNode {
    match node {
        SyntaxNode::Group { children, .. } => fold_items(mode, children),
        other => other,
    }
}

/// Fold a list of items into a single node.
pub fn fold_items(mode: ContentMode, items: Vec<SyntaxNode>) -> SyntaxNode {
    match items.len() {
        0 => SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: vec![],
        },
        1 => items.into_iter().next().unwrap(),
        _ => SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: items,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_integer(src: &str) -> Result<String, ()> {
        let stream = build_token_stream(src);
        integer().parse(stream).into_result().map_err(|_| ())
    }

    fn parse_dimension(src: &str) -> Result<String, ()> {
        let stream = build_token_stream(src);
        dimension().parse(stream).into_result().map_err(|_| ())
    }

    #[test]
    fn test_integer_combinator() {
        assert_eq!(parse_integer("123").unwrap(), "123");
        assert_eq!(parse_integer("+42").unwrap(), "+42");
        assert_eq!(parse_integer("-0").unwrap(), "-0");
        assert!(parse_integer("abc").is_err());
        assert!(parse_integer("+").is_err());
    }

    #[test]
    fn test_dimension_combinator() {
        assert_eq!(parse_dimension("1em").unwrap(), "1em");
        assert_eq!(parse_dimension("1.5em").unwrap(), "1.5em");
        assert_eq!(parse_dimension("1,5em").unwrap(), "1.5em");
        assert_eq!(parse_dimension(".5pt").unwrap(), ".5pt");
        assert_eq!(parse_dimension("1.em").unwrap(), "1.em");
        assert!(parse_dimension("abc").is_err());
    }

    #[test]
    fn test_validate_keyval() {
        assert!(validate_keyval("key=val").is_ok());
        assert!(validate_keyval("key={a,b},other=c").is_ok());
        assert!(validate_keyval("key=\\{,other=c").is_ok());
        assert!(validate_keyval("key=").is_err());
        assert!(validate_keyval("=value").is_err());
        assert!(validate_keyval("key={a").is_err());
    }
}
