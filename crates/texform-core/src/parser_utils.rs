//! Parser utilities and base parsers.
//!
//! This module hosts:
//! - Type aliases and extension traits for parser input handling
//! - Base parsers: delimiters, escapes, characters, whitespace, control sequences
//! - Group builders: implicit/braced/bracket groups
//! - State machines for script parsing and token-level argument parsing
//! - Helper functions for AST normalization

use chumsky::{
    input::{Cursor, InputRef},
    prelude::*,
};

use crate::knowledge;
use crate::lexer::Token;
use texform_interface::syntax_node::{ContentMode, Delimiter, GroupKind, SyntaxNode};

// ============================================================================
// Type Aliases
// ============================================================================

pub type ParserError<'a> = extra::Err<Rich<'a, Token>>;
pub type TokenInput<'a> = &'a [Token];
pub type ParserInput<'src, 'parse> = InputRef<'src, 'parse, TokenInput<'src>, ParserError<'src>>;

// ============================================================================
// Parser Input Extensions
// ============================================================================

/// Ergonomic helpers for building spans and custom errors in imperative parsers.
pub trait ParserInputExt<'src, 'parse> {
    fn span_from_cursor(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
    ) -> SimpleSpan<usize>;

    fn err_since(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token>;

    /// Build an error for the next token without consuming it. Falls back to a
    /// point span at EOF.
    fn err_peek_or_point(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token>;
}

impl<'src, 'parse> ParserInputExt<'src, 'parse> for ParserInput<'src, 'parse> {
    #[inline]
    fn span_from_cursor(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
    ) -> SimpleSpan<usize> {
        self.span_since(start)
    }

    #[inline]
    fn err_since(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token> {
        Rich::custom(self.span_from_cursor(start), msg)
    }

    #[inline]
    fn err_peek_or_point(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token> {
        let start_pos = *start.inner();
        let end = if self.peek().is_some() {
            start_pos + 1
        } else {
            start_pos
        };
        Rich::custom(SimpleSpan::new((), start_pos..end), msg)
    }
}

// ============================================================================
// Base Parsers
// ============================================================================

/// Consume insignificant whitespace tokens and produce no output.
pub fn insignificant_whitespace<'a>() -> impl Parser<'a, TokenInput<'a>, (), ParserError<'a>> + Clone
{
    select! { Token::Whitespaces => () }.repeated().ignored()
}

/// Wrap a parser to accept either `{...}` or inline input.
pub fn maybe_braced<'a, T, P>(
    inner: P,
) -> impl Parser<'a, TokenInput<'a>, T, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, T, ParserError<'a>> + Clone + 'a,
    T: 'a,
{
    let ws = insignificant_whitespace();
    let braced = just(&Token::LBrace)
        .ignore_then(ws.clone())
        .ignore_then(inner.clone())
        .then_ignore(ws)
        .then_ignore(just(&Token::RBrace));

    choice((braced, inner))
}

/// Wrap a parser to accept an optional `[...]` argument.
pub fn optional_bracketed<'a, T, P>(
    inner: P,
) -> impl Parser<'a, TokenInput<'a>, Option<T>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, T, ParserError<'a>> + Clone + 'a,
    T: 'a,
{
    let ws = insignificant_whitespace();
    just(&Token::LBracket)
        .ignore_then(ws.clone())
        .ignore_then(inner)
        .then_ignore(ws)
        .then_ignore(just(&Token::RBracket))
        .or_not()
}

/// Parse a math delimiter token into a typed `Delimiter`.
///
/// Supports:
/// - '.' => Delimiter::None
/// - '(', ')', '[', ']', '|' etc => Delimiter::Char
/// - \langle, \rangle etc => Delimiter::Control
pub fn delimiter<'a>() -> impl Parser<'a, TokenInput<'a>, Delimiter, ParserError<'a>> + Clone {
    select! {
        Token::Char('.') => Delimiter::None,
        Token::Char(c) if matches!(c, '(' | ')' | '[' | ']' | '|' | '<' | '>' | '/' | '\\')
            => Delimiter::Char(c),
        Token::ControlSeq(name) if knowledge::lookup_delimiter_control(name.as_str()).is_some() => {
            Delimiter::Control(knowledge::lookup_delimiter_control(name.as_str()).unwrap())
        }
    }
    .labelled("delimiter")
}

/// Parse escaped symbol control sequences into raw `Char` nodes.
/// 
pub fn escaped_symbol<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if matches!(name.as_str(), "%" | "$" | "&" | "#" | "_" | "{" | "}") => {
            SyntaxNode::Char(name.chars().next().unwrap())
        }
    }
    .labelled("escaped symbol")
}

/// Parse the active character `~` into `ActiveSpace`.
pub fn active_char<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    just(&Token::ActiveChar).to(SyntaxNode::ActiveSpace)
}

/// Parse plain math characters (including `*` and `&` tokens).
pub fn math_char<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => SyntaxNode::Char(c),
        Token::Star => SyntaxNode::Char('*'),
        Token::Alignment => SyntaxNode::Char('&'),
    }
    .labelled("math character")
}

/// Parse and coalesce consecutive text characters/whitespace into a single `Text` node.
pub fn text_chunk<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => c,
        Token::Whitespaces => ' ',
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
) -> impl Parser<'a, TokenInput<'a>, (), ParserError<'a>> + Clone {
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
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
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
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    just(&Token::LBrace)
        .ignore_then(content)
        .then_ignore(just(&Token::RBrace))
        .map(move |children| SyntaxNode::Group {
            mode,
            kind: GroupKind::Explicit,
            children,
        })
}

/// Parse a bracketed `[...]` group with the given content parser.
pub fn bracket_group_parser<'a, P>(
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    just(&Token::LBracket)
        .ignore_then(content)
        .then_ignore(just(&Token::RBracket))
        .map(move |children| SyntaxNode::Group {
            mode,
            kind: GroupKind::Explicit,
            children,
        })
}

/// Parse `\left ... \right` delimited math group.
pub fn delimited_group_parser<'a, P>(
    math_content: P,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    control_seq("left")
        .ignore_then(delimiter())
        .then(math_content)
        .then_ignore(control_seq("right"))
        .then(delimiter())
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
    P: Parser<'src, TokenInput<'src>, SyntaxNode, ParserError<'src>> + Clone + 'src,
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

/// Parse an inline integer value from the token stream.
pub(crate) fn parse_inline_integer<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
) -> Result<String, Rich<'src, Token>> {
    let start = input.cursor();
    let mut out = String::new();

    if let Some(Token::Char(c @ ('+' | '-'))) = input.peek() {
        out.push(c);
        input.next();
    }

    let mut digits = 0usize;
    while let Some(Token::Char(c)) = input.peek() {
        if c.is_ascii_digit() {
            out.push(c);
            digits += 1;
            input.next();
        } else {
            break;
        }
    }

    if digits == 0 {
        return Err(input.err_since(&start, "invalid integer argument"));
    }

    Ok(out)
}

/// Collect tokens for an inline dimension value (sign, digits, unit).
///
/// Returns raw string for subsequent validation via `validate_dimension`.
pub(crate) fn collect_inline_dimension_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
) -> Result<String, Rich<'src, Token>> {
    let start = input.cursor();
    let mut raw = String::new();

    // Optional sign
    if let Some(Token::Char(c @ ('+' | '-'))) = input.peek() {
        raw.push(c);
        input.next();
    }

    // Integer part
    while let Some(Token::Char(c)) = input.peek() {
        if c.is_ascii_digit() {
            raw.push(c);
            input.next();
        } else {
            break;
        }
    }

    // Fractional part
    if let Some(Token::Char(sep @ ('.' | ','))) = input.peek() {
        raw.push(sep);
        input.next();
        while let Some(Token::Char(c)) = input.peek() {
            if c.is_ascii_digit() {
                raw.push(c);
                input.next();
            } else {
                break;
            }
        }
    }

    // Skip whitespace before unit
    while matches!(input.peek(), Some(Token::Whitespaces)) {
        raw.push(' ');
        input.next();
    }

    // Unit
    while let Some(Token::Char(c)) = input.peek() {
        if c.is_ascii_alphabetic() {
            raw.push(c);
            input.next();
        } else {
            break;
        }
    }

    if raw.is_empty() {
        return Err(input.err_since(&start, "invalid dimension argument"));
    }

    Ok(raw)
}

// ============================================================================
// String Validation Helpers
// ============================================================================

/// Validate and normalize a raw integer string.
/// Validate an integer string.
pub(crate) fn validate_integer(raw: &str) -> Result<(), &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("invalid integer");
    }

    let mut chars = trimmed.chars();
    let mut digits = 0usize;

    if let Some(first) = chars.next() {
        if first == '+' || first == '-' {
            // sign is allowed, digits must follow
        } else if first.is_ascii_digit() {
            digits += 1;
        } else {
            return Err("invalid integer");
        }
    } else {
        return Err("invalid integer");
    }

    for c in chars {
        if c.is_ascii_digit() {
            digits += 1;
        } else {
            return Err("invalid integer");
        }
    }

    if digits == 0 {
        return Err("invalid integer");
    }

    Ok(())
}

/// Validate a dimension string using MathJax-compatible shape rules.
pub(crate) fn validate_dimension(raw: &str) -> Result<(), &'static str> {
    parse_dimension_parts(raw).map(|_| ())
}

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

/// Normalize a valid integer string for storage.
pub(crate) fn normalize_integer_string(raw: &str) -> String {
    raw.trim().to_string()
}

/// Normalize a valid dimension string for storage.
pub(crate) fn normalize_dimension_string(raw: &str) -> Result<String, &'static str> {
    let (value, unit) = parse_dimension_parts(raw)?;
    Ok(format!("{}{}", value, unit))
}

/// Normalize a valid keyval string for storage.
pub(crate) fn normalize_keyval_string(raw: &str) -> String {
    raw.trim().to_string()
}

// ============================================================================
// Value Parsers (Braced/Inline + Validate + Normalize)
// ============================================================================

/// Parse an integer value argument (required or optional).
///
/// - Required: accepts `{...}` or inline form
/// - Optional: accepts `[...]` or returns empty string
pub fn integer_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenInput<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let raw = if required {
            if matches!(input.peek(), Some(Token::LBrace)) {
                let tokens = collect_braced_tokens(input, false)?;
                tokens_to_string(&tokens)
            } else {
                parse_inline_integer(input)?
            }
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            return Ok(String::new());
        };

        validate_integer(&raw).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg)
        })?;

        Ok(normalize_integer_string(&raw))
    })
}

/// Parse a dimension value argument (required or optional).
///
/// - Required: accepts `{...}` or inline form
/// - Optional: accepts `[...]` or returns empty string
pub fn dimension_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenInput<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let raw = if required {
            if matches!(input.peek(), Some(Token::LBrace)) {
                let tokens = collect_braced_tokens(input, false)?;
                tokens_to_string(&tokens)
            } else {
                collect_inline_dimension_tokens(input)?
            }
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            return Ok(String::new());
        };

        validate_dimension(&raw).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg)
        })?;

        normalize_dimension_string(&raw).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg)
        })
    })
}

/// Parse a keyval value argument (required or optional).
///
/// - Required: must be `{...}` form
/// - Optional: accepts `[...]` or returns empty string
pub fn keyval_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenInput<'a>, String, ParserError<'a>> + Clone {
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

fn parse_dimension_parts(raw: &str) -> Result<(String, String), &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("invalid dimension");
    }

    let mut chars = trimmed.chars().peekable();
    let mut value = String::new();

    if matches!(chars.peek(), Some('+' | '-')) {
        value.push(chars.next().unwrap());
    }

    let mut int_digits = 0usize;
    while let Some(c) = chars.peek().copied() {
        if c.is_ascii_digit() {
            value.push(c);
            int_digits += 1;
            chars.next();
        } else {
            break;
        }
    }

    let mut frac_digits = 0usize;
    if let Some(sep @ ('.' | ',')) = chars.peek().copied() {
        value.push(if sep == ',' { '.' } else { sep });
        chars.next();
        while let Some(c) = chars.peek().copied() {
            if c.is_ascii_digit() {
                value.push(c);
                frac_digits += 1;
                chars.next();
            } else {
                break;
            }
        }
    }

    if int_digits == 0 && frac_digits == 0 {
        return Err("invalid dimension");
    }

    while let Some(c) = chars.peek().copied() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }

    let mut unit = String::new();
    while let Some(c) = chars.peek().copied() {
        if c.is_ascii_alphabetic() {
            unit.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if unit.is_empty() {
        return Err("missing dimension unit");
    }

    if chars.peek().is_some() {
        return Err("invalid dimension");
    }

    if !is_valid_dimension_unit(&unit) {
        return Err("unsupported dimension unit");
    }

    Ok((value, unit))
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

    #[test]
    fn test_validate_integer() {
        assert!(validate_integer("123").is_ok());
        assert!(validate_integer("  +42 ").is_ok());
        assert!(validate_integer("-0").is_ok());
        assert!(validate_integer("12.5").is_err());
        assert!(validate_integer("abc").is_err());
        assert!(validate_integer("+").is_err());
        assert!(validate_integer("1 2").is_err());
    }

    #[test]
    fn test_validate_dimension() {
        assert!(validate_dimension("1em").is_ok());
        assert!(validate_dimension(" 1,5 em ").is_ok());
        assert!(validate_dimension(".5pt").is_ok());
        assert!(validate_dimension("1.em").is_ok());
        assert!(validate_dimension("1").is_err());
        assert!(validate_dimension("1em2").is_err());
        assert!(validate_dimension("abc").is_err());
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
