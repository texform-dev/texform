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
    assert!(expect_arg(&args[0]).no_leading_space);
    assert!(expect_arg(&args[1]).no_leading_space);
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
        assert!(expect_arg(&args[0]).no_leading_space);
        if let Some(arg) = args[1].as_ref() {
            assert!(arg.no_leading_space);
        }
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
        assert!(expect_arg(&args[0]).no_leading_space);
        if let Some(arg) = args[1].as_ref() {
            assert!(arg.no_leading_space);
        }
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
                assert!(expect_arg(&args[0]).no_leading_space);
                assert!(expect_arg(&args[1]).no_leading_space);
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

fn parse_and_serialize(ctx: &ParseContext, src: &str) -> String {
    let output = ctx.parse(src, &ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for {src}: {:?}",
        output.diagnostics
    );
    serialize_node(
        &output
            .document()
            .unwrap_or_else(|| panic!("expected parse result for {src}"))
            .to_syntax(),
    )
}

#[test]
fn test_latex_array_linebreak_skips_spaces_before_spacing() {
    // LaTeX core `\@arraycr` looks ahead with `\@ifnextchar`, which skips
    // spaces.
    let ctx = ParseContext::from_packages(&["ams", "base"]);

    for (src, expected) in [
        (
            r"\begin{array}{c} a \\ [-5pt] b \end{array}",
            r"\begin {array} {c} a \\[-5pt] b \end {array}",
        ),
        (
            r"\begin{array}{c} a \\* [2pt] b \end{array}",
            r"\begin {array} {c} a \\*[2pt] b \end {array}",
        ),
        // A spaced `*` stays a matrix entry.
        (
            r"\begin{array}{cc} a & b \\ * & c \end{array}",
            r"\begin {array} {cc} a & b \\ * & c \end {array}",
        ),
        (
            r"\begin{eqnarray} a \\ [1pt] b \end{eqnarray}",
            r"\begin {eqnarray} a \\[1pt] b \end {eqnarray}",
        ),
        (
            r"\begin{eqnarray*} a \\ [1pt] b \end{eqnarray*}",
            r"\begin {eqnarray*} a \\[1pt] b \end {eqnarray*}",
        ),
        // Whitespace not followed by an argument stays content separation.
        (
            r"\begin{array}{c} a \\ b \end{array}",
            r"\begin {array} {c} a \\ b \end {array}",
        ),
    ] {
        let serialized = parse_and_serialize(&ctx, src);
        assert_eq!(serialized, expected, "for {src}");
        assert_eq!(parse_and_serialize(&ctx, &serialized), serialized);
    }
}

#[test]
fn test_amsmath_and_top_level_linebreak_keep_spaced_bracket_as_content() {
    // amsmath `\math@cr` uses `\new@ifnextchar`, which does not skip spaces,
    // so a row may start with an interval.
    let ctx = ParseContext::from_packages(&["ams", "base"]);

    for (src, expected) in [
        (
            r"\begin{align} a \\ [a,b] \end{align}",
            r"\begin {align} a \\ [ a , b ] \end {align}",
        ),
        (
            r"\begin{pmatrix} a \\ [a,b] \end{pmatrix}",
            r"\begin {pmatrix} a \\ [ a , b ] \end {pmatrix}",
        ),
        (
            r"\begin{subarray}{c} a \\ [a,b] \end{subarray}",
            r"\begin {subarray} {c} a \\ [ a , b ] \end {subarray}",
        ),
        (r"a \\ [a,b]", r"a \\ [ a , b ]"),
        // The innermost environment decides, in both nesting directions.
        (
            r"\begin{array}{c} \begin{matrix} a \\ [a,b] \end{matrix} \\ [2pt] c \end{array}",
            r"\begin {array} {c} \begin {matrix} a \\ [ a , b ] \end {matrix} \\[2pt] c \end {array}",
        ),
        (
            r"\begin{aligned} \begin{array}{c} a \\ [2pt] b \end{array} \\ [a,b] \end{aligned}",
            r"\begin {aligned} \begin {array} {c} a \\[2pt] b \end {array} \\ [ a , b ] \end {aligned}",
        ),
        // Command arguments do not inherit the array policy.
        (
            r"\begin{array}{c} \substack{a \\ [a,b]} \end{array}",
            r"\begin {array} {c} \substack { a \\ [ a , b ] } \end {array}",
        ),
    ] {
        let serialized = parse_and_serialize(&ctx, src);
        assert_eq!(serialized, expected, "for {src}");
        assert_eq!(parse_and_serialize(&ctx, &serialized), serialized);
    }
}
