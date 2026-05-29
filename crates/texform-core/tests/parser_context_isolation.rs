mod support;

use support::parser::*;
use texform_core::parse::{AllowedMode, CommandKind, ParseConfig};
use texform_interface::syntax_node::{ArgumentValue, ContentMode, SyntaxNode};

#[test]
fn underline_uses_math_and_text_variants_in_matching_modes() {
    let ctx = test_context_with_items([
        command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m"),
        command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T"),
    ]);

    let math = ctx
        .parse(r"\underline{x}", &ParseConfig::LENIENT)
        .try_into_document()
        .expect("expected math parse result")
        .0
        .to_syntax();
    let text = ctx
        .parse(r"\text{a \underline{b}}", &ParseConfig::LENIENT)
        .try_into_document()
        .expect("expected text parse result")
        .0
        .to_syntax();

    match math {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => {
                assert_eq!(unwrap_content(&args[0]), &SyntaxNode::Char('x'));
            }
            other => panic!("expected underline command, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }

    match text {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => match unwrap_content(&args[0]) {
                SyntaxNode::Group {
                    mode: ContentMode::Text,
                    children,
                    ..
                } => {
                    assert_eq!(children.len(), 2);
                    assert_eq!(children[0], SyntaxNode::Text("a ".to_string()));
                    match &children[1] {
                        SyntaxNode::Command { args, .. } => match &expect_arg(&args[0]).value {
                            ArgumentValue::TextContent(node) => {
                                assert_eq!(node, &SyntaxNode::Text("b".to_string()));
                            }
                            other => panic!("expected text content argument, got {:?}", other),
                        },
                        other => panic!("expected nested underline command, got {:?}", other),
                    }
                }
                other => panic!("expected text content group, got {:?}", other),
            },
            other => panic!("expected text command, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

#[test]
fn known_but_disallowed_command_is_mode_error_even_when_non_strict() {
    let ctx = test_context_with_items([command_item(
        "textonly",
        CommandKind::Prefix,
        AllowedMode::Text,
        "m:T",
    )]);

    let output = ctx.parse(r"\textonly{x}", &ParseConfig::LENIENT);
    let messages = output
        .diagnostics
        .into_iter()
        .map(|diag| diag.message)
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec!["Command \\textonly is not allowed in math mode"]
    );
}

#[test]
fn known_but_disallowed_environment_is_mode_error_in_both_strictness_modes() {
    let ctx = test_context_with_items([environment_item(
        "textenv",
        AllowedMode::Text,
        ContentMode::Text,
        "",
    )]);

    for strict in [false, true] {
        let config = if strict {
            ParseConfig::STRICT
        } else {
            ParseConfig::LENIENT
        };
        let output = ctx.parse(r"a \begin{textenv}b\end{textenv} c", &config);
        let messages = output
            .diagnostics
            .into_iter()
            .map(|diag| diag.message)
            .collect::<Vec<_>>();

        assert_eq!(
            messages,
            vec!["Environment textenv is not allowed in math mode"],
            "strict={strict}"
        );
    }
}

#[test]
fn disallowed_environment_does_not_rewrite_unrelated_generic_error() {
    let ctx = test_context_with_items([environment_item(
        "textenv",
        AllowedMode::Text,
        ContentMode::Text,
        "",
    )]);

    let output = ctx.parse(r"a \begin{textenv}b\end{textenv} }", &ParseConfig::LENIENT);
    let messages = output
        .diagnostics
        .into_iter()
        .map(|diag| diag.message)
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "Environment textenv is not allowed in math mode",
            "found '}' expected something else, or end of input",
        ]
    );
}

#[test]
fn test_parser_isolation_for_custom_commands() {
    let ctx1 = test_context_with_items([command_item(
        "fooisolated",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m",
    )]);

    let out1_foo = ctx1.parse(r"\fooisolated{a}", &ParseConfig::STRICT);
    assert!(out1_foo.diagnostics.is_empty());
    assert!(out1_foo.document().is_some());

    let out1_bar = ctx1.parse(r"\barisolated{a}", &ParseConfig::STRICT);
    assert!(!out1_bar.diagnostics.is_empty());
    assert!(out1_bar.document().is_none());

    let ctx2 = test_context_with_items([command_item(
        "barisolated",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m",
    )]);

    let out2_bar = ctx2.parse(r"\barisolated{a}", &ParseConfig::STRICT);
    assert!(out2_bar.diagnostics.is_empty());
    assert!(out2_bar.document().is_some());

    let out2_foo = ctx2.parse(r"\fooisolated{a}", &ParseConfig::STRICT);
    assert!(!out2_foo.diagnostics.is_empty());
    assert!(out2_foo.document().is_none());
}
