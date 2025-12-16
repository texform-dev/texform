use texform_core::lexer::Token;
use texform_core::parser::parse;
use texform_interface::syntax_node::{ArgumentKind, ContentMode, Delimiter, GroupKind, SyntaxNode};

// ========================================================================
// Stage 1-2 Tests (Basic parsing)
// ========================================================================

#[test]
fn test_parse_simple_chars() {
    let tokens = vec![Token::Char('a'), Token::Char('b'), Token::Char('c')];
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group {
            mode,
            kind,
            children,
        } => {
            assert_eq!(mode, ContentMode::Math);
            assert_eq!(kind, GroupKind::Implicit);
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('a'));
            assert_eq!(children[1], SyntaxNode::Char('b'));
            assert_eq!(children[2], SyntaxNode::Char('c'));
        }
        _ => panic!("Expected Group node"),
    }
}

#[test]
fn test_parse_empty() {
    let tokens = vec![];
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group {
            mode,
            kind,
            children,
        } => {
            assert_eq!(mode, ContentMode::Math);
            assert_eq!(kind, GroupKind::Implicit);
            assert!(children.is_empty());
        }
        _ => panic!("Expected Group node"),
    }
}

#[test]
fn test_escaped_symbols() {
    let tokens = vec![
        Token::ControlSeq("%".to_string()),
        Token::ControlSeq("$".to_string()),
        Token::ControlSeq("&".to_string()),
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('%'));
            assert_eq!(children[1], SyntaxNode::Char('$'));
            assert_eq!(children[2], SyntaxNode::Char('&'));
        }
        _ => panic!("Expected Group"),
    }
}

#[test]
fn test_active_char() {
    let tokens = vec![Token::Char('a'), Token::ActiveChar, Token::Char('b')];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('a'));
            assert_eq!(children[1], SyntaxNode::ActiveSpace);
            assert_eq!(children[2], SyntaxNode::Char('b'));
        }
        _ => panic!("Expected Group"),
    }
}

#[test]
fn test_explicit_group() {
    let tokens = vec![Token::LBrace, Token::Char('a'), Token::RBrace];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group {
            mode,
            kind,
            children,
        } => {
            assert_eq!(mode, ContentMode::Math);
            assert_eq!(kind, GroupKind::Implicit);
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Group {
                    mode: inner_mode,
                    kind: inner_kind,
                    children: inner_children,
                } => {
                    assert_eq!(*inner_mode, ContentMode::Math);
                    assert_eq!(*inner_kind, GroupKind::Explicit);
                    assert_eq!(inner_children.len(), 1);
                    assert_eq!(inner_children[0], SyntaxNode::Char('a'));
                }
                _ => panic!("Expected inner Group"),
            }
        }
        _ => panic!("Expected Group node"),
    }
}

#[test]
fn test_nested_groups() {
    let tokens = vec![
        Token::Char('a'),
        Token::LBrace,
        Token::Char('b'),
        Token::LBrace,
        Token::Char('c'),
        Token::RBrace,
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 2);
            assert_eq!(children[0], SyntaxNode::Char('a'));

            match &children[1] {
                SyntaxNode::Group {
                    kind: GroupKind::Explicit,
                    children: inner1,
                    ..
                } => {
                    assert_eq!(inner1.len(), 2);
                    assert_eq!(inner1[0], SyntaxNode::Char('b'));

                    match &inner1[1] {
                        SyntaxNode::Group {
                            kind: GroupKind::Explicit,
                            children: inner2,
                            ..
                        } => {
                            assert_eq!(inner2.len(), 1);
                            assert_eq!(inner2[0], SyntaxNode::Char('c'));
                        }
                        _ => panic!("Expected second level Group"),
                    }
                }
                _ => panic!("Expected first level Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_simple_script() {
    let tokens = vec![Token::Char('x'), Token::Superscript, Token::Char('2')];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    subscript,
                    superscript,
                } => {
                    assert_eq!(**base, SyntaxNode::Char('x'));
                    assert!(subscript.is_none());
                    assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('2'));
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected Group node"),
    }
}

#[test]
fn test_script_normalization() {
    // "x^a_b" and "x_b^a" should produce equivalent AST
    let tokens1 = vec![
        Token::Char('x'),
        Token::Superscript,
        Token::Char('a'),
        Token::Subscript,
        Token::Char('b'),
    ];

    let tokens2 = vec![
        Token::Char('x'),
        Token::Subscript,
        Token::Char('b'),
        Token::Superscript,
        Token::Char('a'),
    ];

    let result1 = parse(&tokens1, false).unwrap();
    let result2 = parse(&tokens2, false).unwrap();

    assert_eq!(result1, result2);
}

#[test]
fn test_script_duplicate_last_wins() {
    // "x^a^b" -> double exponent should error
    let tokens = vec![
        Token::Char('x'),
        Token::Superscript,
        Token::Char('a'),
        Token::Superscript,
        Token::Char('b'),
    ];

    let result = parse(&tokens, false);
    assert!(result.is_err());
}

#[test]
fn test_script_with_group() {
    // "x^{ab}" -> Scripted with group as superscript
    let tokens = vec![
        Token::Char('x'),
        Token::Superscript,
        Token::LBrace,
        Token::Char('a'),
        Token::Char('b'),
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Scripted {
                base,
                superscript,
                subscript,
            } => {
                assert_eq!(**base, SyntaxNode::Char('x'));
                assert!(subscript.is_none());

                match superscript.as_ref().unwrap().as_ref() {
                    SyntaxNode::Group { children, .. } => {
                        assert_eq!(children.len(), 2);
                        assert_eq!(children[0], SyntaxNode::Char('a'));
                        assert_eq!(children[1], SyntaxNode::Char('b'));
                    }
                    _ => panic!("Expected Group as superscript"),
                }
            }
            _ => panic!("Expected Scripted node"),
        },
        _ => panic!("Expected Group node"),
    }
}

// ========================================================================
// Stage 3 Tests (Command parsing)
// ========================================================================

#[test]
fn test_frac_command() {
    // "\frac{a}{b}"
    let tokens = vec![
        Token::ControlSeq("frac".to_string()),
        Token::LBrace,
        Token::Char('a'),
        Token::RBrace,
        Token::LBrace,
        Token::Char('b'),
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Command {
                    name,
                    starred,
                    args,
                } => {
                    assert_eq!(name, "frac");
                    assert!(!starred);
                    assert_eq!(args.len(), 2);

                    assert_eq!(args[0].kind, ArgumentKind::Mandatory);
                    assert_eq!(args[1].kind, ArgumentKind::Mandatory);
                }
                _ => panic!("Expected Command node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_sqrt_without_optional() {
    // "\sqrt{x}"
    let tokens = vec![
        Token::ControlSeq("sqrt".to_string()),
        Token::LBrace,
        Token::Char('x'),
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);

                    // Optional arg should be empty
                    assert_eq!(args[0].kind, ArgumentKind::Optional);
                    match &args[0].value {
                        SyntaxNode::Group { children, .. } => {
                            assert!(children.is_empty());
                        }
                        _ => panic!("Expected Group in optional arg"),
                    }

                    // Mandatory arg
                    assert_eq!(args[1].kind, ArgumentKind::Mandatory);
                }
                _ => panic!("Expected Command node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_sqrt_with_optional() {
    // "\sqrt[3]{8}"
    let tokens = vec![
        Token::ControlSeq("sqrt".to_string()),
        Token::LBracket,
        Token::Char('3'),
        Token::RBracket,
        Token::LBrace,
        Token::Char('8'),
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);

                    // Optional arg - normalized to single Char
                    assert_eq!(args[0].kind, ArgumentKind::Optional);
                    assert_eq!(args[0].value, SyntaxNode::Char('3'));

                    // Mandatory arg - normalized to single Char
                    assert_eq!(args[1].kind, ArgumentKind::Mandatory);
                    assert_eq!(args[1].value, SyntaxNode::Char('8'));
                }
                _ => panic!("Expected Command node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_text_command() {
    // "\text{hello}"
    let tokens = vec![
        Token::ControlSeq("text".to_string()),
        Token::LBrace,
        Token::Char('h'),
        Token::Char('e'),
        Token::Char('l'),
        Token::Char('l'),
        Token::Char('o'),
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                assert_eq!(args.len(), 1);
                match &args[0].value {
                    SyntaxNode::Text(s) => {
                        assert_eq!(s, "hello");
                    }
                    _ => panic!("Expected Text node"),
                }
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_nested_commands() {
    // "\frac{a}{\sqrt{b}}"
    let tokens = vec![
        Token::ControlSeq("frac".to_string()),
        Token::LBrace,
        Token::Char('a'),
        Token::RBrace,
        Token::LBrace,
        Token::ControlSeq("sqrt".to_string()),
        Token::LBrace,
        Token::Char('b'),
        Token::RBrace,
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");

                    // First argument should be normalized to single Char
                    assert_eq!(args[0].value, SyntaxNode::Char('a'));

                    // Second argument should be \sqrt command (normalized from single-element group)
                    match &args[1].value {
                        SyntaxNode::Command { name, .. } => {
                            assert_eq!(name, "sqrt");
                        }
                        _ => panic!("Expected sqrt Command in arg 1"),
                    }
                }
                _ => panic!("Expected Command node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_unknown_command_nonstrict() {
    // "\unknown{x}" in non-strict mode
    let tokens = vec![
        Token::ControlSeq("unknown".to_string()),
        Token::LBrace,
        Token::Char('x'),
        Token::RBrace,
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 2);

            // First: UnknownCommand node
            match &children[0] {
                SyntaxNode::UnknownCommand { name, starred } => {
                    assert_eq!(name, "unknown");
                    assert!(!starred);
                }
                _ => panic!("Expected UnknownCommand node"),
            }

            // Second: explicit group {x}
            match &children[1] {
                SyntaxNode::Group {
                    kind: GroupKind::Explicit,
                    children,
                    ..
                } => {
                    assert_eq!(children.len(), 1);
                    assert_eq!(children[0], SyntaxNode::Char('x'));
                }
                _ => panic!("Expected Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_unknown_command_strict() {
    // "\unknown{x}" in strict mode should error
    let tokens = vec![
        Token::ControlSeq("unknown".to_string()),
        Token::LBrace,
        Token::Char('x'),
        Token::RBrace,
    ];

    let result = parse(&tokens, true);
    assert!(result.is_err());
}

#[test]
fn test_blacklisted_command() {
    // "\ifnum" is blacklisted
    let tokens = vec![Token::ControlSeq("ifnum".to_string())];

    let result = parse(&tokens, false);
    assert!(result.is_err());
}

// ========================================================================
// Stage 4 Tests (Infix and Declarative commands)
// ========================================================================

#[test]
fn test_infix_over_simple() {
    // "a \over b"
    let tokens = vec![
        Token::Char('a'),
        Token::ControlSeq("over".to_string()),
        Token::Char('b'),
    ];

    let result = match parse(&tokens, false) {
        Ok(r) => r,
        Err(errors) => {
            eprintln!("Parse errors:");
            for err in &errors {
                eprintln!("  {:?}", err);
            }
            panic!("Parse failed with {} errors", errors.len());
        }
    };

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Infix {
                    name,
                    starred,
                    left,
                    right,
                    args,
                } => {
                    assert_eq!(name, "over");
                    assert!(!starred);
                    assert!(args.is_empty());
                    assert_eq!(**left, SyntaxNode::Char('a'));
                    assert_eq!(**right, SyntaxNode::Char('b'));
                }
                _ => panic!("Expected Infix node, got {:?}", children[0]),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_infix_choose() {
    // "n \choose k"
    let tokens = vec![
        Token::Char('n'),
        Token::ControlSeq("choose".to_string()),
        Token::Char('k'),
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Infix {
                    name, left, right, ..
                } => {
                    assert_eq!(name, "choose");
                    assert_eq!(**left, SyntaxNode::Char('n'));
                    assert_eq!(**right, SyntaxNode::Char('k'));
                }
                _ => panic!("Expected Infix node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_infix_multiple_items() {
    // "a+b \over c+d"
    let tokens = vec![
        Token::Char('a'),
        Token::Char('+'),
        Token::Char('b'),
        Token::ControlSeq("over".to_string()),
        Token::Char('c'),
        Token::Char('+'),
        Token::Char('d'),
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Infix { left, right, .. } => {
                    // Left should be folded into implicit group
                    match &**left {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert_eq!(children.len(), 3);
                            assert_eq!(children[0], SyntaxNode::Char('a'));
                            assert_eq!(children[1], SyntaxNode::Char('+'));
                            assert_eq!(children[2], SyntaxNode::Char('b'));
                        }
                        _ => panic!("Expected implicit group for left operand"),
                    }

                    // Right should be folded into implicit group
                    match &**right {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert_eq!(children.len(), 3);
                            assert_eq!(children[0], SyntaxNode::Char('c'));
                            assert_eq!(children[1], SyntaxNode::Char('+'));
                            assert_eq!(children[2], SyntaxNode::Char('d'));
                        }
                        _ => panic!("Expected implicit group for right operand"),
                    }
                }
                _ => panic!("Expected Infix node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_declarative_bfseries() {
    // "\bfseries text" -> Declarative with scope containing "text"
    let tokens = vec![
        Token::ControlSeq("bfseries".to_string()),
        Token::Char('t'),
        Token::Char('e'),
        Token::Char('x'),
        Token::Char('t'),
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Declarative {
                    name,
                    starred,
                    args,
                    scope,
                } => {
                    assert_eq!(name, "bfseries");
                    assert!(!starred);
                    assert!(args.is_empty());

                    // Scope should contain 4 chars
                    match &**scope {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert_eq!(children.len(), 4);
                        }
                        _ => panic!("Expected Group for scope"),
                    }
                }
                _ => panic!("Expected Declarative node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_declarative_with_leading() {
    // "a \bfseries b c"
    let tokens = vec![
        Token::Char('a'),
        Token::ControlSeq("bfseries".to_string()),
        Token::Char('b'),
        Token::Char('c'),
    ];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 2);

            // First item: 'a'
            assert_eq!(children[0], SyntaxNode::Char('a'));

            // Second item: Declarative
            match &children[1] {
                SyntaxNode::Declarative { name, scope, .. } => {
                    assert_eq!(name, "bfseries");

                    match &**scope {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            assert_eq!(children[0], SyntaxNode::Char('b'));
                            assert_eq!(children[1], SyntaxNode::Char('c'));
                        }
                        _ => panic!("Expected Group for scope"),
                    }
                }
                _ => panic!("Expected Declarative node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_declarative_empty_scope() {
    // "\bfseries" with nothing after it
    let tokens = vec![Token::ControlSeq("bfseries".to_string())];

    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Declarative { scope, .. } => {
                    // Scope should be empty implicit group
                    match &**scope {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert!(children.is_empty());
                        }
                        _ => panic!("Expected empty Group for scope"),
                    }
                }
                _ => panic!("Expected Declarative node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

// ========================================================================
// Stage 5 Tests (Text mode, inline math, delimited groups, environments)
// ========================================================================

// Note: These tests use the lexer to generate tokens from LaTeX source
use logos::Logos;
use texform_core::lexer::Token as LexerToken;

macro_rules! lex_tokens {
    ($source:expr) => {{
        let mut tokens = Vec::new();
        for result in LexerToken::lexer($source) {
            match result {
                Ok(tok) => tokens.push(tok),
                Err(_) => panic!("Lexer error in test: {}", $source),
            }
        }
        tokens
    }};
}

// TODO: Add test for text mode - currently parse_text_block is not exposed
// We can test text mode through \text{} command which uses Text mode args

#[test]
fn test_text_in_command() {
    // "\text{Hello World}" - text mode in command argument
    let tokens = lex_tokens!(r"\text{Hello World}");
    let result = parse(&tokens, false).unwrap();

    // Debug print the result
    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "text");
                    assert_eq!(args.len(), 1);
                    match &args[0].value {
                        SyntaxNode::Group { mode, children, .. } => {
                            assert_eq!(*mode, ContentMode::Text);
                            assert_eq!(children.len(), 1);
                            assert_eq!(children[0], SyntaxNode::Text("Hello World".to_string()));
                        }
                        SyntaxNode::Text(s) => {
                            assert_eq!(s, "Hello World");
                        }
                        other => {
                            panic!("Expected Group or Text for text argument, got {:?}", other)
                        }
                    }
                }
                _ => panic!("Expected Command node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_text_inline_math_segment() {
    let tokens = lex_tokens!(r"\text{foo$a+b$bar}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                assert_eq!(args.len(), 1);
                match &args[0].value {
                    SyntaxNode::Group { mode, children, .. } => {
                        assert_eq!(*mode, ContentMode::Text);
                        assert_eq!(children.len(), 3);
                        assert_eq!(children[0], SyntaxNode::Text("foo".to_string()));
                        match &children[1] {
                            SyntaxNode::Group {
                                kind,
                                children: math_children,
                                ..
                            } => {
                                assert_eq!(*kind, GroupKind::InlineMath);
                                assert_eq!(math_children.len(), 3);
                                assert_eq!(math_children[0], SyntaxNode::Char('a'));
                                assert_eq!(math_children[1], SyntaxNode::Char('+'));
                                assert_eq!(math_children[2], SyntaxNode::Char('b'));
                            }
                            _ => panic!("Expected inline math group"),
                        }
                        assert_eq!(children[2], SyntaxNode::Text("bar".to_string()));
                    }
                    _ => panic!("Expected text group"),
                }
            }
            _ => panic!("Expected text command"),
        },
        _ => panic!("Expected root group"),
    }
}

#[test]
fn test_text_inline_math_active_char_and_command() {
    let tokens = lex_tokens!(r"\text{A~$x$B\frac{a}{b}}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                assert_eq!(args.len(), 1);
                match &args[0].value {
                    SyntaxNode::Group { mode, children, .. } => {
                        assert_eq!(*mode, ContentMode::Text);
                        assert!(children.len() >= 5);
                        assert_eq!(children[0], SyntaxNode::Text("A".to_string()));
                        assert_eq!(children[1], SyntaxNode::ActiveSpace);
                        match &children[2] {
                            SyntaxNode::Group {
                                kind,
                                children: math_children,
                                ..
                            } => {
                                assert_eq!(*kind, GroupKind::InlineMath);
                                assert_eq!(math_children.len(), 1);
                                assert_eq!(math_children[0], SyntaxNode::Char('x'));
                            }
                            _ => panic!("Expected inline math for $x$"),
                        }
                        assert_eq!(children[3], SyntaxNode::Text("B".to_string()));
                        match &children[4] {
                            SyntaxNode::Command { name, args, .. } => {
                                assert_eq!(name, "frac");
                                assert_eq!(args.len(), 2);
                            }
                            _ => panic!("Expected fraction command"),
                        }
                    }
                    _ => panic!("Expected text group"),
                }
            }
            _ => panic!("Expected text command"),
        },
        _ => panic!("Expected root group"),
    }
}

#[test]
fn test_delimited_group_simple() {
    // "\left( a+b \right)"
    let tokens = lex_tokens!(r"\left(a+b\right)");
    let result = parse(&tokens, false).unwrap();

    eprintln!("Parsed result:\n{:#?}", result);

    match result {
        SyntaxNode::Group { ref children, .. } => {
            eprintln!("Root children count: {}", children.len());
            for (i, child) in children.iter().enumerate() {
                eprintln!("Child {}: {:?}", i, child);
            }
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Group {
                    kind,
                    mode,
                    children,
                    ..
                } => {
                    assert_eq!(*mode, ContentMode::Math);
                    match kind {
                        GroupKind::Delimited { left, right } => {
                            assert_eq!(*left, Delimiter::Char('('));
                            assert_eq!(*right, Delimiter::Char(')'));
                            assert_eq!(children.len(), 3);
                            assert_eq!(children[0], SyntaxNode::Char('a'));
                            assert_eq!(children[1], SyntaxNode::Char('+'));
                            assert_eq!(children[2], SyntaxNode::Char('b'));
                        }
                        _ => panic!("Expected Delimited GroupKind"),
                    }
                }
                _ => panic!("Expected Delimited Group node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_delimited_group_with_dot() {
    // "\left. x \right|" - dot means no delimiter
    let tokens = lex_tokens!(r"\left.x\right|");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Group { kind, children, .. } => match kind {
                    GroupKind::Delimited { left, right } => {
                        assert_eq!(*left, Delimiter::None);
                        assert_eq!(*right, Delimiter::Char('|'));
                        assert_eq!(children.len(), 1);
                        assert_eq!(children[0], SyntaxNode::Char('x'));
                    }
                    _ => panic!("Expected Delimited GroupKind"),
                },
                _ => panic!("Expected Delimited Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_delimited_group_nested() {
    // "\left( \frac{a}{b} \right)"
    let tokens = lex_tokens!(r"\left(\frac{a}{b}\right)");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Group { kind, children, .. } => match kind {
                    GroupKind::Delimited { left, right } => {
                        assert_eq!(*left, Delimiter::Char('('));
                        assert_eq!(*right, Delimiter::Char(')'));
                        assert_eq!(children.len(), 1);
                        match &children[0] {
                            SyntaxNode::Command { name, .. } => {
                                assert_eq!(name, "frac");
                            }
                            _ => panic!("Expected frac command inside delimited group"),
                        }
                    }
                    _ => panic!("Expected Delimited GroupKind"),
                },
                _ => panic!("Expected Delimited Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_environment_basic() {
    let tokens = lex_tokens!(r"\begin{matrix}ab\end{matrix}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Environment {
                    name,
                    starred,
                    args,
                    body,
                } => {
                    assert_eq!(name, "matrix");
                    assert!(!starred);
                    assert!(args.is_empty());
                    match &**body {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            assert_eq!(children[0], SyntaxNode::Char('a'));
                            assert_eq!(children[1], SyntaxNode::Char('b'));
                        }
                        _ => panic!("Expected group body"),
                    }
                }
                _ => panic!("Expected Environment node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_environment_nested() {
    let tokens = lex_tokens!(r"\begin{matrix}\begin{matrix}x\end{matrix}\end{matrix}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Environment { body, .. } => match &**body {
                SyntaxNode::Group { children, .. } => {
                    assert_eq!(children.len(), 1);
                    match &children[0] {
                        SyntaxNode::Environment { body, .. } => match &**body {
                            SyntaxNode::Group { children, .. } => {
                                assert_eq!(children.len(), 1);
                                assert_eq!(children[0], SyntaxNode::Char('x'));
                            }
                            _ => panic!("Expected inner environment body group"),
                        },
                        _ => panic!("Expected inner Environment"),
                    }
                }
                _ => panic!("Expected outer environment body"),
            },
            _ => panic!("Expected outer Environment"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_environment_name_mismatch() {
    let tokens = lex_tokens!(r"\begin{matrix}a\end{align}");
    let result = parse(&tokens, false);
    assert!(result.is_err());
}

// ========================================================================
// Stage 6 Tests (Argument normalization and shorthand syntax)
// ========================================================================

#[test]
fn test_frac_shorthand_equivalence() {
    // "\frac ab" should produce the same AST as "\frac{a}{b}"
    let tokens_short = lex_tokens!(r"\frac ab");
    let tokens_full = lex_tokens!(r"\frac{a}{b}");

    let result_short = parse(&tokens_short, false).unwrap();
    let result_full = parse(&tokens_full, false).unwrap();

    assert_eq!(result_short, result_full);
}

#[test]
fn test_frac_mixed_shorthand() {
    // "\frac a{bc}" - one shorthand, one braced
    let tokens_mixed = lex_tokens!(r"\frac a{bc}");
    let result = parse(&tokens_mixed, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");
                    assert_eq!(args.len(), 2);
                    // First arg: single char 'a'
                    assert_eq!(args[0].value, SyntaxNode::Char('a'));
                    // Second arg: group with 'bc'
                    match &args[1].value {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            assert_eq!(children[0], SyntaxNode::Char('b'));
                            assert_eq!(children[1], SyntaxNode::Char('c'));
                        }
                        _ => panic!("Expected Group for second arg"),
                    }
                }
                _ => panic!("Expected Command"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_frac_shorthand_with_command() {
    // "\frac\alpha\beta" - shorthand with commands as arguments
    let tokens = lex_tokens!(r"\frac\alpha\beta");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");
                    assert_eq!(args.len(), 2);
                    // Both args should be Command nodes
                    match &args[0].value {
                        SyntaxNode::Command { name, .. } => assert_eq!(name, "alpha"),
                        _ => panic!("Expected alpha command"),
                    }
                    match &args[1].value {
                        SyntaxNode::Command { name, .. } => assert_eq!(name, "beta"),
                        _ => panic!("Expected beta command"),
                    }
                }
                _ => panic!("Expected frac Command"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_sqrt_shorthand_equivalence() {
    // "\sqrt 2" should produce the same AST as "\sqrt{2}"
    let tokens_short = lex_tokens!(r"\sqrt 2");
    let tokens_full = lex_tokens!(r"\sqrt{2}");

    let result_short = parse(&tokens_short, false).unwrap();
    let result_full = parse(&tokens_full, false).unwrap();

    assert_eq!(result_short, result_full);
}

#[test]
fn test_sqrt_with_optional_shorthand() {
    // "\sqrt[3]8" vs "\sqrt[3]{8}"
    let tokens_short = lex_tokens!(r"\sqrt[3]8");
    let tokens_full = lex_tokens!(r"\sqrt[3]{8}");

    let result_short = parse(&tokens_short, false).unwrap();
    let result_full = parse(&tokens_full, false).unwrap();

    assert_eq!(result_short, result_full);
}

// ========================================================================
// Stage 7 Tests (Prime/apostrophe parsing)
// ========================================================================

#[test]
fn test_prime_single() {
    // "f'" -> Scripted with prime as superscript
    let tokens = lex_tokens!(r"f'");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    superscript,
                    subscript,
                } => {
                    assert_eq!(**base, SyntaxNode::Char('f'));
                    assert!(subscript.is_none());
                    assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('\''));
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_prime_multiple() {
    // "f'''" -> Scripted with 3 primes grouped as superscript
    let tokens = lex_tokens!(r"f'''");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base, superscript, ..
                } => {
                    assert_eq!(**base, SyntaxNode::Char('f'));
                    match superscript.as_ref().unwrap().as_ref() {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 3);
                            assert!(children.iter().all(|c| *c == SyntaxNode::Char('\'')));
                        }
                        _ => panic!("Expected Group of primes"),
                    }
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_prime_with_superscript() {
    // "f'^2" -> prime and superscript combined
    let tokens = lex_tokens!(r"f'^2");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base, superscript, ..
                } => {
                    assert_eq!(**base, SyntaxNode::Char('f'));
                    // Superscript should be grouped: prime then 2
                    match superscript.as_ref().unwrap().as_ref() {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            assert_eq!(children[0], SyntaxNode::Char('\''));
                            assert_eq!(children[1], SyntaxNode::Char('2'));
                        }
                        _ => panic!("Expected Group for combined superscript"),
                    }
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_prime_with_subscript() {
    // "f'_n" -> prime as superscript, n as subscript
    let tokens = lex_tokens!(r"f'_n");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    superscript,
                    subscript,
                } => {
                    assert_eq!(**base, SyntaxNode::Char('f'));
                    assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('\''));
                    assert_eq!(**subscript.as_ref().unwrap(), SyntaxNode::Char('n'));
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_prime_on_sub_grouped() {
    // "x^{'_{a}}" -> prime with grouped subscript inside superscript
    let tokens = lex_tokens!(r"x^{'_{a}}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => match superscript
                    .as_ref()
                    .unwrap()
                    .as_ref()
                {
                    SyntaxNode::Group { children, .. } => {
                        assert_eq!(children.len(), 1);
                        match &children[0] {
                            SyntaxNode::Scripted {
                                superscript,
                                subscript,
                                base,
                            } => {
                                assert!(subscript.is_some());
                                assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('\''));
                                match base.as_ref() {
                                    SyntaxNode::Group { .. } => {} // empty base inside nested scripted
                                    _ => panic!("Expected nested empty base"),
                                }
                            }
                            other => panic!(
                                "Expected scripted prime inside superscript, got {:?}",
                                other
                            ),
                        }
                    }
                    other => panic!("Expected grouped superscript, got {:?}", other),
                },
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_empty_base_superscript() {
    // "^2" -> empty base with superscript
    let tokens = lex_tokens!(r"^2");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    superscript,
                    subscript,
                } => {
                    assert!(subscript.is_none());
                    match base.as_ref() {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert!(children.is_empty());
                        }
                        _ => panic!("Expected empty group base"),
                    }
                    assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('2'));
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_empty_base_subscript() {
    // "_3" -> empty base with subscript
    let tokens = lex_tokens!(r"_3");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    subscript,
                    superscript,
                } => {
                    assert!(superscript.is_none());
                    match base.as_ref() {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert!(children.is_empty());
                        }
                        _ => panic!("Expected empty group base"),
                    }
                    assert_eq!(**subscript.as_ref().unwrap(), SyntaxNode::Char('3'));
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_preprime() {
    // "'x" -> prime with empty base then x
    let tokens = lex_tokens!(r"'x");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    superscript,
                    subscript,
                } => {
                    assert!(subscript.is_none());
                    match base.as_ref() {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert!(children.is_empty());
                        }
                        _ => panic!("Expected empty group base"),
                    }
                    assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('\''));
                }
                _ => panic!("Expected Scripted node"),
            }
            assert_eq!(children[1], SyntaxNode::Char('x'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_double_superscript_error() {
    let tokens = lex_tokens!(r"x^2^3");
    assert!(parse(&tokens, false).is_err());
}

#[test]
fn test_double_subscript_error() {
    let tokens = lex_tokens!(r"x_2_3");
    assert!(parse(&tokens, false).is_err());
}

#[test]
fn test_prime_after_superscript_error() {
    let tokens = lex_tokens!(r"x^a'");
    assert!(parse(&tokens, false).is_err());
}

#[test]
fn test_prime_brace_superscript_error() {
    // x'^' should fail because ^ expects a superscript atom, not a prime marker
    let tokens = lex_tokens!(r"x'^'");
    assert!(parse(&tokens, false).is_err());
}

#[test]
fn test_prime_on_prime_nested() {
    // "x^{'^{'}}" - prime on prime nesting
    let tokens = lex_tokens!(r"x^{'^{'}}");
    let result = parse(&tokens, false).unwrap();

    fn count_primes(node: &SyntaxNode) -> usize {
        match node {
            SyntaxNode::Char('\'') => 1,
            SyntaxNode::Group { children, .. } => children.iter().map(count_primes).sum(),
            SyntaxNode::Scripted {
                base,
                superscript,
                subscript,
            } => {
                count_primes(base)
                    + superscript.as_ref().map(|n| count_primes(n)).unwrap_or(0)
                    + subscript.as_ref().map(|n| count_primes(n)).unwrap_or(0)
            }
            _ => 0,
        }
    }

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    match superscript.as_ref().unwrap().as_ref() {
                        node => {
                            // Expect at least two primes somewhere inside the superscript tree
                            assert!(
                                count_primes(node) >= 2,
                                "expected at least two primes, got {}",
                                count_primes(node)
                            );
                        }
                    }
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_prime_on_sup_nested() {
    // "x^{a^{'}}" - superscript contains a prime on its own empty base
    let tokens = lex_tokens!(r"x^{a^{'}}");
    let result = parse(&tokens, false).unwrap();

    fn count_primes(node: &SyntaxNode) -> usize {
        match node {
            SyntaxNode::Char('\'') => 1,
            SyntaxNode::Group { children, .. } => children.iter().map(count_primes).sum(),
            SyntaxNode::Scripted {
                base,
                superscript,
                subscript,
            } => {
                count_primes(base)
                    + superscript.as_ref().map(|n| count_primes(n)).unwrap_or(0)
                    + subscript.as_ref().map(|n| count_primes(n)).unwrap_or(0)
            }
            _ => 0,
        }
    }

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    match superscript.as_ref().unwrap().as_ref() {
                        node => {
                            assert!(
                                count_primes(node) >= 1,
                                "expected at least one prime in superscript"
                            );
                        }
                    }
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_sup_on_prime_nested() {
    // "x^{'^{a}}" - prime that itself has a superscript a
    let tokens = lex_tokens!(r"x^{'^{a}}");
    let result = parse(&tokens, false).unwrap();

    fn count_a(node: &SyntaxNode) -> usize {
        match node {
            SyntaxNode::Char('a') => 1,
            SyntaxNode::Group { children, .. } => children.iter().map(count_a).sum(),
            SyntaxNode::Scripted {
                base,
                superscript,
                subscript,
            } => {
                count_a(base)
                    + superscript.as_ref().map(|n| count_a(n)).unwrap_or(0)
                    + subscript.as_ref().map(|n| count_a(n)).unwrap_or(0)
            }
            _ => 0,
        }
    }

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    let sup = superscript.as_ref().unwrap();
                    assert!(
                        count_a(sup) >= 1,
                        "expected at least one 'a' in superscript tree"
                    );
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_prime_then_superscript_merge() {
    let tokens = lex_tokens!(r"x'^a");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    match superscript.as_ref().unwrap().as_ref() {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            assert_eq!(children[0], SyntaxNode::Char('\''));
                            assert_eq!(children[1], SyntaxNode::Char('a'));
                        }
                        other => panic!("Expected grouped superscript, got {:?}", other),
                    }
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_double_prime_then_superscript_merge() {
    let tokens = lex_tokens!(r"x''^a");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    match superscript.as_ref().unwrap().as_ref() {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            match &children[0] {
                                SyntaxNode::Group {
                                    children: prime_children,
                                    ..
                                } => {
                                    assert_eq!(prime_children.len(), 2);
                                    assert_eq!(prime_children[0], SyntaxNode::Char('\''));
                                    assert_eq!(prime_children[1], SyntaxNode::Char('\''));
                                }
                                other => panic!("Expected grouped prime node, got {:?}", other),
                            }
                            assert_eq!(children[1], SyntaxNode::Char('a'));
                        }
                        other => panic!("Expected grouped superscript, got {:?}", other),
                    }
                }
                _ => panic!("Expected Scripted node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

// ========================================================================
// Stage 8 Tests (Control sequence delimiters)
// ========================================================================

#[test]
fn test_delimited_group_langle_rangle() {
    // "\left\langle x \right\rangle"
    let tokens = lex_tokens!(r"\left\langle x\right\rangle");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Group { kind, children, .. } => match kind {
                    GroupKind::Delimited { left, right } => {
                        assert_eq!(*left, Delimiter::Control("langle"));
                        assert_eq!(*right, Delimiter::Control("rangle"));
                        assert_eq!(children.len(), 1);
                        assert_eq!(children[0], SyntaxNode::Char('x'));
                    }
                    _ => panic!("Expected Delimited GroupKind"),
                },
                _ => panic!("Expected Delimited Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_delimited_group_lfloor_rfloor() {
    // "\left\lfloor x \right\rfloor"
    let tokens = lex_tokens!(r"\left\lfloor x\right\rfloor");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Group { kind, .. } => match kind {
                    GroupKind::Delimited { left, right } => {
                        assert_eq!(*left, Delimiter::Control("lfloor"));
                        assert_eq!(*right, Delimiter::Control("rfloor"));
                    }
                    _ => panic!("Expected Delimited GroupKind"),
                },
                _ => panic!("Expected Delimited Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

// ========================================================================
// Stage 9 Tests (Starred variants)
// ========================================================================

#[test]
fn test_starred_environment() {
    // "\begin{align*}...\end{align*}"
    let tokens = lex_tokens!(r"\begin{align*}a+b\end{align*}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Environment {
                    name,
                    starred,
                    body,
                    ..
                } => {
                    assert_eq!(name, "align");
                    assert!(*starred);
                    match &**body {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 3);
                        }
                        _ => panic!("Expected group body"),
                    }
                }
                _ => panic!("Expected Environment node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

// ========================================================================
// Stage 10 Tests (Whitespace handling)
// ========================================================================

#[test]
fn test_whitespace_ignored_in_frac_args() {
    // "\frac  {a}  {b}" should equal "\frac{a}{b}"
    let tokens_spaces = lex_tokens!(r"\frac  {a}  {b}");
    let tokens_no_spaces = lex_tokens!(r"\frac{a}{b}");

    let result_spaces = parse(&tokens_spaces, false).unwrap();
    let result_no_spaces = parse(&tokens_no_spaces, false).unwrap();

    assert_eq!(result_spaces, result_no_spaces);
}

#[test]
fn test_whitespace_ignored_in_scripts() {
    // "x ^ 2" should equal "x^2"
    let tokens_spaces = lex_tokens!(r"x ^ 2");
    let tokens_no_spaces = lex_tokens!(r"x^2");

    let result_spaces = parse(&tokens_spaces, false).unwrap();
    let result_no_spaces = parse(&tokens_no_spaces, false).unwrap();

    assert_eq!(result_spaces, result_no_spaces);
}

#[test]
fn test_whitespace_ignored_between_items() {
    // "a  b  c" should produce 3 Char nodes
    let tokens = lex_tokens!(r"a  b  c");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('a'));
            assert_eq!(children[1], SyntaxNode::Char('b'));
            assert_eq!(children[2], SyntaxNode::Char('c'));
        }
        _ => panic!("Expected root Group"),
    }
}

// ========================================================================
// Stage 11 Tests (Other edge cases)
// ========================================================================

#[test]
fn test_empty_group() {
    // "{}" should parse as empty explicit group
    let tokens = lex_tokens!(r"{}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Group {
                    kind: GroupKind::Explicit,
                    children,
                    ..
                } => {
                    assert!(children.is_empty());
                }
                _ => panic!("Expected empty Explicit Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_consecutive_groups() {
    // "{a}{b}" should parse as two groups
    let tokens = lex_tokens!(r"{a}{b}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Group {
                    kind: GroupKind::Explicit,
                    children,
                    ..
                } => {
                    assert_eq!(children.len(), 1);
                    assert_eq!(children[0], SyntaxNode::Char('a'));
                }
                _ => panic!("Expected first Explicit Group"),
            }
            match &children[1] {
                SyntaxNode::Group {
                    kind: GroupKind::Explicit,
                    children,
                    ..
                } => {
                    assert_eq!(children.len(), 1);
                    assert_eq!(children[0], SyntaxNode::Char('b'));
                }
                _ => panic!("Expected second Explicit Group"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_script_in_argument() {
    // "\frac{x^2}{y}" - script inside argument
    let tokens = lex_tokens!(r"\frac{x^2}{y}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");
                    // First arg should contain scripted x^2
                    match &args[0].value {
                        SyntaxNode::Scripted {
                            base, superscript, ..
                        } => {
                            assert_eq!(**base, SyntaxNode::Char('x'));
                            assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('2'));
                        }
                        _ => panic!("Expected Scripted in first arg"),
                    }
                    // Second arg is just 'y'
                    assert_eq!(args[1].value, SyntaxNode::Char('y'));
                }
                _ => panic!("Expected Command"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_consecutive_commands() {
    // "\alpha\beta\gamma" should parse as 3 commands
    let tokens = lex_tokens!(r"\alpha\beta\gamma");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 3);
            for (i, expected) in ["alpha", "beta", "gamma"].iter().enumerate() {
                match &children[i] {
                    SyntaxNode::Command { name, .. } => {
                        assert_eq!(name, *expected);
                    }
                    _ => panic!("Expected Command {}", expected),
                }
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_infix_then_declarative() {
    // "a \over b \bfseries c" - infix followed by declarative
    let tokens = lex_tokens!(r"a \over b \bfseries c");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            // Should have infix node and declarative node
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Infix {
                    name, left, right, ..
                } => {
                    assert_eq!(name, "over");
                    assert_eq!(**left, SyntaxNode::Char('a'));
                    assert_eq!(**right, SyntaxNode::Char('b'));
                }
                _ => panic!("Expected Infix node"),
            }
            match &children[1] {
                SyntaxNode::Declarative { name, scope, .. } => {
                    assert_eq!(name, "bfseries");
                    match &**scope {
                        SyntaxNode::Char('c') => {}
                        _ => panic!("Expected Char('c') in scope"),
                    }
                }
                _ => panic!("Expected Declarative node"),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_alignment_char() {
    // "&" should parse as Char('&') in math mode
    let tokens = lex_tokens!(r"a & b");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('a'));
            assert_eq!(children[1], SyntaxNode::Char('&'));
            assert_eq!(children[2], SyntaxNode::Char('b'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_text_escaped_symbols() {
    // "\text{\%\$\&}" - escaped symbols in text mode
    let tokens = lex_tokens!(r"\text{\%\$\&}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                match &args[0].value {
                    SyntaxNode::Group { children, .. } => {
                        assert_eq!(children.len(), 3);
                        assert_eq!(children[0], SyntaxNode::Char('%'));
                        assert_eq!(children[1], SyntaxNode::Char('$'));
                        assert_eq!(children[2], SyntaxNode::Char('&'));
                    }
                    _ => panic!("Expected Group in text arg"),
                }
            }
            _ => panic!("Expected text Command"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_text_explicit_group() {
    // "\text{{a}}" - explicit group inside text
    let tokens = lex_tokens!(r"\text{{a}}");
    let result = parse(&tokens, false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                match &args[0].value {
                    SyntaxNode::Group {
                        kind: GroupKind::Explicit,
                        children,
                        mode,
                        ..
                    } => {
                        assert_eq!(*mode, ContentMode::Text);
                        assert_eq!(children.len(), 1);
                        assert_eq!(children[0], SyntaxNode::Text("a".to_string()));
                    }
                    _ => panic!("Expected Explicit Group in text arg"),
                }
            }
            _ => panic!("Expected text Command"),
        },
        _ => panic!("Expected root Group"),
    }
}
