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

    /// Prime mark: ' or ' (U+2019 right single quotation mark)
    /// - In math mode, represents derivative notation (f' = f^\prime)
    /// - Multiple primes are common: f'', f'''
    #[regex(r"['\u2019]")]
    Prime,

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
    #[regex(r"%[^\n]*\n?", |lex| lex.slice().to_string())]
    Comment(String),

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_sequences() {
        let mut lex = Token::lexer(r"\alpha \frac \$ ~");
        assert_eq!(lex.next(), Some(Ok(Token::ControlSeq("alpha".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(lex.next(), Some(Ok(Token::ControlSeq("frac".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(lex.next(), Some(Ok(Token::ControlSeq("$".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(lex.next(), Some(Ok(Token::ActiveChar)));
    }

    #[test]
    fn test_structural_tokens() {
        let mut lex = Token::lexer(r"{a^b_c}$&");
        assert_eq!(lex.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
        assert_eq!(lex.next(), Some(Ok(Token::Superscript)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));
        assert_eq!(lex.next(), Some(Ok(Token::Subscript)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('c'))));
        assert_eq!(lex.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lex.next(), Some(Ok(Token::MathShift)));
        assert_eq!(lex.next(), Some(Ok(Token::Alignment)));
    }

    #[test]
    fn test_whitespace_and_comments() {
        let mut lex = Token::lexer("a  \t\n  b % comment\nc");
        assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(
            lex.next(),
            Some(Ok(Token::Comment("% comment\n".to_string())))
        );
        assert_eq!(lex.next(), Some(Ok(Token::Char('c'))));
    }

    #[test]
    fn test_brackets() {
        let mut lex = Token::lexer(r"\frac[1]{2}");
        assert_eq!(lex.next(), Some(Ok(Token::ControlSeq("frac".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::LBracket)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('1'))));
        assert_eq!(lex.next(), Some(Ok(Token::RBracket)));
        assert_eq!(lex.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('2'))));
        assert_eq!(lex.next(), Some(Ok(Token::RBrace)));
    }

    #[test]
    fn test_star_token() {
        // Test starred command variants
        let mut lex = Token::lexer(r"\section*{Title}");
        assert_eq!(
            lex.next(),
            Some(Ok(Token::ControlSeq("section".to_string())))
        );
        assert_eq!(lex.next(), Some(Ok(Token::Star)));
        assert_eq!(lex.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('T'))));
        assert_eq!(lex.next(), Some(Ok(Token::Char('i'))));
        assert_eq!(lex.next(), Some(Ok(Token::Char('t'))));
        assert_eq!(lex.next(), Some(Ok(Token::Char('l'))));
        assert_eq!(lex.next(), Some(Ok(Token::Char('e'))));
        assert_eq!(lex.next(), Some(Ok(Token::RBrace)));

        // Test star in math context (multiplication)
        let mut lex = Token::lexer(r"a*b");
        assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
        assert_eq!(lex.next(), Some(Ok(Token::Star)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));
    }

    #[test]
    fn test_parameter_marker() {
        let mut lex = Token::lexer(r"#1 #2");
        assert_eq!(lex.next(), Some(Ok(Token::Parameter)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('1'))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(lex.next(), Some(Ok(Token::Parameter)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('2'))));
    }

    #[test]
    fn test_invalid_characters() {
        // Test catcode 9 (Ignore): null character
        let mut lex = Token::lexer("a\x00b");
        assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
        assert_eq!(lex.next(), Some(Err(())));
        assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));

        // Test catcode 15 (Invalid): DEL character
        let mut lex = Token::lexer("x\x7Fy");
        assert_eq!(lex.next(), Some(Ok(Token::Char('x'))));
        assert_eq!(lex.next(), Some(Err(())));
        assert_eq!(lex.next(), Some(Ok(Token::Char('y'))));

        // Test control character
        let mut lex = Token::lexer("m\x01n");
        assert_eq!(lex.next(), Some(Ok(Token::Char('m'))));
        assert_eq!(lex.next(), Some(Err(())));
        assert_eq!(lex.next(), Some(Ok(Token::Char('n'))));
    }

    #[test]
    fn test_active_char() {
        let mut lex = Token::lexer("a~b");
        assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
        assert_eq!(lex.next(), Some(Ok(Token::ActiveChar)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));
    }

    #[test]
    fn test_prime_token() {
        // ASCII apostrophe
        let mut lex = Token::lexer("f'");
        assert_eq!(lex.next(), Some(Ok(Token::Char('f'))));
        assert_eq!(lex.next(), Some(Ok(Token::Prime)));

        // Multiple primes
        let mut lex = Token::lexer("f''");
        assert_eq!(lex.next(), Some(Ok(Token::Char('f'))));
        assert_eq!(lex.next(), Some(Ok(Token::Prime)));
        assert_eq!(lex.next(), Some(Ok(Token::Prime)));

        // Unicode right single quotation mark (U+2019)
        let mut lex = Token::lexer("f'");
        assert_eq!(lex.next(), Some(Ok(Token::Char('f'))));
        assert_eq!(lex.next(), Some(Ok(Token::Prime)));
    }

    #[test]
    fn test_nbsp_whitespace() {
        // Non-breaking space (U+00A0) should be treated as whitespace
        let mut lex = Token::lexer("a\u{00A0}b");
        assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));

        // Mixed whitespace including NBSP
        let mut lex = Token::lexer("a \u{00A0}\t b");
        assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
        assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
        assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));
    }
}
