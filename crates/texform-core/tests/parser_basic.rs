mod support;

use support::parser::*;
use texform_core::parse::{AllowedMode, CommandKind, ParseConfig, ParseContext};
use texform_interface::syntax_node::{
    ArgumentKind, ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};

#[test]
fn prime_then_explicit_superscript_stays_in_single_superscript_slot() {
    let parsed = parse("x'^2", false).expect("expected parse success").0;

    match parsed {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Scripted {
                base,
                subscript,
                superscript,
            } => {
                assert_eq!(**base, SyntaxNode::Char('x'));
                assert!(subscript.is_none());
                match superscript.as_deref().expect("expected superscript") {
                    SyntaxNode::Group {
                        kind: GroupKind::Implicit,
                        children,
                        ..
                    } => {
                        assert_eq!(
                            children,
                            &vec![SyntaxNode::Prime { count: 1 }, SyntaxNode::Char('2')]
                        );
                    }
                    other => panic!("expected implicit superscript group, got {:?}", other),
                }
            }
            other => panic!("expected scripted node, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

#[test]
fn test_text_argument_uses_text_content_variant_for_single_char_item() {
    let output = ParseContext::shared().parse(r"\text{\%}", &ParseConfig::STRICT);
    let result = output.try_into_document().expect("expected parse result").0;

    match result.to_syntax() {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => {
                let arg = args[0].as_ref().expect("expected text argument");
                assert_eq!(arg.value, ArgumentValue::TextContent(SyntaxNode::Char('%')));
            }
            other => panic!("expected text command, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

// ========================================================================
// Stage 1-2 Tests (Basic parsing)
// ========================================================================

#[test]
fn test_parse_simple_chars() {
    let (result, _) = parse("abc", false).unwrap();

    match result {
        SyntaxNode::Root { mode, children } => {
            assert_eq!(mode, ContentMode::Math);
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('a'));
            assert_eq!(children[1], SyntaxNode::Char('b'));
            assert_eq!(children[2], SyntaxNode::Char('c'));
        }
        _ => panic!("Expected Root node"),
    }
}

#[test]
fn test_parse_empty() {
    let (result, _) = parse("", false).unwrap();

    match result {
        SyntaxNode::Root { mode, children } => {
            assert_eq!(mode, ContentMode::Math);
            assert!(children.is_empty());
        }
        _ => panic!("Expected Root node"),
    }
}

#[test]
fn test_escaped_symbols() {
    let (result, _) = parse(r"\%\$\&", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('%'));
            assert_eq!(children[1], SyntaxNode::Char('$'));
            assert_eq!(children[2], SyntaxNode::Char('&'));
        }
        _ => panic!("Expected Root"),
    }
}

#[test]
fn test_active_char() {
    let (result, _) = parse("a~b", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('a'));
            assert_eq!(children[1], SyntaxNode::ActiveSpace);
            assert_eq!(children[2], SyntaxNode::Char('b'));
        }
        _ => panic!("Expected Root"),
    }
}

#[test]
fn test_explicit_group() {
    let (result, _) = parse("{a}", false).unwrap();

    match result {
        SyntaxNode::Root { mode, children } => {
            assert_eq!(mode, ContentMode::Math);
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
        _ => panic!("Expected Root node"),
    }
}

#[test]
fn test_nested_groups() {
    let (result, _) = parse("a{b{c}}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
        _ => panic!("Expected Root node"),
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        _ => panic!("Expected Root node"),
    }
}

#[test]
fn test_frac_command() {
    // "\frac{a}{b}"
    let (result, _) = parse(r"\frac{a}{b}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "frac");
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);

                    // Optional arg - normalized to single Char
                    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Optional);
                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('3'))
                    );

                    // Mandatory arg - normalized to single Char
                    assert_eq!(expect_arg(&args[1]).kind, ArgumentKind::Mandatory);
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('8'))
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
fn test_bare_delimiter_control_in_math_item() {
    let (result, _) = parse(r"\langle", true).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command {
                name, args, known, ..
            } => {
                assert_eq!(name, "langle");
                assert!(args.is_empty());
                assert!(*known);
            }
            other => panic!("Expected Command node, got {:?}", other),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_bare_delimiter_controls_inside_math_expression() {
    let (result, _) = parse(r"a \langle b \rangle", true).unwrap();

    let children = match result {
        SyntaxNode::Root { children, .. } => children,
        _ => panic!("Expected root Group"),
    };

    assert!(matches!(children[0], SyntaxNode::Char('a')));
    match &children[1] {
        SyntaxNode::Command {
            name, args, known, ..
        } => {
            assert_eq!(name, "langle");
            assert!(args.is_empty());
            assert!(*known);
        }
        other => panic!("Expected \\langle command, got {:?}", other),
    }
    assert!(matches!(children[2], SyntaxNode::Char('b')));
    match &children[3] {
        SyntaxNode::Command {
            name, args, known, ..
        } => {
            assert_eq!(name, "rangle");
            assert!(args.is_empty());
            assert!(*known);
        }
        other => panic!("Expected \\rangle command, got {:?}", other),
    }
}

#[test]
fn test_bare_delimiter_control_remains_unknown_in_text_content() {
    let errors = parse(r"\text{\langle}", true).unwrap_err();
    assert_eq!(errors, vec![r"Unknown command: \langle".to_string()]);
}

#[test]
fn test_nested_commands() {
    // "\frac{a}{\sqrt{b}}"
    let (result, _) = parse(r"\frac{a}{\sqrt{b}}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 2);

            // First: command node with known=false
            match &children[0] {
                SyntaxNode::Command { name, args, known } => {
                    assert_eq!(name, "unknown");
                    assert!(args.is_empty());
                    assert!(!known);
                }
                _ => panic!("Expected unknown Command node"),
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
fn test_unknown_environment_nonstrict() {
    let (result, _) = parse(r"\begin{foo}a\end{foo}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Environment {
                name,
                args,
                known,
                body,
            } => {
                assert_eq!(name, "foo");
                assert!(args.is_empty());
                assert!(!known);
                assert!(matches!(**body, SyntaxNode::Group { .. }));
            }
            other => panic!("Expected Environment node, got {:?}", other),
        },
        other => panic!("Expected root Group, got {:?}", other),
    }
}

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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Infix {
                    name,
                    left,
                    right,
                    args,
                } => {
                    assert_eq!(name, "over");
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
fn test_infix_over_allows_empty_left_operand() {
    let (result, _) = parse(r"\over x", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Infix { left, right, .. } => {
                assert_eq!(
                    **left,
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![],
                    }
                );
                assert_eq!(**right, SyntaxNode::Char('x'));
            }
            other => panic!("Expected infix node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_infix_over_allows_empty_right_operand() {
    let (result, _) = parse(r"x \over", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Infix { left, right, .. } => {
                assert_eq!(**left, SyntaxNode::Char('x'));
                assert_eq!(
                    **right,
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![],
                    }
                );
            }
            other => panic!("Expected infix node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_repeated_buildrel_over_parses_as_separate_infixes() {
    let ctx = ParseContext::from_packages(&["base"]);
    let src = r"\cdots\to K\buildrel f\over\longrightarrow K\buildrel f\over\longrightarrow K";

    for config in [ParseConfig::STRICT, ParseConfig::LENIENT] {
        let output = ctx.parse(src, &config);
        assert!(
            output.diagnostics.is_empty(),
            "diagnostics for {:?}: {:?}",
            config,
            output.diagnostics
        );
        assert!(
            output.document().is_some(),
            "expected parse result for {:?}",
            config
        );
    }
}

#[test]
fn test_infix_over_allows_declarative_before_command() {
    let output = test_context_with_items([command_item(
        "displaystyle",
        CommandKind::Declarative,
        AllowedMode::Math,
        "",
    )])
    .parse(r"\displaystyle \over x", &ParseConfig::LENIENT);

    assert!(
        output.diagnostics.is_empty(),
        "expected declarative-before-infix parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .try_into_document()
        .expect("parse without diagnostics should produce a result")
        .0
        .to_syntax();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Infix { left, right, .. } => {
                assert_eq!(
                    **left,
                    SyntaxNode::Declarative {
                        name: "displaystyle".to_string(),
                        args: vec![],
                    }
                );
                assert_eq!(**right, SyntaxNode::Char('x'));
            }
            other => panic!("Expected infix node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_grouped_nested_over_is_not_ambiguous() {
    let (result, _) = parse(r"{a \over b} \over c", true).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Infix { left, right, .. } => {
                assert!(matches!(**right, SyntaxNode::Char('c')));
                match &**left {
                    SyntaxNode::Group { kind, children, .. } => {
                        assert_eq!(*kind, GroupKind::Explicit);
                        assert_eq!(children.len(), 1);
                        assert!(matches!(children[0], SyntaxNode::Infix { .. }));
                    }
                    other => panic!("Expected explicit left group, got {:?}", other),
                }
            }
            other => panic!("Expected outer infix node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_right_grouped_nested_over_is_not_ambiguous() {
    let (result, _) = parse(r"a \over {b + 1 \over c}", true).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Infix { left, right, .. } => {
                assert!(matches!(**left, SyntaxNode::Char('a')));
                match &**right {
                    SyntaxNode::Group { kind, children, .. } => {
                        assert_eq!(*kind, GroupKind::Explicit);
                        assert_eq!(children.len(), 1);
                        assert!(matches!(children[0], SyntaxNode::Infix { .. }));
                    }
                    other => panic!("Expected explicit right group, got {:?}", other),
                }
            }
            other => panic!("Expected outer infix node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_declarative_bfseries_is_flat() {
    let (result, _) = parse(r"\bfseries text", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 5);
            assert_eq!(
                children[0],
                SyntaxNode::Declarative {
                    name: "bfseries".to_string(),
                    args: vec![],
                }
            );
            assert_eq!(children[1], SyntaxNode::Char('t'));
            assert_eq!(children[2], SyntaxNode::Char('e'));
            assert_eq!(children[3], SyntaxNode::Char('x'));
            assert_eq!(children[4], SyntaxNode::Char('t'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_declarative_with_leading_is_flat() {
    let (result, _) = parse(r"a \bfseries bc", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            assert_eq!(children[0], SyntaxNode::Char('a'));
            assert_eq!(
                children[1],
                SyntaxNode::Declarative {
                    name: "bfseries".to_string(),
                    args: vec![],
                }
            );
            assert_eq!(children[2], SyntaxNode::Char('b'));
            assert_eq!(children[3], SyntaxNode::Char('c'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_declarative_empty_scope() {
    let (result, _) = parse(r"\bfseries", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            assert_eq!(
                children[0],
                SyntaxNode::Declarative {
                    name: "bfseries".to_string(),
                    args: vec![],
                }
            );
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_declarative_nested_textstyle() {
    let ctx = test_context_with_items([
        command_item("textstyle", CommandKind::Declarative, AllowedMode::Math, ""),
        command_item("sum", CommandKind::Prefix, AllowedMode::Math, ""),
    ]);
    let output = ctx.parse(
        r"\textstyle f(x) = \textstyle \sum_{i=0}^{n}",
        &ParseConfig::LENIENT,
    );
    assert!(
        output.diagnostics.is_empty(),
        "Expected nested textstyle parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .try_into_document()
        .expect("parse without diagnostics should produce a result")
        .0
        .to_syntax();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 8);
            assert_eq!(
                children[0],
                SyntaxNode::Declarative {
                    name: "textstyle".to_string(),
                    args: vec![],
                }
            );
            assert_eq!(children[1], SyntaxNode::Char('f'));
            assert_eq!(children[2], SyntaxNode::Char('('));
            assert_eq!(children[3], SyntaxNode::Char('x'));
            assert_eq!(children[4], SyntaxNode::Char(')'));
            assert_eq!(children[5], SyntaxNode::Char('='));
            assert_eq!(
                children[6],
                SyntaxNode::Declarative {
                    name: "textstyle".to_string(),
                    args: vec![],
                }
            );
            assert!(matches!(children[7], SyntaxNode::Scripted { .. }));
        }
        other => panic!("Expected root Group, got {:?}", other),
    }
}

// ========================================================================
// Stage 5 Tests (Text mode, inline math, delimited groups, environments)
// ========================================================================

// TODO: Add test for text mode - currently parse_text_block is not exposed
// We can test text mode through \text{} command which uses Text mode args

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
fn test_text_mode_scripted_syntax_reports_explicit_error() {
    let ctx = test_context_with_items([
        command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m"),
        command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T"),
    ]);

    let errors = ctx
        .parse(r"\text{\underline{a^2}}", &ParseConfig::STRICT)
        .diagnostics
        .into_iter()
        .map(|diag| diag.message)
        .collect::<Vec<_>>();

    assert!(
        !errors.is_empty(),
        "scripted syntax should fail in text mode"
    );
    assert_eq!(errors, vec!["Scripted syntax is not allowed in Text mode"]);
}

#[test]
fn test_text_mode_declarative_is_flat() {
    let ctx = test_context_with_items([command_item(
        "tiny",
        CommandKind::Declarative,
        AllowedMode::Text,
        "",
    )]);

    let output = ctx.parse(r"\text{\tiny{FP}}", &ParseConfig::LENIENT);
    assert!(
        output.diagnostics.is_empty(),
        "expected flat text declarative parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .try_into_document()
        .expect("parse without diagnostics should produce a result")
        .0
        .to_syntax();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                match unwrap_content(&args[0]) {
                    SyntaxNode::Group { mode, children, .. } => {
                        assert_eq!(*mode, ContentMode::Text);
                        assert_eq!(children.len(), 2);
                        assert_eq!(
                            children[0],
                            SyntaxNode::Declarative {
                                name: "tiny".to_string(),
                                args: vec![],
                            }
                        );
                        match &children[1] {
                            SyntaxNode::Group {
                                mode,
                                kind,
                                children,
                            } => {
                                assert_eq!(*mode, ContentMode::Text);
                                assert_eq!(*kind, GroupKind::Explicit);
                                assert_eq!(children.len(), 1);
                                assert_eq!(children[0], SyntaxNode::Text("FP".to_string()));
                            }
                            other => panic!(
                                "Expected explicit text group after declarative, got {:?}",
                                other
                            ),
                        }
                    }
                    other => panic!("Expected text group argument, got {:?}", other),
                }
            }
            other => panic!("Expected text command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_delimited_group_simple() {
    // "\left( a+b \right)"
    let (result, _) = parse(r"\left(a+b\right)", false).unwrap();

    eprintln!("Parsed result:\n{:#?}", result);

    match result {
        SyntaxNode::Root { ref children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
fn test_delimited_group_accepts_space_after_left_and_right() {
    let (result, _) = parse(r"\left (a+b\right )", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Group { kind, .. } => match kind {
                GroupKind::Delimited { left, right } => {
                    assert_eq!(*left, Delimiter::Char('('));
                    assert_eq!(*right, Delimiter::Char(')'));
                }
                other => panic!("Expected delimited group, got {:?}", other),
            },
            other => panic!("Expected inner group, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_delimited_group_square_brackets() {
    let (result, _) = parse(r"\left[a+b\right]", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Group { kind, children, .. } => match kind {
                GroupKind::Delimited { left, right } => {
                    assert_eq!(*left, Delimiter::Char('['));
                    assert_eq!(*right, Delimiter::Char(']'));
                    assert_eq!(children.len(), 3);
                    assert_eq!(children[0], SyntaxNode::Char('a'));
                    assert_eq!(children[1], SyntaxNode::Char('+'));
                    assert_eq!(children[2], SyntaxNode::Char('b'));
                }
                other => panic!("Expected delimited group, got {:?}", other),
            },
            other => panic!("Expected inner group, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_environment_basic() {
    let (result, _) = parse(r"\begin{matrix}ab\end{matrix}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Environment {
                    name, args, body, ..
                } => {
                    assert_eq!(name, "matrix");
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
fn test_environment_header_accepts_space_after_begin_and_end() {
    let (result, _) = parse(r"\begin {matrix}a&b\\c&d\end {matrix}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Environment { name, body, .. } => {
                assert_eq!(name, "matrix");
                match &**body {
                    SyntaxNode::Group { children, .. } => {
                        assert_eq!(children.len(), 7);
                    }
                    other => panic!("Expected environment body group, got {:?}", other),
                }
            }
            other => panic!("Expected environment node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 3);

            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);

                    match &expect_arg(&args[0]).value {
                        ArgumentValue::MathContent(SyntaxNode::Group { children, .. }) => {
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
                        ArgumentValue::MathContent(SyntaxNode::Char('c'))
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    superscript,
                    subscript,
                } => {
                    assert_eq!(**base, SyntaxNode::Char('f'));
                    assert!(subscript.is_none());
                    assert_eq!(
                        **superscript.as_ref().unwrap(),
                        SyntaxNode::Prime { count: 1 }
                    );
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base, superscript, ..
                } => {
                    assert_eq!(**base, SyntaxNode::Char('f'));
                    assert_eq!(
                        **superscript.as_ref().unwrap(),
                        SyntaxNode::Prime { count: 3 }
                    );
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
        SyntaxNode::Root { children, .. } => {
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
                            assert_eq!(children[0], SyntaxNode::Prime { count: 1 });
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    superscript,
                    subscript,
                } => {
                    assert_eq!(**base, SyntaxNode::Char('f'));
                    assert_eq!(
                        **superscript.as_ref().unwrap(),
                        SyntaxNode::Prime { count: 1 }
                    );
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    match superscript.as_ref().unwrap().as_ref() {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 1);
                            match &children[0] {
                                SyntaxNode::Scripted {
                                    superscript,
                                    subscript,
                                    base,
                                } => {
                                    assert!(subscript.is_some());
                                    assert!(superscript.is_none());
                                    assert_eq!(**base, SyntaxNode::Prime { count: 1 });
                                }
                                other => panic!(
                                    "Expected scripted prime inside superscript, got {:?}",
                                    other
                                ),
                            }
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
fn test_braced_prime_superscript_content() {
    let (result, _) = parse(r"x ^ { ' }", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted {
                    base,
                    superscript,
                    subscript,
                } => {
                    assert_eq!(**base, SyntaxNode::Char('x'));
                    assert!(subscript.is_none());
                    assert_eq!(
                        superscript.as_deref(),
                        Some(&SyntaxNode::Group {
                            mode: ContentMode::Math,
                            kind: GroupKind::Explicit,
                            children: vec![SyntaxNode::Prime { count: 1 }],
                        })
                    );
                }
                other => panic!("Expected Scripted node, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_command_prime_superscript_still_parses() {
    let (result, _) = parse(r"f^{\prime}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    assert!(matches!(
                        superscript.as_deref(),
                        Some(SyntaxNode::Group {
                            mode: ContentMode::Math,
                            kind: GroupKind::Explicit,
                            children,
                        }) if matches!(children.as_slice(), [SyntaxNode::Command { name, .. }] if name == "prime")
                    ));
                }
                other => panic!("Expected Scripted node, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_empty_base_superscript() {
    // "^2" -> empty base with superscript
    let (result, _) = parse("^2", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
    // "'x" -> leading prime atom then x
    let (result, _) = parse("'x", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 2);
            assert_eq!(children[0], SyntaxNode::Prime { count: 1 });
            assert_eq!(children[1], SyntaxNode::Char('x'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_prime_nested_shapes() {
    let cases = [
        (
            r"x^{'^{'}}",
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Explicit,
                children: vec![SyntaxNode::Scripted {
                    base: Box::new(SyntaxNode::Prime { count: 1 }),
                    subscript: None,
                    superscript: Some(Box::new(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Explicit,
                        children: vec![SyntaxNode::Prime { count: 1 }],
                    })),
                }],
            },
        ),
        (
            r"x^{a^{'}}",
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Explicit,
                children: vec![SyntaxNode::Scripted {
                    base: Box::new(SyntaxNode::Char('a')),
                    subscript: None,
                    superscript: Some(Box::new(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Explicit,
                        children: vec![SyntaxNode::Prime { count: 1 }],
                    })),
                }],
            },
        ),
        (
            r"x^{'^{a}}",
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Explicit,
                children: vec![SyntaxNode::Scripted {
                    base: Box::new(SyntaxNode::Prime { count: 1 }),
                    subscript: None,
                    superscript: Some(Box::new(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Explicit,
                        children: vec![SyntaxNode::Char('a')],
                    })),
                }],
            },
        ),
    ];

    for (src, expected_superscript) in cases {
        let (result, _) = parse(src, false).unwrap();

        match result {
            SyntaxNode::Root { children, .. } => {
                assert_eq!(children.len(), 1, "src={src}");
                match &children[0] {
                    SyntaxNode::Scripted { superscript, .. } => {
                        assert_eq!(
                            superscript.as_deref(),
                            Some(&expected_superscript),
                            "src={src}"
                        );
                    }
                    other => panic!("expected scripted node for {src}, got {:?}", other),
                }
            }
            other => panic!("expected root node for {src}, got {:?}", other),
        }
    }
}

#[test]
fn test_prime_then_superscript_merge() {
    let (result, _) = parse(r"x'^a", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    match superscript.as_ref().unwrap().as_ref() {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            assert_eq!(children[0], SyntaxNode::Prime { count: 1 });
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Scripted { superscript, .. } => {
                    match superscript.as_ref().unwrap().as_ref() {
                        SyntaxNode::Group { children, .. } => {
                            assert_eq!(children.len(), 2);
                            assert_eq!(children[0], SyntaxNode::Prime { count: 2 });
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
        SyntaxNode::Root { children, .. } => {
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
    let (result, _) = parse(r"\left\lfloor x\right\rfloor", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Environment { name, body, .. } => {
                    assert_eq!(name, "align*");
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

#[test]
fn test_starred_environment_name_rejects_internal_space() {
    let result = parse(r"\begin{align *}a+b\end{align*}", true);
    assert!(result.is_err(), "internal space before * must be rejected");
}

// ========================================================================
// Stage 10 Tests (Whitespace handling)
// ========================================================================

#[test]
fn test_whitespace_is_ignored_in_structural_positions() {
    assert_same_structure(r"\frac  {a}  {b}", r"\frac{a}{b}");
    assert_same_structure(r"x ^ 2", r"x^2");
}

#[test]
fn test_whitespace_ignored_between_items() {
    // "a  b  c" should produce 3 Char nodes
    let (result, _) = parse(r"a  b  c", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
    // "a \over b \bfseries c" - declarative remains part of the denominator
    let (result, _) = parse(r"a \over b \bfseries c", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Infix {
                    name, left, right, ..
                } => {
                    assert_eq!(name, "over");
                    assert_eq!(**left, SyntaxNode::Char('a'));

                    match &**right {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert_eq!(children.len(), 3);
                            assert_eq!(children[0], SyntaxNode::Char('b'));
                            assert_eq!(
                                children[1],
                                SyntaxNode::Declarative {
                                    name: "bfseries".to_string(),
                                    args: vec![],
                                }
                            );
                            assert_eq!(children[2], SyntaxNode::Char('c'));
                        }
                        other => panic!("Expected grouped denominator, got {:?}", other),
                    }
                }
                other => panic!("Expected Infix node, got {:?}", other),
            }
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_infix_over_with_declarative_right_operand() {
    let ctx = test_context_with_items([command_item(
        "displaystyle",
        CommandKind::Declarative,
        AllowedMode::Math,
        "",
    )]);
    let output = ctx.parse(r"a \over \displaystyle b", &ParseConfig::LENIENT);
    assert!(
        output.diagnostics.is_empty(),
        "Expected declarative denominator parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .try_into_document()
        .expect("parse without diagnostics should produce a result")
        .0
        .to_syntax();

    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);

            match &children[0] {
                SyntaxNode::Infix {
                    name,
                    args,
                    left,
                    right,
                } => {
                    assert_eq!(name, "over");
                    assert!(args.is_empty());
                    assert_eq!(**left, SyntaxNode::Char('a'));

                    match &**right {
                        SyntaxNode::Group { children, kind, .. } => {
                            assert_eq!(*kind, GroupKind::Implicit);
                            assert_eq!(children.len(), 2);
                            assert_eq!(
                                children[0],
                                SyntaxNode::Declarative {
                                    name: "displaystyle".to_string(),
                                    args: vec![],
                                }
                            );
                            assert_eq!(children[1], SyntaxNode::Char('b'));
                        }
                        other => panic!(
                            "Expected grouped declarative denominator for infix over, got {:?}",
                            other
                        ),
                    }
                }
                other => panic!("Expected Infix node, got {:?}", other),
            }
        }
        other => panic!("Expected root Group, got {:?}", other),
    }
}

#[test]
fn test_infix_left_collects_flat_declarative_items() {
    let output = test_context_with_items([command_item(
        "displaystyle",
        CommandKind::Declarative,
        AllowedMode::Math,
        "",
    )])
    .parse(r"a \displaystyle b \over c", &ParseConfig::LENIENT);

    assert!(
        output.diagnostics.is_empty(),
        "expected flat declarative infix parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .try_into_document()
        .expect("parse without diagnostics should produce a result")
        .0
        .to_syntax();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Infix { left, right, .. } => {
                assert!(matches!(**right, SyntaxNode::Char('c')));
                match &**left {
                    SyntaxNode::Group { children, kind, .. } => {
                        assert_eq!(*kind, GroupKind::Implicit);
                        assert_eq!(children.len(), 3);
                        assert_eq!(children[0], SyntaxNode::Char('a'));
                        assert_eq!(
                            children[1],
                            SyntaxNode::Declarative {
                                name: "displaystyle".to_string(),
                                args: vec![],
                            }
                        );
                        assert_eq!(children[2], SyntaxNode::Char('b'));
                    }
                    other => panic!("Expected implicit left group, got {:?}", other),
                }
            }
            other => panic!("Expected infix node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_alignment_char() {
    // "&" should parse as Char('&') in math mode
    let (result, _) = parse(r"a & b", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
fn test_text_escaped_braces_non_regression() {
    let (result, _) = parse(r"\text{\{a\}}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                match unwrap_content(&args[0]) {
                    SyntaxNode::Group { children, .. } => {
                        assert_eq!(children.len(), 3);
                        assert_eq!(children[0], SyntaxNode::Char('{'));
                        assert_eq!(children[1], SyntaxNode::Text("a".to_string()));
                        assert_eq!(children[2], SyntaxNode::Char('}'));
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
