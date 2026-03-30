use logos::Logos;
use texform_core::lexer::Token;

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
    assert_eq!(lex.next(), Some(Ok(Token::Char('c'))));
    assert_eq!(lex.next(), None);
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
    let mut lex = Token::lexer("a\x00b");
    assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
    assert_eq!(lex.next(), Some(Err(())));
    assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));

    let mut lex = Token::lexer("x\x7Fy");
    assert_eq!(lex.next(), Some(Ok(Token::Char('x'))));
    assert_eq!(lex.next(), Some(Err(())));
    assert_eq!(lex.next(), Some(Ok(Token::Char('y'))));

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
    let mut lex = Token::lexer("f'");
    assert_eq!(lex.next(), Some(Ok(Token::Char('f'))));
    assert_eq!(lex.next(), Some(Ok(Token::Prime(1))));

    let mut lex = Token::lexer("f''");
    assert_eq!(lex.next(), Some(Ok(Token::Char('f'))));
    assert_eq!(lex.next(), Some(Ok(Token::Prime(2))));

    let mut lex = Token::lexer("f'");
    assert_eq!(lex.next(), Some(Ok(Token::Char('f'))));
    assert_eq!(lex.next(), Some(Ok(Token::Prime(1))));
}

#[test]
fn test_nbsp_whitespace() {
    let mut lex = Token::lexer("a\u{00A0}b");
    assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
    assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
    assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));

    let mut lex = Token::lexer("a \u{00A0}\t b");
    assert_eq!(lex.next(), Some(Ok(Token::Char('a'))));
    assert_eq!(lex.next(), Some(Ok(Token::Whitespaces)));
    assert_eq!(lex.next(), Some(Ok(Token::Char('b'))));
}
