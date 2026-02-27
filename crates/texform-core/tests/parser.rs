use std::sync::Once;

use texform_core::knowledge;
use texform_core::lexer::Token;
use texform_core::parser::parse as raw_parse;
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};

fn parse(
    src: &str,
    strict: bool,
) -> Result<
    (SyntaxNode, chumsky::span::SimpleSpan),
    Vec<chumsky::error::Rich<'static, Token>>,
> {
    init_test_kb();
    raw_parse(src, strict).map_err(|errors| errors.into_iter().map(|e| e.into_owned()).collect())
}

fn init_test_kb() {
    static INIT: Once = Once::new();
    INIT.call_once(knowledge::init_test_defaults);
}

fn expect_arg(slot: &Option<Argument>) -> &Argument {
    slot.as_ref()
        .unwrap_or_else(|| panic!("Expected argument slot to be present"))
}

fn unwrap_content(slot: &Option<Argument>) -> &SyntaxNode {
    match &expect_arg(slot).value {
        ArgumentValue::Content(node) => node,
        _ => panic!("Expected content argument"),
    }
}

// ========================================================================
// Stage 1-2 Tests (Basic parsing)
// ========================================================================

#[test]
fn test_parse_simple_chars() {
    let (result, _) = parse("abc", false).unwrap();

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
    let (result, _) = parse("", false).unwrap();

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
    let (result, _) = parse(r"\%\$\&", false).unwrap();

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
    let (result, _) = parse("a~b", false).unwrap();

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
    let (result, _) = parse("{a}", false).unwrap();

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
    let (result, _) = parse("a{b{c}}", false).unwrap();

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
    let (result, _) = parse("x^2", false).unwrap();

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
    let (result1, _) = parse("x^a_b", false).unwrap();
    let (result2, _) = parse("x_b^a", false).unwrap();

    assert_eq!(result1, result2);
}

#[test]
fn test_script_duplicate_last_wins() {
    // "x^a^b" -> double exponent should error
    let result = parse("x^a^b", false);
    assert!(result.is_err());
}

#[test]
fn test_script_with_group() {
    // "x^{ab}" -> Scripted with group as superscript
    let (result, _) = parse("x^{ab}", false).unwrap();

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
    let (result, _) = parse(r"\frac{a}{b}", false).unwrap();

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

                    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Mandatory);
                    assert_eq!(expect_arg(&args[1]).kind, ArgumentKind::Mandatory);
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
    let (result, _) = parse(r"\sqrt{x}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);

                    // Optional slot should be absent
                    assert!(args[0].is_none());

                    // Mandatory arg
                    assert_eq!(expect_arg(&args[1]).kind, ArgumentKind::Mandatory);
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
    let (result, _) = parse(r"\sqrt[3]{8}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);

                    // Optional arg - normalized to single Char
                    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Optional);
                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::Content(SyntaxNode::Char('3'))
                    );

                    // Mandatory arg - normalized to single Char
                    assert_eq!(expect_arg(&args[1]).kind, ArgumentKind::Mandatory);
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::Content(SyntaxNode::Char('8'))
                    );
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
    let (result, _) = parse(r"\text{hello}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                assert_eq!(args.len(), 1);
                match unwrap_content(&args[0]) {
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
fn test_delimiter_argument() {
    // "\delim\langle"
    let (result, _) = parse(r"\delim\langle", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "delim");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Mandatory);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Delimiter(Delimiter::Control("langle"))
                );
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_dimension_argument() {
    // "\hspace1em"
    let (result, _) = parse(r"\hspace1em", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "hspace");
                assert_eq!(args.len(), 1);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Dimension("1em".to_string())
                );
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_integer_argument() {
    // "\romannumeral12"
    let (result, _) = parse(r"\romannumeral12", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "romannumeral");
                assert_eq!(args.len(), 1);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Integer("12".to_string())
                );
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_keyval_argument() {
    // "\includegraphics[width=1em,height=2pt]{file}"
    let (result, _) = parse(r"\includegraphics[width=1em,height=2pt]{file}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "includegraphics");
                assert_eq!(args.len(), 2);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Optional);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::KeyVal("width=1em,height=2pt".to_string())
                );
                assert_eq!(
                    unwrap_content(&args[1]),
                    &SyntaxNode::Text("file".to_string())
                );
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_delimiter_argument_braced_matches_inline() {
    let (inline, _) = parse(r"\delim\langle", false).unwrap();
    let (braced, _) = parse(r"\delim{\langle}", false).unwrap();

    let inline_value = match inline {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    let braced_value = match braced {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    assert_eq!(inline_value, braced_value);
}

#[test]
fn test_integer_argument_braced_matches_inline() {
    let (inline, _) = parse(r"\romannumeral12", false).unwrap();
    let (braced, _) = parse(r"\romannumeral{ 12 }", false).unwrap();

    let inline_value = match inline {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    let braced_value = match braced {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    assert_eq!(inline_value, braced_value);
}

#[test]
fn test_dimension_argument_braced_matches_inline() {
    let (inline, _) = parse(r"\hspace1.5em", false).unwrap();
    let (braced, _) = parse(r"\hspace{ 1,5 em }", false).unwrap();

    let inline_value = match inline {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    let braced_value = match braced {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    assert_eq!(inline_value, braced_value);
    assert_eq!(inline_value, ArgumentValue::Dimension("1.5em".to_string()));
}

#[test]
fn test_optional_bracket_closes_at_top_level() {
    let (result, _) = parse(r"\includegraphics[key={[[},width=1em]{file}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => {
                assert_eq!(args.len(), 2);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::KeyVal("key={[[},width=1em".to_string())
                );
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_optional_bracket_missing_closer_errors() {
    let result = parse(r"\includegraphics[width=1em{file}", false);
    assert!(result.is_err());
}

#[test]
fn test_invalid_integer_argument_errors() {
    let result = parse(r"\romannumeral{12.5}", false);
    assert!(result.is_err());
}

#[test]
fn test_invalid_dimension_argument_errors() {
    let result = parse(r"\hspace{abc}", false);
    assert!(result.is_err());
}

#[test]
fn test_unclosed_brace_argument_errors() {
    let result = parse(r"\frac{a", false);
    assert!(result.is_err());
}

#[test]
fn test_nested_commands() {
    // "\frac{a}{\sqrt{b}}"
    let (result, _) = parse(r"\frac{a}{\sqrt{b}}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");

                    // First argument should be normalized to single Char
                    assert_eq!(unwrap_content(&args[0]), &SyntaxNode::Char('a'));

                    // Second argument should be \sqrt command (normalized from single-element group)
                    match unwrap_content(&args[1]) {
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
    let (result, _) = parse(r"\unknown{x}", false).unwrap();

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
    let result = parse(r"\unknown{x}", true);
    assert!(result.is_err());
}

// ========================================================================
// Stage 4 Tests (Infix and Declarative commands)
// ========================================================================

#[test]
fn test_infix_over_simple() {
    // "a \over b"
    let (result, _) = match parse(r"a \over b", false) {
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
    let (result, _) = parse(r"n \choose k", false).unwrap();

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
    let (result, _) = parse(r"a+b \over c+d", false).unwrap();

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
    let (result, _) = parse(r"\bfseries text", false).unwrap();

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
    let (result, _) = parse(r"a \bfseries bc", false).unwrap();

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
    let (result, _) = parse(r"\bfseries", false).unwrap();

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

// TODO: Add test for text mode - currently parse_text_block is not exposed
// We can test text mode through \text{} command which uses Text mode args

#[test]
fn test_text_in_command() {
    // "\text{Hello World}" - text mode in command argument
    let (result, _) = parse(r"\text{Hello World}", false).unwrap();

    // Debug print the result
    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "text");
                    assert_eq!(args.len(), 1);
                    match unwrap_content(&args[0]) {
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
    let (result, _) = parse(r"\text{foo$a+b$bar}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                assert_eq!(args.len(), 1);
                match unwrap_content(&args[0]) {
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
    let errors = parse(r"\text{A~$x$B\frac{a}{b}}", false)
        .expect_err("frac is math-only and must fail in text mode");
    assert!(!errors.is_empty());
}

#[test]
fn test_command_mode_mismatch_reports_explicit_error() {
    let errors =
        parse(r"\text{\frac{a}{b}}", true).expect_err("expected strict mode mismatch error");
    assert!(!errors.is_empty());
}

#[test]
fn test_environment_mode_mismatch_reports_explicit_error() {
    let errors = parse(r"\text{\begin{matrix}a\end{matrix}}", true)
        .expect_err("expected strict mode mismatch error");
    assert!(!errors.is_empty());
}

#[test]
fn test_delimited_group_simple() {
    // "\left( a+b \right)"
    let (result, _) = parse(r"\left(a+b\right)", false).unwrap();

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
    let (result, _) = parse(r"\left.x\right|", false).unwrap();

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
    let (result, _) = parse(r"\left(\frac{a}{b}\right)", false).unwrap();

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
    let (result, _) = parse(r"\begin{matrix}ab\end{matrix}", false).unwrap();

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
    let (result, _) = parse(
        r"\begin{matrix}\begin{matrix}x\end{matrix}\end{matrix}",
        false,
    )
    .unwrap();

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
    let result = parse(r"\begin{matrix}a\end{align}", false);
    assert!(result.is_err());
}

#[test]
fn test_environment_missing_end_errors() {
    let errors = parse(r"\begin{matrix}", false).expect_err("expected missing end error");
    let debug = format!("{errors:?}");
    assert!(
        debug.contains("missing closing \\end{matrix}"),
        "Unexpected errors: {debug}"
    );
}

// ========================================================================
// Stage 6 Tests (Argument normalization and shorthand syntax)
// ========================================================================

#[test]
fn test_frac_shorthand_equivalence() {
    // "\frac ab" should produce the same AST as "\frac{a}{b}"
    let (result_short, _) = parse(r"\frac ab", false).unwrap();
    let (result_full, _) = parse(r"\frac{a}{b}", false).unwrap();

    assert_eq!(result_short, result_full);
}

#[test]
fn test_frac_mixed_shorthand() {
    // "\frac a{bc}" - one shorthand, one braced
    let (result, _) = parse(r"\frac a{bc}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");
                    assert_eq!(args.len(), 2);
                    // First arg: single char 'a'
                    assert_eq!(unwrap_content(&args[0]), &SyntaxNode::Char('a'));
                    // Second arg: group with 'bc'
                    match unwrap_content(&args[1]) {
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
    let (result, _) = parse(r"\frac\alpha\beta", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");
                    assert_eq!(args.len(), 2);
                    // Both args should be Command nodes
                    match unwrap_content(&args[0]) {
                        SyntaxNode::Command { name, .. } => assert_eq!(name, "alpha"),
                        _ => panic!("Expected alpha command"),
                    }
                    match unwrap_content(&args[1]) {
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
    let (result_short, _) = parse(r"\sqrt 2", false).unwrap();
    let (result_full, _) = parse(r"\sqrt{2}", false).unwrap();

    assert_eq!(result_short, result_full);
}

#[test]
fn test_sqrt_with_optional_shorthand() {
    // "\sqrt[3]8" vs "\sqrt[3]{8}"
    let (result_short, _) = parse(r"\sqrt[3]8", false).unwrap();
    let (result_full, _) = parse(r"\sqrt[3]{8}", false).unwrap();

    assert_eq!(result_short, result_full);
}

#[test]
fn test_optional_content_stops_at_first_closing_bracket() {
    let (result, _) = parse(r"\sqrt[a[b]c]{x}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 3);

            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);

                    match &expect_arg(&args[0]).value {
                        ArgumentValue::Content(SyntaxNode::Group { children, .. }) => {
                            assert_eq!(
                                children,
                                &vec![
                                    SyntaxNode::Char('a'),
                                    SyntaxNode::Char('['),
                                    SyntaxNode::Char('b'),
                                ]
                            );
                        }
                        other => panic!("Expected grouped optional content, got {:?}", other),
                    }

                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::Content(SyntaxNode::Char('c'))
                    );
                }
                other => panic!("Expected sqrt command, got {:?}", other),
            }

            assert_eq!(children[1], SyntaxNode::Char(']'));
            assert_eq!(
                children[2],
                SyntaxNode::Group {
                    mode: ContentMode::Math,
                    kind: GroupKind::Explicit,
                    children: vec![SyntaxNode::Char('x')],
                }
            );
        }
        other => panic!("Expected root Group, got {:?}", other),
    }
}

// ========================================================================
// Stage 7 Tests (Prime/apostrophe parsing)
// ========================================================================

#[test]
fn test_prime_single() {
    // "f'" -> Scripted with prime as superscript
    let (result, _) = parse(r"f'", false).unwrap();

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
    let (result, _) = parse(r"f'''", false).unwrap();

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
    let (result, _) = parse(r"f'^2", false).unwrap();

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
    let (result, _) = parse(r"f'_n", false).unwrap();

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
    let (result, _) = parse(r"x^{'_{a}}", false).unwrap();

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
                                    SyntaxNode::Group { .. } => {}
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
    let (result, _) = parse("^2", false).unwrap();

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
    let (result, _) = parse("_3", false).unwrap();

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
    let (result, _) = parse("'x", false).unwrap();

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
    assert!(parse(r"x^2^3", false).is_err());
}

#[test]
fn test_double_subscript_error() {
    assert!(parse(r"x_2_3", false).is_err());
}

#[test]
fn test_prime_after_superscript_error() {
    assert!(parse(r"x^a'", false).is_err());
}

#[test]
fn test_prime_brace_superscript_error() {
    // x'^' should fail because ^ expects a superscript atom, not a prime marker
    assert!(parse(r"x'^'", false).is_err());
}

#[test]
fn test_prime_on_prime_nested() {
    // "x^{'^{'}}" - prime on prime nesting
    let (result, _) = parse(r"x^{'^{'}}", false).unwrap();

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
    let (result, _) = parse(r"x^{a^{'}}", false).unwrap();

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
    let (result, _) = parse(r"x^{'^{a}}", false).unwrap();

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
    let (result, _) = parse(r"x'^a", false).unwrap();

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
    let (result, _) = parse(r"x''^a", false).unwrap();

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
    let (result, _) = parse(r"\left\langle x\right\rangle", false).unwrap();

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
fn test_delimiter_controls_are_interned() {
    fn extract_controls(node: SyntaxNode) -> (&'static str, &'static str) {
        match node {
            SyntaxNode::Group { children, .. } => match &children[0] {
                SyntaxNode::Group { kind, .. } => match kind {
                    GroupKind::Delimited { left, right } => {
                        let left = match left {
                            Delimiter::Control(s) => *s,
                            other => panic!("Expected left control delimiter, got {:?}", other),
                        };
                        let right = match right {
                            Delimiter::Control(s) => *s,
                            other => panic!("Expected right control delimiter, got {:?}", other),
                        };
                        (left, right)
                    }
                    other => panic!("Expected Delimited GroupKind, got {:?}", other),
                },
                other => panic!("Expected Delimited Group, got {:?}", other),
            },
            other => panic!("Expected root Group, got {:?}", other),
        }
    }

    let (left1, right1) = extract_controls(parse(r"\left\langle x\right\rangle", false).unwrap().0);
    let (left2, right2) = extract_controls(parse(r"\left\langle x\right\rangle", false).unwrap().0);

    assert!(std::ptr::eq(left1, left2));
    assert!(std::ptr::eq(right1, right2));
}

#[test]
fn test_delimited_group_lfloor_rfloor() {
    // "\left\lfloor x \right\rfloor"
    let (result, _) = parse(r"\left\lfloor x\right\rfloor", false).unwrap();

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
    let (result, _) = parse(r"\begin{align*}a+b\end{align*}", false).unwrap();

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
    let (result_spaces, _) = parse(r"\frac  {a}  {b}", false).unwrap();
    let (result_no_spaces, _) = parse(r"\frac{a}{b}", false).unwrap();

    assert_eq!(result_spaces, result_no_spaces);
}

#[test]
fn test_whitespace_ignored_in_scripts() {
    // "x ^ 2" should equal "x^2"
    let (result_spaces, _) = parse(r"x ^ 2", false).unwrap();
    let (result_no_spaces, _) = parse(r"x^2", false).unwrap();

    assert_eq!(result_spaces, result_no_spaces);
}

#[test]
fn test_whitespace_ignored_between_items() {
    // "a  b  c" should produce 3 Char nodes
    let (result, _) = parse(r"a  b  c", false).unwrap();

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
    let (result, _) = parse(r"{}", false).unwrap();

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
    let (result, _) = parse(r"{a}{b}", false).unwrap();

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
    let (result, _) = parse(r"\frac{x^2}{y}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");
                    // First arg should contain scripted x^2
                    match unwrap_content(&args[0]) {
                        SyntaxNode::Scripted {
                            base, superscript, ..
                        } => {
                            assert_eq!(**base, SyntaxNode::Char('x'));
                            assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('2'));
                        }
                        _ => panic!("Expected Scripted in first arg"),
                    }
                    // Second arg is just 'y'
                    assert_eq!(unwrap_content(&args[1]), &SyntaxNode::Char('y'));
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
    let (result, _) = parse(r"\alpha\beta\gamma", false).unwrap();

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
    let (result, _) = parse(r"a \over b \bfseries c", false).unwrap();

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
    let (result, _) = parse(r"a & b", false).unwrap();

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
    let (result, _) = parse(r"\text{\%\$\&}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                match unwrap_content(&args[0]) {
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
    let (result, _) = parse(r"\text{{a}}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                match unwrap_content(&args[0]) {
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

// ========================================================================
// Additional edge case tests for dimension and keyval arguments
// ========================================================================

#[test]
fn test_dimension_with_spaces() {
    // "\hspace{1.5 cm}" - dimension with spaces between number and unit
    let (result, _) = parse(r"\hspace{1.5 cm}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "hspace");
                assert_eq!(args.len(), 1);
                // Should be normalized to "1.5cm" (no space)
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Dimension("1.5cm".to_string())
                );
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_dimension_missing_number() {
    // "\hspace{cm}" - dimension missing number should error
    let result = parse(r"\hspace{cm}", false);
    assert!(
        result.is_err(),
        "Expected error for dimension missing number"
    );
}

#[test]
fn test_dimension_missing_unit() {
    // "\hspace{1.5}" - dimension missing unit should error
    let result = parse(r"\hspace{1.5}", false);
    assert!(result.is_err(), "Expected error for dimension missing unit");
}

#[test]
fn test_keyval_empty() {
    // "\includegraphics[{}]{file}" - empty braces in optional keyval argument
    // This should error because keyval requires at least one key=value pair
    let result = parse(r"\includegraphics[{}]{file}", false);
    assert!(result.is_err(), "Expected error for empty keyval");
}

#[test]
fn test_keyval_empty_brackets() {
    // "\includegraphics[]{file}" - empty optional argument brackets
    // This should also error because the brackets exist but contain no valid keyval
    let result = parse(r"\includegraphics[]{file}", false);
    assert!(
        result.is_err(),
        "Expected error for empty optional keyval brackets"
    );
}

// ========================================================================
// XParse-inspired ArgSpec Tests
// ========================================================================

fn extract_first_command(node: SyntaxNode) -> (String, bool, Vec<Option<Argument>>) {
    match node {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command {
                name,
                starred,
                args,
            } => (name.clone(), *starred, args.clone()),
            other => panic!("Expected command node, got {:?}", other),
        },
        other => panic!("Expected root group, got {:?}", other),
    }
}

#[test]
fn test_qty_supports_multiple_delimiter_pairs() {
    let cases = [
        (r"\qty(x)", Delimiter::Char('('), Delimiter::Char(')')),
        (r"\qty[x]", Delimiter::Char('['), Delimiter::Char(']')),
        (r"\qty{x}", Delimiter::Char('{'), Delimiter::Char('}')),
        (r"\qty|x|", Delimiter::Char('|'), Delimiter::Char('|')),
    ];

    for (src, open, close) in cases {
        let (result, _) = parse(src, false).unwrap();
        let (name, starred, args) = extract_first_command(result);
        assert_eq!(name, "qty");
        assert!(!starred);
        assert_eq!(args.len(), 1);

        let arg = expect_arg(&args[0]);
        assert_eq!(arg.value, ArgumentValue::Content(SyntaxNode::Char('x')));
        match arg.kind {
            ArgumentKind::Paired {
                open: matched_open,
                close: matched_close,
            } => {
                assert_eq!(matched_open, open);
                assert_eq!(matched_close, close);
            }
            other => panic!("Expected paired argument kind, got {:?}", other),
        }
    }
}

#[test]
fn test_qty_optional_slot_can_be_missing() {
    let (result, _) = parse(r"\qty", false).unwrap();
    let (name, starred, args) = extract_first_command(result);
    assert_eq!(name, "qty");
    assert!(!starred);
    assert_eq!(args.len(), 1);
    assert!(args[0].is_none());
}

#[test]
fn test_arg_true_quantity_commands_require_braces() {
    let (pqty_ok, _) = parse(r"\pqty{x}", false).unwrap();
    let (name, starred, args) = extract_first_command(pqty_ok);
    assert_eq!(name, "pqty");
    assert!(!starred);
    assert_eq!(args.len(), 2);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));

    let content = expect_arg(&args[1]);
    assert_eq!(content.value, ArgumentValue::Content(SyntaxNode::Char('x')));
    match content.kind {
        ArgumentKind::Delimited { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("Expected brace-delimited argument, got {:?}", other),
    }

    let (abs_ok, _) = parse(r"\abs{x}", false).unwrap();
    let (name, _, args) = extract_first_command(abs_ok);
    assert_eq!(name, "abs");
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::Content(SyntaxNode::Char('x'))
    );

    assert!(parse(r"\pqty(x)", false).is_err());
    assert!(parse(r"\abs|x|", false).is_err());
}

#[test]
fn test_eval_uses_nonsymmetric_paired_delimiter() {
    let (result, _) = parse(r"\eval(x|", false).unwrap();
    let (name, starred, args) = extract_first_command(result);
    assert_eq!(name, "eval");
    assert!(!starred);
    assert_eq!(args.len(), 2);

    let star = expect_arg(&args[0]);
    assert_eq!(star.kind, ArgumentKind::Star);
    assert_eq!(star.value, ArgumentValue::Boolean(false));

    let paired = expect_arg(&args[1]);
    assert_eq!(paired.value, ArgumentValue::Content(SyntaxNode::Char('x')));
    match paired.kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('('));
            assert_eq!(close, Delimiter::Char('|'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }
}

#[test]
fn test_dv_and_pdv_group_slots_are_stable() {
    let (dv_result, _) = parse(r"\dv{f}", false).unwrap();
    let (name, starred, args) = extract_first_command(dv_result);
    assert_eq!(name, "dv");
    assert!(!starred);
    assert_eq!(args.len(), 4);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Boolean(false),
        "star slot should always exist",
    );
    assert!(args[1].is_none(), "optional bracket slot should be None");
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::Content(SyntaxNode::Char('f'))
    );
    assert!(args[3].is_none(), "group slot should be None when absent");

    let (pdv_result, _) = parse(r"\pdv{f}{x}{y}", false).unwrap();
    let (name, starred, args) = extract_first_command(pdv_result);
    assert_eq!(name, "pdv");
    assert!(!starred);
    assert_eq!(args.len(), 5);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
    assert!(args[1].is_none());
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::Content(SyntaxNode::Char('f'))
    );
    assert_eq!(expect_arg(&args[3]).kind, ArgumentKind::Group);
    assert_eq!(expect_arg(&args[4]).kind, ArgumentKind::Group);
}

#[test]
fn test_braket_optional_group_slot() {
    let (result_full, _) = parse(r"\braket{a}{b}", false).unwrap();
    let (_, _, args_full) = extract_first_command(result_full);
    assert_eq!(args_full.len(), 3);
    assert_eq!(
        expect_arg(&args_full[0]).value,
        ArgumentValue::Boolean(false)
    );
    assert_eq!(
        expect_arg(&args_full[1]).value,
        ArgumentValue::Content(SyntaxNode::Char('a'))
    );
    assert_eq!(expect_arg(&args_full[2]).kind, ArgumentKind::Group);

    let (result_short, _) = parse(r"\braket{a}", false).unwrap();
    let (_, _, args_short) = extract_first_command(result_short);
    assert_eq!(args_short.len(), 3);
    assert!(args_short[2].is_none());
}

#[test]
fn test_exp_does_not_consume_star_without_s_slot() {
    let (result, _) = parse(r"\exp*", false).unwrap();
    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Command {
                    name,
                    starred,
                    args,
                } => {
                    assert_eq!(name, "exp");
                    assert!(!starred);
                    assert!(args.is_empty());
                }
                other => panic!("Expected exp command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('*'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_exp_does_not_consume_brackets_without_optional_slot() {
    let (result, _) = parse(r"\exp[x]", false).unwrap();
    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command {
                    name,
                    starred,
                    args,
                } => {
                    assert_eq!(name, "exp");
                    assert!(!starred);
                    assert!(args.is_empty());
                }
                other => panic!("Expected exp command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('['));
            assert_eq!(children[2], SyntaxNode::Char('x'));
            assert_eq!(children[3], SyntaxNode::Char(']'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_bare_brackets_parse_as_regular_characters() {
    let (result, _) = parse("[a]", false).unwrap();
    match result {
        SyntaxNode::Group { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('['));
            assert_eq!(children[1], SyntaxNode::Char('a'));
            assert_eq!(children[2], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root group, got {:?}", other),
    }
}

#[test]
fn test_no_leading_space_prefix_for_linebreak_command() {
    let (immediate, _) = parse(r"\\*[1cm]", false).unwrap();
    let (name, starred, args) = extract_first_command(immediate);
    assert_eq!(name, "\\");
    assert!(starred);
    assert_eq!(args.len(), 2);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::Dimension("1cm".to_string())
    );

    let (spaced_star, _) = parse(r"\\ *", false).unwrap();
    match spaced_star {
        SyntaxNode::Group { children, .. } => {
            assert!(!children.is_empty());
            match &children[0] {
                SyntaxNode::Command {
                    name,
                    starred,
                    args,
                } => {
                    assert_eq!(name, "\\");
                    assert!(!starred);
                    assert_eq!(args.len(), 2);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
                    assert!(args[1].is_none());
                }
                other => panic!("Expected linebreak command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('*'));
        }
        _ => panic!("Expected root group"),
    }

    let (spaced_dimension, _) = parse(r"\\ [1cm]", false).unwrap();
    match spaced_dimension {
        SyntaxNode::Group { children, .. } => {
            assert!(!children.is_empty());
            match &children[0] {
                SyntaxNode::Command {
                    name,
                    starred,
                    args,
                } => {
                    assert_eq!(name, "\\");
                    assert!(!starred);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
                    assert!(args[1].is_none());
                }
                other => panic!("Expected linebreak command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('['));
            assert_eq!(children[2], SyntaxNode::Char('1'));
            assert_eq!(children[3], SyntaxNode::Char('c'));
            assert_eq!(children[4], SyntaxNode::Char('m'));
            assert_eq!(children[5], SyntaxNode::Char(']'));
        }
        _ => panic!("Expected root group"),
    }
}

// ========================================================================
// Span Correctness Tests
// ========================================================================

#[test]
fn test_span_covers_full_input() {
    let src = r"\frac{a}{b}";
    let (_, span) = parse(src, false).unwrap();
    assert_eq!(span.start, 0);
    assert_eq!(span.end, src.len());
}

#[test]
fn test_span_empty_input() {
    let src = "";
    let (_, span) = parse(src, false).unwrap();
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 0);
}

#[test]
fn test_span_simple_chars() {
    let src = "abc";
    let (_, span) = parse(src, false).unwrap();
    assert_eq!(&src[span.start..span.end], "abc");
}

#[test]
fn test_span_command_with_args() {
    let src = r"\sqrt[3]{x}";
    let (_, span) = parse(src, false).unwrap();
    assert_eq!(&src[span.start..span.end], src);
}

#[test]
fn test_span_with_whitespace() {
    let src = r" a + b ";
    let (_, span) = parse(src, false).unwrap();
    // Span should cover the full input range
    assert_eq!(span.start, 0);
    assert_eq!(span.end, src.len());
}

#[test]
fn test_span_environment() {
    let src = r"\begin{matrix}x\end{matrix}";
    let (_, span) = parse(src, false).unwrap();
    assert_eq!(&src[span.start..span.end], src);
}

#[test]
fn test_span_scripted() {
    let src = "x^{2}_{i}";
    let (_, span) = parse(src, false).unwrap();
    assert_eq!(&src[span.start..span.end], src);
}
