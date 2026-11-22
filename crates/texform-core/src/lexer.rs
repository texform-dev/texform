//! LaTeX lexical analysis powered by [Logos](https://docs.rs/logos).
//!
//! The lexer maps LaTeX source bytes into a flat stream of [`Token`]s.
//! It follows the TeX catcode model in a simplified form:
//!
//! - Catcode 0 (Escape) triggers control-sequence scanning.
//! - Catcodes 1–8 map to dedicated structural tokens.
//! - Catcode 10 (Spacer) and catcode 14 (Comment) are handled as
//!   whitespace / skip rules.
//! - Catcodes 11/12 (Letter/Other) fall through to [`Token::Char`].
//! - Catcodes 9 (Ignore) and 15 (Invalid) are not matched by any rule
//!   and produce lexer errors automatically.
//!
//! The lexer is intentionally lossy: comments are discarded and runs of
//! whitespace are collapsed, matching TeXForm's normalization goals.

use logos::Logos;

use crate::parse::{ParseDiagnostic, ParseDiagnosticKind, Span};

/// Token types for LaTeX lexical analysis.
///
/// This lexer recognizes LaTeX tokens based on character categories (catcode).
/// It provides a simplified view where special characters and control sequences
/// are identified, while preserving enough information for parsing.
#[derive(Logos, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Token<'src> {
    // --- Control Sequences ---
    /// Control sequence: \command
    /// - catcode 0 (Escape): backslash triggers control sequence scanning
    /// - Matches: \<letters> (control word) or \<single-char> (control symbol)
    /// - Returns the command name without the backslash
    #[regex(r"\\(?:[a-zA-Z]+|[^\r\n])", |lex| &lex.slice()[1..])]
    #[regex(r"\\(?:\r\n|\r|\n)", |_| " ")]
    #[token("\\", |_| " ")]
    ControlSeq(&'src str),

    /// Active character: ~
    /// - catcode 13: Active Character
    /// - Treated as a command but without escape character
    /// - In LaTeX, ~ produces a non-breaking space
    #[token("~")]
    ActiveChar,

    // --- Structural Tokens ---
    /// Left brace: {
    /// - catcode 1: Begin Group
    /// - Used for grouping and delimiting arguments
    #[token("{")]
    LBrace,

    /// Right brace: }
    /// - catcode 2: End Group
    /// - Closes groups started by LBrace
    #[token("}")]
    RBrace,

    /// Dollar sign: $
    /// - catcode 3: Math Shift
    /// - Toggles inline math mode; $$ indicates display math
    #[token("$")]
    MathShift,

    /// Ampersand: &
    /// - catcode 4: Alignment Tab
    /// - Used in tables and alignment environments
    #[token("&")]
    Alignment,

    /// Hash/pound sign: #
    /// - catcode 6: Parameter
    /// - Used in macro definitions and arguments
    #[token("#")]
    Parameter,

    /// Caret: ^
    /// - catcode 7: Superscript
    /// - Indicates superscript in math mode
    #[token("^")]
    Superscript,

    /// Underscore: _
    /// - catcode 8: Subscript
    /// - Indicates subscript in math mode
    #[token("_")]
    Subscript,

    /// Star/Asterisk: *
    /// - catcode 12: Other
    /// - Used for starred command variants (e.g., \section*)
    /// - Must be checked immediately after command names
    #[token("*")]
    Star,

    /// Left bracket: [
    /// - catcode 12: Other
    /// - Often used for optional arguments
    #[token("[")]
    LBracket,

    /// Right bracket: ]
    /// - catcode 12: Other
    /// - Closes optional arguments
    #[token("]")]
    RBracket,

    /// Prime mark(s): one or more ' or U+2019
    /// - In math mode, represents derivative notation (f' = f^\prime)
    /// - Multiple primes are common: f'', f'''
    /// - We store the count to simplify parser handling
    #[regex(r"['\u2019]+", callback = |lex| lex.slice().chars().count())]
    Prime(usize),

    // --- Whitespace and Comments ---
    /// Whitespace: spaces, tabs, CR/LF newlines, form feeds, non-breaking space
    /// - catcode 10: Spacer
    /// - Multiple consecutive whitespace characters are merged
    /// - CR is treated as a line ending, matching TeX catcode 5 (end of line)
    /// - Includes U+00A0 (non-breaking space) for copy-paste behavior
    ///
    /// Character membership matches [`is_whitespace_char`].
    #[regex(r"[ \t\n\r\f\u{00A0}]+")]
    Whitespaces,

    /// Comment: % to end of line
    /// - catcode 14: Comment
    /// - Lexer consumes everything from % to line end (inclusive)
    /// - Line endings are CR, LF, or CRLF
    /// - Comments are discarded and do not produce tokens
    #[regex(r"%[^\r\n]*\r?\n?", logos::skip)]
    Comment,

    // --- Character Tokens ---
    /// Regular character: letters, digits, punctuation, Unicode (excluding invalid chars)
    /// - catcode 11: Letter (a-z, A-Z)
    /// - catcode 12: Other (digits, punctuation, etc.)
    /// - Matches any single printable character not covered by above patterns
    /// - Has lowest priority (1) to act as fallback
    ///
    /// Note: Control characters (catcode 9, 15) are NOT matched by any pattern
    /// and will cause lexing errors automatically:
    /// - catcode 9 (Ignore): \x00-\x08, \x0B-\x1F (control chars except \t, \n, \f, \r)
    /// - catcode 15 (Invalid): \x7F (DEL character)
    #[regex(r"[\x20-\x7E\u{80}-\u{10FFFF}]", priority = 1, callback = |lex| {
        let slice = lex.slice();
        slice.chars().next().unwrap()
    })]
    Char(char),
}

/// Ordinary whitespace characters recognized by [`Token::Whitespaces`].
///
/// This matches the Logos regex on [`Token::Whitespaces`] exactly: SPACE, TAB,
/// LF, CR, FORM FEED, and NO-BREAK SPACE. Other Unicode whitespace (for example
/// U+2007, U+202F, U+3000) is intentionally excluded and tokenizes as
/// [`Token::Char`].
#[inline]
pub fn is_whitespace_char(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000C}' | '\u{00A0}')
}

/// Parallel token and span buffers produced by lexical analysis (SoA layout).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TokenStream<'src> {
    pub tokens: Vec<Token<'src>>,
    pub spans: Vec<Span>,
}

/// Lex the whole source, reporting the first untokenizable character.
pub(crate) fn lex_source(src: &str) -> Result<TokenStream<'_>, ParseDiagnostic> {
    let mut tokens = Vec::new();
    let mut spans = Vec::new();
    for (token, range) in Token::lexer(src).spanned() {
        match token {
            Ok(tok) => {
                tokens.push(tok);
                spans.push(Span::new(range.start, range.end));
            }
            Err(()) => {
                let slice = &src[range.start..range.end];
                return Err(ParseDiagnostic {
                    kind: ParseDiagnosticKind::InvalidCharacter,
                    message: format!(
                        "Invalid character {:?} at byte offset {}..{}",
                        slice, range.start, range.end
                    ),
                    span: Span::new(range.start, range.end),
                    expected: Vec::new(),
                    found: Some(slice.to_owned()),
                    contexts: Vec::new(),
                });
            }
        }
    }
    Ok(TokenStream { tokens, spans })
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::ControlSeq(name) => write!(f, "\\{name}"),
            Token::ActiveChar => write!(f, "~"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::MathShift => write!(f, "$"),
            Token::Alignment => write!(f, "&"),
            Token::Parameter => write!(f, "#"),
            Token::Superscript => write!(f, "^"),
            Token::Subscript => write!(f, "_"),
            Token::Star => write!(f, "*"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Prime(n) => {
                for _ in 0..*n {
                    write!(f, "'")?;
                }
                Ok(())
            }
            Token::Whitespaces => write!(f, " "),
            Token::Comment => write!(f, "%"),
            Token::Char(c) => write!(f, "{c}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseConfig, ParseContext};

    #[test]
    fn lex_source_collects_tokens_and_byte_spans() {
        let stream = lex_source(r"\alpha + é''\\").expect("valid input should lex");
        assert_eq!(
            stream.tokens,
            vec![
                Token::ControlSeq("alpha"),
                Token::Whitespaces,
                Token::Char('+'),
                Token::Whitespaces,
                Token::Char('é'),
                Token::Prime(2),
                Token::ControlSeq("\\"),
            ]
        );
        assert_eq!(
            stream.spans,
            vec![
                Span::new(0, 6),
                Span::new(6, 7),
                Span::new(7, 8),
                Span::new(8, 9),
                Span::new(9, 11),
                Span::new(11, 13),
                Span::new(13, 15),
            ]
        );
    }

    #[test]
    fn lex_source_skips_comments_and_preserves_subsequent_spans() {
        let src = "a% ignore\r\nb % tail";
        let stream = lex_source(src).expect("comments should be skipped");
        assert_eq!(
            stream.tokens,
            vec![Token::Char('a'), Token::Char('b'), Token::Whitespaces]
        );
        assert_eq!(
            stream.spans,
            vec![Span::new(0, 1), Span::new(11, 12), Span::new(12, 13)]
        );
    }

    #[test]
    fn lex_source_invalid_character_matches_parse_diagnostic() {
        let ctx = ParseContext::shared();
        let config = ParseConfig::default();
        for src in ["é\u{0}x", "\\begin{matrix}\u{1b}\\left x", "x\x7Fy"] {
            let lex_diag = lex_source(src).expect_err("invalid character should fail");
            let parse_result = ctx.parse(src, &config);
            assert!(parse_result.document.is_none());
            assert_eq!(parse_result.diagnostics.len(), 1);
            assert_eq!(lex_diag, parse_result.diagnostics[0], "{src:?}");
            assert_eq!(lex_diag.kind, ParseDiagnosticKind::InvalidCharacter);
            assert!(lex_diag.expected.is_empty());
            assert!(lex_diag.contexts.is_empty());
        }
    }
}
