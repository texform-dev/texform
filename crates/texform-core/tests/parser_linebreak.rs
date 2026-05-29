mod support;

use support::parser::*;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_interface::syntax_node::{ArgumentValue, SyntaxNode};

#[test]
fn test_no_leading_space_prefix_for_linebreak_command() {
    let (immediate, _) = parse(r"\\*[1cm]", false).unwrap();
    let (name, args) = extract_first_command(immediate);
    assert_eq!(name, "\\");
    assert_eq!(args.len(), 2);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::Dimension("1cm".to_string())
    );

    let (spaced_star, _) = parse(r"\\ *", false).unwrap();
    match spaced_star {
        SyntaxNode::Root { children, .. } => {
            assert!(!children.is_empty());
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "\\");
                    assert_eq!(args.len(), 2);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
                    assert!(args[1].is_none());
                }
                other => panic!("Expected linebreak command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('*'));
        }
        _ => panic!("Expected root node"),
    }

    let (spaced_dimension, _) = parse(r"\\ [1cm]", false).unwrap();
    match spaced_dimension {
        SyntaxNode::Root { children, .. } => {
            assert!(!children.is_empty());
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "\\");
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
        _ => panic!("Expected root node"),
    }
}

#[test]
fn test_package_loaded_math_linebreak_supports_representative_forms() {
    let ctx = ParseContext::from_packages(&["ams", "base"]);

    for (src, expected_star, expected_dimension) in [
        (r"\begin{matrix}a\\b\end{matrix}", false, None),
        (r"\begin{matrix}a\\*b\end{matrix}", true, None),
        (r"\begin{matrix}a\\[5pt]b\end{matrix}", false, Some("5pt")),
    ] {
        let output = ctx.parse(src, &ParseConfig::STRICT);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let result = output
            .document()
            .unwrap_or_else(|| panic!("expected parse result for {src}"));
        let syntax = result.to_syntax();
        let args = extract_command_args(&syntax, "\\")
            .unwrap_or_else(|| panic!("expected linebreak command in {src}"));

        assert_eq!(args.len(), 2, "expected star + optional length slots");
        assert_eq!(
            expect_arg(&args[0]).value,
            ArgumentValue::Boolean(expected_star)
        );
        match expected_dimension {
            Some(length) => assert_eq!(
                expect_arg(&args[1]).value,
                ArgumentValue::Dimension(length.to_string())
            ),
            None => assert!(args[1].is_none(), "unexpected optional length for {src}"),
        }
    }
}

#[test]
fn test_package_loaded_text_linebreak_supports_representative_forms() {
    let ctx = ParseContext::from_packages(&["base", "textmacros"]);

    for (src, expected_star, expected_dimension) in [
        (r"\text{a\\b}", false, None),
        (r"\text{a\\*b}", true, None),
        (r"\text{a\\[5pt]b}", false, Some("5pt")),
    ] {
        let output = ctx.parse(src, &ParseConfig::STRICT);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let result = output
            .document()
            .unwrap_or_else(|| panic!("expected parse result for {src}"));
        let syntax = result.to_syntax();
        let args = extract_command_args(&syntax, "\\")
            .unwrap_or_else(|| panic!("expected linebreak command in {src}"));

        assert_eq!(args.len(), 2, "expected star + optional length slots");
        assert_eq!(
            expect_arg(&args[0]).value,
            ArgumentValue::Boolean(expected_star)
        );
        match expected_dimension {
            Some(length) => assert_eq!(
                expect_arg(&args[1]).value,
                ArgumentValue::Dimension(length.to_string())
            ),
            None => assert!(args[1].is_none(), "unexpected optional length for {src}"),
        }
    }
}

#[test]
fn test_newline_command_preserves_no_leading_space_behavior() {
    let ctx = test_context();

    let immediate = ctx.parse(r"\newline*[1cm]", &ParseConfig::STRICT);
    assert!(
        immediate.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        immediate.diagnostics
    );
    let immediate_node = immediate
        .document()
        .unwrap_or_else(|| panic!("expected parse result"))
        .to_syntax()
        .clone();
    match immediate_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "newline");
                assert_eq!(args.len(), 2);
                assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
                assert_eq!(
                    expect_arg(&args[1]).value,
                    ArgumentValue::Dimension("1cm".to_string())
                );
            }
            other => panic!("Expected newline command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }

    let spaced = ctx.parse(r"\newline * [1cm]", &ParseConfig::STRICT);
    assert!(
        spaced.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        spaced.diagnostics
    );
    let spaced_node = spaced
        .document()
        .unwrap_or_else(|| panic!("expected parse result"))
        .to_syntax()
        .clone();
    match spaced_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 7);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "newline");
                    assert_eq!(args.len(), 2);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
                    assert!(
                        args[1].is_none(),
                        "spaced optional dimension should not match"
                    );
                }
                other => panic!("Expected newline command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('*'));
            assert_eq!(children[2], SyntaxNode::Char('['));
            assert_eq!(children[3], SyntaxNode::Char('1'));
            assert_eq!(children[4], SyntaxNode::Char('c'));
            assert_eq!(children[5], SyntaxNode::Char('m'));
            assert_eq!(children[6], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}
