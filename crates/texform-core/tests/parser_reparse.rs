mod support;

use support::parser::*;
use texform_core::parse::{AllowedMode, CommandKind, ParseConfig};
use texform_interface::syntax_node::{ContentMode, GroupKind, SyntaxNode};

#[test]
fn test_text_in_command() {
    // "\text{Hello World}" - text mode in command argument
    let (result, _) = parse(r"\text{Hello World}", false).unwrap();

    // Debug print the result
    match result {
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        _ => panic!("Expected root node"),
    }
}

#[test]
fn test_text_apostrophes_parse_as_text() {
    for (src, expected_text, expected_serialized) in [
        (
            r"\text{Graham's number}",
            "Graham's number",
            r"\text {Graham's number}",
        ),
        (r"\text{Z'-factor}", "Z'-factor", r"\text {Z'-factor}"),
    ] {
        let (result, _) = parse(src, false).unwrap();
        assert_eq!(serialize_node(&result), expected_serialized);

        match result {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => match unwrap_content(&args[0]) {
                    SyntaxNode::Group { mode, children, .. } => {
                        assert_eq!(*mode, ContentMode::Text);
                        assert_eq!(children, &[SyntaxNode::Text(expected_text.to_string())]);
                    }
                    SyntaxNode::Text(text) => assert_eq!(text, expected_text),
                    other => panic!("Expected text content, got {:?}", other),
                },
                other => panic!("Expected text command, got {:?}", other),
            },
            other => panic!("Expected root node, got {:?}", other),
        }
    }
}

#[test]
fn test_text_inline_math_active_char_and_command() {
    let errors = parse(r"\text{A~$x$B\frac{a}{b}}", false)
        .expect_err("frac is math-only and must fail in text mode");
    assert!(!errors.is_empty());
}

#[test]
fn test_sqrt_accepts_command_token_as_best_fit_argument() {
    let (result, _) = parse(r"\sqrt\frac{1}{2}", false).unwrap();
    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "sqrt");
                    assert_eq!(args.len(), 2);
                    assert!(
                        args[0].is_none(),
                        "optional slot should be None when not provided"
                    );
                    match unwrap_content(&args[1]) {
                        SyntaxNode::Command {
                            name: inner_name, ..
                        } => {
                            assert_eq!(inner_name, "frac");
                        }
                        other => panic!("Expected frac command as sqrt argument, got {:?}", other),
                    }
                }
                other => panic!("Expected sqrt command, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_optional_argument_reparse_keeps_known_command() {
    let (result, _) = parse(r"\sqrt[\frac{1}{2}]{x}", false).unwrap();
    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "sqrt");
                match unwrap_content(&args[0]) {
                    SyntaxNode::Command { name, .. } => {
                        assert_eq!(name, "frac");
                    }
                    other => panic!("Expected frac command in optional slot, got {:?}", other),
                }
            }
            other => panic!("Expected sqrt command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_delimited_content_argument_reparse_keeps_known_command() {
    let ctx = test_context_with_items([
        command_item("probe", CommandKind::Prefix, AllowedMode::Math, "r()"),
        command_item("frac", CommandKind::Prefix, AllowedMode::Math, "mm"),
    ]);

    let output = ctx.parse(r"\probe(\frac{1}{2})", &ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let node = output
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "probe");
                match unwrap_content(&args[0]) {
                    SyntaxNode::Command { name, .. } => {
                        assert_eq!(name, "frac");
                    }
                    other => panic!("Expected frac command in delimited slot, got {:?}", other),
                }
            }
            other => panic!("Expected probe command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_text_mode_inline_math_reparse_keeps_known_command() {
    let (result, _) = parse(r"\text{$\frac{a}{b}$}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "text");
                match unwrap_content(&args[0]) {
                    SyntaxNode::Group {
                        kind: GroupKind::InlineMath,
                        children,
                        ..
                    } => match &children[0] {
                        SyntaxNode::Command { name, .. } => assert_eq!(name, "frac"),
                        other => {
                            panic!(
                                "Expected frac command inside inline math group, got {:?}",
                                other
                            )
                        }
                    },
                    other => panic!("Expected inline math group, got {:?}", other),
                }
            }
            other => panic!("Expected text command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}
