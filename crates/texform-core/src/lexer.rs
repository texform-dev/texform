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

/// Token types for LaTeX lexical analysis.
///
/// This lexer recognizes LaTeX tokens based on character categories (catcode).
/// It provides a simplified view where special characters and control sequences
/// are identified, while preserving enough information for parsing.
#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    // --- Control Sequences ---
    /// Control sequence: \command
    /// - catcode 0 (Escape): backslash triggers control sequence scanning
    /// - Matches: \<letters> (control word) or \<single-char> (control symbol)
    /// - Returns the command name without the backslash
    #[regex(r"\\(?:[a-zA-Z]+|.)", |lex| {
        let slice = lex.slice();
        slice[1..].to_string()
    })]
    ControlSeq(String),

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
    /// Whitespace: spaces, tabs, newlines, form feeds, non-breaking space
    /// - catcode 10: Spacer
    /// - Multiple consecutive whitespace characters are merged
    /// - Includes U+00A0 (non-breaking space) for copy-paste compatibility
    #[regex(r"[ \t\n\f\u{00A0}]+")]
    Whitespaces,

    /// Comment: % to end of line
    /// - catcode 14: Comment
    /// - Lexer consumes everything from % to line end (inclusive)
    /// - Comments are discarded and do not produce tokens
    #[regex(r"%[^\n]*\n?", logos::skip)]
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
    /// - catcode 9 (Ignore): \x00-\x08, \x0B-\x1F (control chars except \t, \n, \f)
    /// - catcode 15 (Invalid): \x7F (DEL character)
    #[regex(r"[\x20-\x7E\u{80}-\u{10FFFF}]", priority = 1, callback = |lex| {
        let slice = lex.slice();
        slice.chars().next().unwrap()
    })]
    Char(char),
}

impl std::fmt::Display for Token {
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
