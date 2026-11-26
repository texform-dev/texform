//! Pure cursor/slice scalar argument parsers, lightweight token serialization
//! (without `SourceMap`), and [`validate_scalar_argument`].

use logos::Logos;
use texform_interface::syntax_node::{ArgumentValue, ContentMode, Delimiter};

use crate::column_parser::parse_column_template;
use crate::dimension::is_valid_dimension_unit;
use crate::knowledge::{ArgSpec, DelimiterToken};
use crate::lexer::Token;
use crate::parse::config::ParseConfig;
use crate::parse::context::ParseContext;
use crate::parse::cursor::Cursor;
use crate::parse::parser::TokenParser;
use crate::parse::result::Span;

/// Map a single-character spec delimiter to the lexer token that represents it.
///
/// Argspec parsing rejects characters that never lex as a single token of
/// their own (`%`, apostrophes, whitespace, control characters), so every
/// accepted delimiter maps directly here or to [`Token::Char`].
#[inline]
pub(super) fn char_delimiter_token(c: char) -> Token<'static> {
    match c {
        '{' => Token::LBrace,
        '}' => Token::RBrace,
        '[' => Token::LBracket,
        ']' => Token::RBracket,
        '*' => Token::Star,
        '$' => Token::MathShift,
        '&' => Token::Alignment,
        '#' => Token::Parameter,
        '^' => Token::Superscript,
        '_' => Token::Subscript,
        '~' => Token::ActiveChar,
        c => Token::Char(c),
    }
}

/// Borrow a spec-level [`DelimiterToken`] as a zero-copy [`Token`].
#[inline]
pub(super) fn delimiter_token(delimiter: &DelimiterToken) -> Token<'_> {
    match delimiter {
        DelimiterToken::Char(c) => char_delimiter_token(*c),
        DelimiterToken::ControlSeq(name) => Token::ControlSeq(name.as_ref()),
    }
}

/// Returns `true` when `token` matches `delimiter`.
#[inline]
pub(super) fn token_matches_delimiter(token: Token<'_>, delimiter: &DelimiterToken) -> bool {
    token == delimiter_token(delimiter)
}

/// Convert a spec-level [`DelimiterToken`] into a syntax-tree [`Delimiter`].
#[inline]
pub(super) fn syntax_delimiter(delimiter: &'static DelimiterToken) -> Delimiter {
    match delimiter {
        DelimiterToken::Char(c) => Delimiter::Char(*c),
        DelimiterToken::ControlSeq(name) => Delimiter::Control(name.as_ref()),
    }
}

/// Serialize a token slice back to a compact LaTeX string for scalar arguments
/// (`KeyVal`, `Column`, `CSName`) without constructing a `SourceMap`.
///
/// Comments are already omitted by lexing, whitespace runs collapse to a single
/// ASCII space, and a space is inserted between a control word and an
/// immediately following ASCII letter so tokenization boundaries are preserved.
pub(super) fn tokens_to_string(tokens: &[Token<'_>]) -> String {
    let mut out = String::new();
    let mut control_word = false;
    for token in tokens {
        if control_word && matches!(token, Token::Char(c) if c.is_ascii_alphabetic()) {
            out.push(' ');
        }
        control_word = matches!(
            token,
            Token::ControlSeq(name) if name.chars().all(|c| c.is_ascii_alphabetic())
        );
        match *token {
            Token::ControlSeq(name) => {
                out.push('\\');
                out.push_str(name);
            }
            Token::Char(c) => out.push(c),
            Token::Star => out.push('*'),
            Token::Alignment => out.push('&'),
            Token::MathShift => out.push('$'),
            Token::Parameter => out.push('#'),
            Token::Superscript => out.push('^'),
            Token::Subscript => out.push('_'),
            Token::Prime(count) => {
                for _ in 0..count {
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

/// Returns `true` when serializing `tokens` and trimming Unicode whitespace
/// would yield an empty string.
#[inline]
pub(super) fn is_trimmed_empty(tokens: &[Token<'_>]) -> bool {
    tokens.iter().all(|tok| {
        matches!(tok, Token::Whitespaces | Token::Comment)
            || matches!(tok, Token::Char(c) if c.is_whitespace())
    })
}

/// Failure detail from inline scalar parsing over a [`Cursor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ScalarError {
    pub message: &'static str,
    pub span: Span,
}

/// Parse an integer (`[+-]?[0-9]+`) directly from `cursor`.
///
/// Leaves `cursor` untouched on error and returns the failure span.
pub(super) fn parse_integer_cursor(cursor: &mut Cursor<'_, '_>) -> Result<String, ScalarError> {
    let start_pos = cursor.pos();
    let mut out = String::new();

    if let Some(Token::Char(sign @ ('+' | '-'))) = cursor.peek() {
        cursor.bump();
        out.push(sign);
    }

    let digit_start = cursor.pos();
    while let Some(Token::Char(c)) = cursor.peek()
        && c.is_ascii_digit()
    {
        cursor.bump();
        out.push(c);
    }

    if cursor.pos() == digit_start {
        let span = cursor.peek_span();
        cursor.set_pos(start_pos);
        return Err(ScalarError {
            message: "invalid integer argument",
            span,
        });
    }

    Ok(out)
}

/// Parse a whole token slice as an integer (ignoring surrounding whitespace).
pub(super) fn parse_integer_slice(
    tokens: &[Token<'_>],
    spans: &[Span],
    fallback_eoi: usize,
    nullable: bool,
) -> Result<String, &'static str> {
    if nullable && is_trimmed_empty(tokens) {
        return Ok(String::new());
    }
    let mut cursor = Cursor::new(tokens, spans, fallback_eoi);
    cursor.skip_ws();
    let value = parse_integer_cursor(&mut cursor).map_err(|_| "invalid integer argument")?;
    cursor.skip_ws();
    if !cursor.is_at_end() {
        return Err("invalid integer argument");
    }
    Ok(value)
}

/// Parse a unitless decimal (`[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)`) directly
/// from `cursor`.
///
/// Matches MathJax's signed-number syntax: no units, no decimal comma, and no
/// numeric canonicalization of the stored text. Restores `cursor` on error.
pub(super) fn parse_real_cursor(cursor: &mut Cursor<'_, '_>) -> Result<String, ScalarError> {
    let start_pos = cursor.pos();
    let mut out = String::new();

    if let Some(Token::Char(sign @ ('+' | '-'))) = cursor.peek() {
        cursor.bump();
        out.push(sign);
    }

    fn push_digits(cursor: &mut Cursor<'_, '_>, out: &mut String) -> usize {
        let mut count = 0;
        while let Some(Token::Char(c)) = cursor.peek()
            && c.is_ascii_digit()
        {
            cursor.bump();
            out.push(c);
            count += 1;
        }
        count
    }

    let mut digits = push_digits(cursor, &mut out);
    if cursor.peek() == Some(Token::Char('.')) {
        cursor.bump();
        out.push('.');
        digits += push_digits(cursor, &mut out);
    }

    if digits == 0 {
        let span = cursor.peek_span();
        cursor.set_pos(start_pos);
        return Err(ScalarError {
            message: "invalid real number argument",
            span,
        });
    }

    Ok(out)
}

/// Parse a whole token slice as a unitless decimal (ignoring surrounding
/// whitespace).
pub(super) fn parse_real_slice(
    tokens: &[Token<'_>],
    spans: &[Span],
    fallback_eoi: usize,
    nullable: bool,
) -> Result<String, &'static str> {
    if nullable && is_trimmed_empty(tokens) {
        return Ok(String::new());
    }
    let mut cursor = Cursor::new(tokens, spans, fallback_eoi);
    cursor.skip_ws();
    let value = parse_real_cursor(&mut cursor).map_err(|_| "invalid real number argument")?;
    cursor.skip_ws();
    if !cursor.is_at_end() {
        return Err("invalid real number argument");
    }
    Ok(value)
}

/// Parse a LaTeX dimension (`[+-]?[0-9]*([.,][0-9]*)?\s*[a-zA-Z]+`) directly
/// from `cursor`.
///
/// Normalizes decimal commas to dots (`1,5 em` -> `1.5em`) and validates the
/// unit against [`is_valid_dimension_unit`]. Restores `cursor` on error.
pub(super) fn parse_dimension_cursor(cursor: &mut Cursor<'_, '_>) -> Result<String, ScalarError> {
    let start_pos = cursor.pos();

    let sign = match cursor.peek() {
        Some(Token::Char(c @ ('+' | '-'))) => {
            cursor.bump();
            Some(c)
        }
        _ => None,
    };

    let int_start = cursor.pos();
    while let Some(Token::Char(c)) = cursor.peek()
        && c.is_ascii_digit()
    {
        cursor.bump();
    }
    let int_end = cursor.pos();

    let frac_range = if matches!(cursor.peek(), Some(Token::Char('.' | ','))) {
        cursor.bump();
        let frac_start = cursor.pos();
        while let Some(Token::Char(c)) = cursor.peek()
            && c.is_ascii_digit()
        {
            cursor.bump();
        }
        Some(frac_start..cursor.pos())
    } else {
        None
    };

    cursor.skip_ws();

    let unit_start = cursor.pos();
    while let Some(Token::Char(c)) = cursor.peek()
        && c.is_ascii_alphabetic()
    {
        cursor.bump();
    }
    let unit_end = cursor.pos();

    if unit_start == unit_end {
        let span = cursor.peek_span();
        cursor.set_pos(start_pos);
        return Err(ScalarError {
            message: "invalid dimension",
            span,
        });
    }

    let matched_span = cursor.span_of_range(start_pos..unit_end);
    let has_int = int_end > int_start;
    let has_frac = frac_range.as_ref().is_some_and(|r| !r.is_empty());
    if !has_int && !has_frac {
        cursor.set_pos(start_pos);
        return Err(ScalarError {
            message: "invalid dimension",
            span: matched_span,
        });
    }

    let slice = cursor.tokens_slice();
    let mut unit = String::with_capacity(unit_end - unit_start);
    for tok in &slice[unit_start..unit_end] {
        if let Token::Char(c) = *tok {
            unit.push(c);
        }
    }
    if !is_valid_dimension_unit(&unit) {
        cursor.set_pos(start_pos);
        return Err(ScalarError {
            message: "unsupported dimension unit",
            span: matched_span,
        });
    }

    let frac_len = frac_range.as_ref().map_or(0, |r| r.len() + 1);
    let mut value = String::with_capacity(
        usize::from(sign.is_some()) + (int_end - int_start) + frac_len + unit.len(),
    );
    if let Some(s) = sign {
        value.push(s);
    }
    for tok in &slice[int_start..int_end] {
        if let Token::Char(c) = *tok {
            value.push(c);
        }
    }
    if let Some(frac) = frac_range {
        value.push('.');
        for tok in &slice[frac] {
            if let Token::Char(c) = *tok {
                value.push(c);
            }
        }
    }
    value.push_str(&unit);
    Ok(value)
}

/// Parse a whole token slice as a dimension (ignoring surrounding whitespace).
pub(super) fn parse_dimension_slice(
    tokens: &[Token<'_>],
    spans: &[Span],
    fallback_eoi: usize,
    nullable: bool,
) -> Result<String, &'static str> {
    if nullable && is_trimmed_empty(tokens) {
        return Ok(String::new());
    }
    let mut cursor = Cursor::new(tokens, spans, fallback_eoi);
    cursor.skip_ws();
    let value = parse_dimension_cursor(&mut cursor).map_err(|_| "invalid dimension argument")?;
    cursor.skip_ws();
    if !cursor.is_at_end() {
        return Err("invalid dimension argument");
    }
    Ok(value)
}

/// Parse a registered math delimiter (or `.` -> [`Delimiter::None`]) from
/// `cursor`.
pub(super) fn parse_delimiter_cursor(
    ctx: &ParseContext,
    cursor: &mut Cursor<'_, '_>,
) -> Option<Delimiter> {
    let delim = match cursor.peek()? {
        Token::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            let record = ctx.lookup_delimiter(s, false, ContentMode::Math)?;
            if record.name == "." && record.unicode_value.is_empty() {
                Delimiter::None
            } else {
                Delimiter::Char(c)
            }
        }
        Token::LBracket => {
            ctx.lookup_delimiter("[", false, ContentMode::Math)?;
            Delimiter::Char('[')
        }
        Token::RBracket => {
            ctx.lookup_delimiter("]", false, ContentMode::Math)?;
            Delimiter::Char(']')
        }
        Token::ControlSeq(name) => {
            ctx.lookup_delimiter(name, true, ContentMode::Math)?;
            let canonical = ctx.lookup_delimiter_control(name)?;
            Delimiter::Control(canonical)
        }
        _ => return None,
    };
    cursor.bump();
    Some(delim)
}

/// Parse a whole token slice as a math delimiter (ignoring surrounding
/// whitespace).
pub(super) fn parse_delimiter_slice(
    ctx: &ParseContext,
    tokens: &[Token<'_>],
    spans: &[Span],
    fallback_eoi: usize,
    nullable: bool,
) -> Result<Delimiter, &'static str> {
    if nullable && is_trimmed_empty(tokens) {
        return Ok(Delimiter::None);
    }
    let mut cursor = Cursor::new(tokens, spans, fallback_eoi);
    cursor.skip_ws();
    let value = parse_delimiter_cursor(ctx, &mut cursor).ok_or("invalid delimiter argument")?;
    cursor.skip_ws();
    if !cursor.is_at_end() {
        return Err("invalid delimiter argument");
    }
    Ok(value)
}

/// Validate that `raw` is a well-formed `key=value,...` sequence.
///
/// Checks balanced braces, non-empty keys/values, and allows backslash escapes
/// inside both keys and values.
pub(super) fn validate_keyval(raw: &str) -> Result<(), &'static str> {
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
    if !in_value || value.trim().is_empty() {
        return Err("keyval missing value");
    }

    Ok(())
}

/// Parse and validate a `KeyVal` argument from collected tokens.
pub(super) fn parse_keyval_slice(
    tokens: &[Token<'_>],
    nullable: bool,
) -> Result<String, &'static str> {
    let raw = tokens_to_string(tokens);
    let trimmed = raw.trim();
    if nullable && trimmed.is_empty() {
        return Ok(String::new());
    }
    validate_keyval(&raw)?;
    Ok(trimmed.to_string())
}

/// Parse and validate a `Column` template argument from collected tokens.
pub(super) fn parse_column_slice(tokens: &[Token<'_>]) -> Result<String, String> {
    let raw = tokens_to_string(tokens);
    let normalized = raw.trim().to_string();
    parse_column_template(&normalized).map_err(|err| err.to_string())?;
    Ok(normalized)
}

/// Parse and validate a `CSName` argument from collected tokens.
///
/// Rejects embedded control sequences. When `nullable` is `true`, empty input
/// (or whitespace-only input when `collapse_ws_nullable` is `true`) yields
/// `String::new()`.
pub(super) fn parse_cs_name_slice(
    tokens: &[Token<'_>],
    nullable: bool,
    collapse_ws_nullable: bool,
) -> Result<String, &'static str> {
    if nullable {
        let is_empty = if collapse_ws_nullable {
            tokens.iter().all(|tok| *tok == Token::Whitespaces)
        } else {
            tokens.is_empty()
        };
        if is_empty {
            return Ok(String::new());
        }
    }
    if tokens.iter().any(|tok| matches!(tok, Token::ControlSeq(_))) {
        return Err("escape sequence should not appear in CSName");
    }
    Ok(tokens_to_string(tokens))
}

/// Parse exactly one control-sequence token (control word or control symbol)
/// from `cursor`, returning it with its backslash (`\abs`, `\,`).
///
/// The value is a scalar, not a command node: it is never looked up in the
/// knowledge base. Leaves `cursor` untouched on error.
pub(super) fn parse_control_sequence_cursor(
    cursor: &mut Cursor<'_, '_>,
) -> Result<String, ScalarError> {
    match cursor.peek() {
        Some(Token::ControlSeq(name)) => {
            cursor.bump();
            Ok(format!("\\{name}"))
        }
        _ => Err(ScalarError {
            message: "expected a single control sequence",
            span: cursor.peek_span(),
        }),
    }
}

/// Parse a whole token slice as one control sequence (ignoring surrounding
/// whitespace).
pub(super) fn parse_control_sequence_slice(
    tokens: &[Token<'_>],
    spans: &[Span],
    fallback_eoi: usize,
    nullable: bool,
) -> Result<String, &'static str> {
    if nullable && is_trimmed_empty(tokens) {
        return Ok(String::new());
    }
    let mut cursor = Cursor::new(tokens, spans, fallback_eoi);
    cursor.skip_ws();
    let value = parse_control_sequence_cursor(&mut cursor).map_err(|err| err.message)?;
    cursor.skip_ws();
    if !cursor.is_at_end() {
        return Err("expected a single control sequence");
    }
    Ok(value)
}

/// Check that the argument parser reads a stored scalar value back from its slot.
///
/// The slot's token stream is the recorded boundaries around the lexed value.
/// Values the parser rejects or normalizes, such as comments, early boundaries,
/// or `1,5pt`, cannot be represented in the slot.
pub(crate) fn validate_scalar_argument(
    ctx: &ParseContext,
    argument: &crate::ast::Argument,
    spec: &'static ArgSpec,
) -> Result<(), &'static str> {
    use crate::ast::{ArgumentKind as Form, ArgumentValue as Value, Delimiter as Boundary};

    let raw = match &argument.value {
        Value::CSName(raw)
        | Value::ControlSequence(raw)
        | Value::Dimension(raw)
        | Value::Integer(raw)
        | Value::Real(raw)
        | Value::KeyVal(raw)
        | Value::Column(raw) => raw,
        _ => return Ok(()),
    };

    fn boundary(delimiter: &Boundary) -> Token<'_> {
        match delimiter {
            Boundary::None => Token::Char('.'),
            Boundary::Char(c) => char_delimiter_token(*c),
            Boundary::Control(name) => Token::ControlSeq(name.as_str()),
        }
    }

    let (open, close) = match &argument.kind {
        Form::Mandatory | Form::Group => (Token::LBrace, Token::RBrace),
        Form::Optional => (Token::LBracket, Token::RBracket),
        // The protective brace layer precedes the terminator (`{raw}\of`).
        Form::Until { close } => (Token::LBrace, boundary(close)),
        Form::Delimited { open, close } | Form::Paired { open, close } => {
            (boundary(open), boundary(close))
        }
        Form::Star => return Err("scalar arguments cannot use star form"),
    };

    let mut tokens = Vec::new();
    let mut spans = Vec::new();
    tokens.push(open);
    spans.push(Span::point(0));

    for (token, range) in Token::lexer(raw).spanned() {
        let tok = token.map_err(|()| "invalid character in argument")?;
        tokens.push(tok);
        spans.push(Span::new(range.start, range.end));
    }

    if matches!(argument.kind, Form::Until { .. }) {
        tokens.push(Token::RBrace);
        spans.push(Span::point(raw.len()));
    }
    tokens.push(close);
    spans.push(Span::point(raw.len()));

    let config = ParseConfig::default();
    let cursor = Cursor::new(&tokens, &spans, raw.len());
    let mut parser = TokenParser::new(cursor, raw, ctx, &config);

    let parsed = match parser.parse_single_argument(spec, "command argument") {
        Ok(arg) if parser.cursor.is_at_end() => arg.slot,
        _ => None,
    };

    match parsed.map(|arg| arg.value) {
        Some(
            ArgumentValue::CSName(value)
            | ArgumentValue::ControlSequence(value)
            | ArgumentValue::Dimension(value)
            | ArgumentValue::Integer(value)
            | ArgumentValue::Real(value)
            | ArgumentValue::KeyVal(value)
            | ArgumentValue::Column(value),
        ) if value == *raw => Ok(()),
        _ => Err("the argument parser does not read this value back from its slot"),
    }
}
