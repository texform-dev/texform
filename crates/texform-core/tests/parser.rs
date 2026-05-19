use std::sync::OnceLock;

use chumsky::error::RichReason;
use texform_core::api::serialize_latex;
use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseConfig, ParseContext, ParseContextBuilder, ParseDiagnosticKind,
};
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};
use texform_specs::specs::load_package_specs_from_str;

fn parse(src: &str, strict: bool) -> Result<(SyntaxNode, chumsky::span::SimpleSpan), Vec<String>> {
    let config = if strict {
        ParseConfig::STRICT_NO_RECOVER
    } else {
        ParseConfig::NONSTRICT_RECOVER
    };
    let output = test_context().parse(src, &config);
    if output.diagnostics.is_empty() {
        let result = output
            .result
            .expect("parse succeeded without diagnostics but produced no result");
        Ok((
            result.node,
            chumsky::span::SimpleSpan::from(result.span.start..result.span.end),
        ))
    } else {
        Err(output
            .diagnostics
            .into_iter()
            .map(|diag| diag.message)
            .collect())
    }
}

fn linebreak_test_items() -> [ContextItem; 2] {
    [
        command_item("\\", CommandKind::Prefix, AllowedMode::Both, "!s !o:L").into(),
        command_item("newline", CommandKind::Prefix, AllowedMode::Both, "!s !o:L").into(),
    ]
}

fn test_context() -> ParseContext {
    static BASE_CTX: OnceLock<ParseContext> = OnceLock::new();
    BASE_CTX
        .get_or_init(|| {
            let mut builder = ParseContextBuilder::empty().packages(&["base"]);
            for item in shared_test_items() {
                builder = builder.insert_item(item.clone());
            }
            for item in linebreak_test_items() {
                builder = builder.insert_item(item);
            }
            builder.build().expect("shared test items should be valid")
        })
        .clone()
}

fn test_context_with_items<I, T>(items: I) -> ParseContext
where
    I: IntoIterator<Item = T>,
    T: Into<ContextItem>,
{
    let mut builder = ParseContextBuilder::empty().packages(&["base"]);
    for item in shared_test_items() {
        builder = builder.insert_item(item.clone());
    }
    for item in linebreak_test_items() {
        builder = builder.insert_item(item);
    }
    for item in items {
        builder = builder.insert_item(item);
    }
    builder
        .build()
        .expect("test items should have valid xparse specs")
}

fn shared_test_items() -> &'static [ContextItem] {
    static ITEMS: OnceLock<Vec<ContextItem>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        let specs = load_package_specs_from_str(
            r#"
commands:
  - name: frac
    kind: prefix
    allowed_mode: math
    argspec: 'm m'
  - name: sqrt
    kind: prefix
    allowed_mode: math
    argspec: 'o m'
  - name: text
    kind: prefix
    allowed_mode: math
    argspec: 'm:T'
  - name: alpha
    kind: prefix
    allowed_mode: math
    argspec: ''
  - name: beta
    kind: prefix
    allowed_mode: math
    argspec: ''
  - name: gamma
    kind: prefix
    allowed_mode: math
    argspec: ''
  - name: delim
    kind: prefix
    allowed_mode: math
    argspec: 'm:D'
  - name: hspace
    kind: prefix
    allowed_mode: both
    argspec: 'm:L'
  - name: romannumeral
    kind: prefix
    allowed_mode: both
    argspec: 'm:I'
  - name: includegraphics
    kind: prefix
    allowed_mode: both
    argspec: 'o:K m:T'
  - name: label
    kind: prefix
    allowed_mode: both
    argspec: 'm:N'
  - name: over
    kind: infix
    allowed_mode: math
    argspec: ''
  - name: choose
    kind: infix
    allowed_mode: math
    argspec: ''
  - name: bfseries
    kind: declarative
    allowed_mode: both
    argspec: ''
  - name: qty
    kind: prefix
    allowed_mode: math
    argspec: 'd<(,)><[,]><{,}><|,|>'
  - name: pqty
    kind: prefix
    allowed_mode: math
    argspec: 's r{}'
  - name: abs
    kind: prefix
    allowed_mode: math
    argspec: 's r{}'
  - name: eval
    kind: prefix
    allowed_mode: math
    argspec: 's d<(,|><[,|><{,}>'
  - name: mqty
    kind: prefix
    allowed_mode: math
    argspec: 's d<(,)><[,]><{,}><|,|>'
  - name: dd
    kind: prefix
    allowed_mode: math
    argspec: 'o d<(,)><{,}>'
  - name: dv
    kind: prefix
    allowed_mode: math
    argspec: 's o m g'
  - name: pdv
    kind: prefix
    allowed_mode: math
    argspec: 's o m g g'
  - name: braket
    kind: prefix
    allowed_mode: math
    argspec: 's m g'
  - name: exp
    kind: prefix
    allowed_mode: math
    argspec: ''

environments:
  - name: matrix
    allowed_mode: math
    argspec: ''
    body_mode: math
  - name: align
    allowed_mode: math
    argspec: ''
    body_mode: math
  - name: align*
    allowed_mode: math
    argspec: ''
    body_mode: math

delimiters:
  - name: "."
    is_control_sequence: false
    allowed_mode: math
    unicode_value: ""
    attributes: {}
  - name: "("
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "("
    attributes: {}
  - name: ")"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: ")"
    attributes: {}
  - name: "["
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "["
    attributes: {}
  - name: "]"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "]"
    attributes: {}
  - name: "|"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "|"
    attributes: {}
  - name: "<"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "<"
    attributes: {}
  - name: ">"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: ">"
    attributes: {}
  - name: "/"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "/"
    attributes: {}
  - name: langle
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟨"
    attributes: {}
  - name: rangle
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟩"
    attributes: {}
  - name: "{"
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "{"
    attributes: {}
  - name: "}"
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "}"
    attributes: {}
  - name: lfloor
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌊"
    attributes: {}
  - name: rfloor
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌋"
    attributes: {}
  - name: lceil
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌈"
    attributes: {}
  - name: rceil
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌉"
    attributes: {}
  - name: lvert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "|"
    attributes: {}
  - name: rvert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "|"
    attributes: {}
  - name: lVert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "‖"
    attributes: {}
  - name: rVert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "‖"
    attributes: {}
  - name: lgroup
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟮"
    attributes: {}
  - name: rgroup
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟯"
    attributes: {}
  - name: lmoustache
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⎰"
    attributes: {}
  - name: rmoustache
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⎱"
    attributes: {}
  - name: "|"
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "‖"
    attributes: {}
"#,
            "inline-test",
        );

        let mut items = Vec::new();
        for command in specs.commands {
            items.push(
                CommandItem::new(
                    command.name,
                    command.kind,
                    command.allowed_mode,
                    command.argspec.source,
                )
                .with_tags(command.tags)
                .into(),
            );
        }
        for environment in specs.environments {
            items.push(
                EnvironmentItem::new(
                    environment.name,
                    environment.allowed_mode,
                    environment.body_mode,
                    environment.argspec.source,
                )
                .with_tags(environment.tags)
                .into(),
            );
        }
        for delimiter in specs.delimiters {
            if delimiter.is_control_sequence {
                items.push(DelimiterControlItem::new(delimiter.name).into());
            }
        }
        items
    })
}

fn command_item(
    name: &str,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    spec: &str,
) -> CommandItem {
    CommandItem::new(name, kind, allowed_mode, spec)
}

fn environment_item(
    name: &str,
    allowed_mode: AllowedMode,
    body_mode: ContentMode,
    spec: &str,
) -> EnvironmentItem {
    EnvironmentItem::new(name, allowed_mode, body_mode, spec)
}

fn label_command_item() -> CommandItem {
    command_item("label", CommandKind::Prefix, AllowedMode::Both, "m:N")
}

fn expect_arg(slot: &Option<Argument>) -> &Argument {
    slot.as_ref()
        .unwrap_or_else(|| panic!("Expected argument slot to be present"))
}

fn unwrap_content(slot: &Option<Argument>) -> &SyntaxNode {
    match &expect_arg(slot).value {
        ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => node,
        _ => panic!("Expected content argument"),
    }
}

fn assert_same_structure(with_spaces: &str, compact: &str) {
    let (result_spaces, _) = parse(with_spaces, false).unwrap();
    let (result_compact, _) = parse(compact, false).unwrap();
    assert_eq!(result_spaces, result_compact);
}

#[test]
fn underline_uses_math_and_text_variants_in_matching_modes() {
    let ctx = test_context_with_items([
        command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m"),
        command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T"),
    ]);

    let math = ctx
        .parse(r"\underline{x}", &ParseConfig::NONSTRICT_RECOVER)
        .result
        .expect("expected math parse result")
        .node;
    let text = ctx
        .parse(r"\text{a \underline{b}}", &ParseConfig::NONSTRICT_RECOVER)
        .result
        .expect("expected text parse result")
        .node;

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

    let output = ctx.parse(r"\textonly{x}", &ParseConfig::NONSTRICT_RECOVER);
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
            ParseConfig::STRICT_NO_RECOVER
        } else {
            ParseConfig::NONSTRICT_RECOVER
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
                            &vec![SyntaxNode::Char('\''), SyntaxNode::Char('2')]
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
fn disallowed_environment_does_not_rewrite_unrelated_generic_error() {
    let ctx = test_context_with_items([environment_item(
        "textenv",
        AllowedMode::Text,
        ContentMode::Text,
        "",
    )]);

    let output = ctx.parse(
        r"a \begin{textenv}b\end{textenv} }",
        &ParseConfig::NONSTRICT_RECOVER,
    );
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
fn test_text_argument_uses_text_content_variant_for_single_char_item() {
    let output = ParseContext::shared().parse(r"\text{\%}", &ParseConfig::STRICT_NO_RECOVER);
    let result = output.result.expect("expected parse result");

    match result.node {
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
fn test_bare_left_reports_invalid_left_delimiter() {
    let diagnostics = parse(r"\left\foo x \right)", false).unwrap_err();
    assert_eq!(diagnostics[0], "invalid \\left delimiter");
}

#[test]
fn invalid_left_delimiter_reports_root_cause_and_contexts() {
    let src = r"\begin{aligned}\probe[\left\foo x]\end{aligned}";
    let output = test_context_with_items([
        ContextItem::from(environment_item(
            "aligned",
            AllowedMode::Math,
            ContentMode::Math,
            "",
        )),
        ContextItem::from(command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "o",
        )),
    ])
    .parse(src, &ParseConfig::NONSTRICT_RECOVER);

    assert!(
        output.result.is_some(),
        "recoverable subparse should keep a partial result"
    );

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message == "invalid \\left delimiter")
        .unwrap_or_else(|| panic!("diagnostics: {:?}", output.diagnostics));
    assert_eq!(diagnostic.contexts.len(), 3);

    let environment_context = diagnostic
        .contexts
        .iter()
        .find(|context| context.label == "environment body")
        .expect("missing environment body context");

    let argument_context = diagnostic
        .contexts
        .iter()
        .find(|context| context.label == "command argument")
        .expect("missing command argument context");

    let left_context = diagnostic
        .contexts
        .iter()
        .find(|context| context.label == "left-delimited group")
        .expect("missing left-delimited group context");

    assert_eq!(
        &src[argument_context.span.start..argument_context.span.end],
        r"[\left\foo x]"
    );
    assert_eq!(
        &src[environment_context.span.start..environment_context.span.end],
        r"\probe[\left\foo"
    );

    assert_eq!(
        &src[left_context.span.start..left_context.span.end],
        r"\left\foo"
    );
}

#[test]
fn test_later_left_item_reports_invalid_left_delimiter() {
    let diagnostics = parse(r"\left( x \right) + \left\foo y \right)", false).unwrap_err();
    assert_eq!(diagnostics[0], "invalid \\left delimiter");
}

// ========================================================================
// Stage 3 Tests (Command parsing)
// ========================================================================

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
fn test_dimension_argument() {
    // "\hspace1em"
    let (result, _) = parse(r"\hspace1em", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
fn test_csname_argument() {
    let (result, _) = parse(r"\label{sec:intro}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "label");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Mandatory);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::CSName("sec:intro".to_string())
                );
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_csname_argument_rejects_escape_sequence() {
    let ctx = test_context_with_items([label_command_item()]);

    let command = ctx.parse(r"\label{\alpha}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        command.result.is_none(),
        "control sequence inside CSName should fail"
    );
    assert!(
        !command.diagnostics.is_empty(),
        "expected CSName diagnostics, got {:?}",
        command.diagnostics
    );

    let escaped_symbol = ctx.parse(r"\label{sec\_a}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        escaped_symbol.result.is_none(),
        "escaped symbol inside CSName should fail"
    );
    assert!(
        !escaped_symbol.diagnostics.is_empty(),
        "expected CSName diagnostics, got {:?}",
        escaped_symbol.diagnostics
    );
}

#[test]
fn test_delimiter_argument_braced_matches_inline() {
    let (inline, _) = parse(r"\delim\langle", false).unwrap();
    let (braced, _) = parse(r"\delim{\langle}", false).unwrap();

    let inline_value = match inline {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    let braced_value = match braced {
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    let braced_value = match braced {
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    };

    let braced_value = match braced {
        SyntaxNode::Root { children, .. } => match &children[0] {
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
        SyntaxNode::Root { children, .. } => match &children[0] {
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
fn test_comment_truncated_argument_reports_specific_kind() {
    let output = test_context().parse(
        r"\frac{%\ change \ in \ x}{%\ change \ in \ y}",
        &ParseConfig::NONSTRICT_RECOVER,
    );

    let diagnostic = output
        .diagnostics
        .first()
        .unwrap_or_else(|| panic!("expected diagnostic, got output: {:?}", output.result));
    assert_eq!(
        diagnostic.kind,
        Some(ParseDiagnosticKind::CommentTruncatedArgument)
    );
    assert_eq!(
        diagnostic.message,
        "Unescaped % starts a comment inside this argument"
    );
}

#[test]
fn test_math_shift_inside_formula_reports_specific_kind() {
    let output = test_context_with_items([environment_item(
        "cases",
        AllowedMode::Math,
        ContentMode::Math,
        "",
    )])
    .parse(
        r"f_X(x) = \begin{cases}$1000 & 0.01\\$0 & 0.99\end{cases}",
        &ParseConfig::NONSTRICT_RECOVER,
    );

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.kind == Some(ParseDiagnosticKind::UnexpectedMathShift))
        .unwrap_or_else(|| panic!("diagnostics: {:?}", output.diagnostics));
    assert_eq!(
        diagnostic.message,
        "Unexpected $ inside a math formula; it looks like a currency marker"
    );
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
fn test_unknown_command_strict() {
    // "\unknown{x}" in strict mode should error
    let result = parse(r"\unknown{x}", true);
    assert!(result.is_err());
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
fn test_unknown_environment_strict() {
    let result = parse(r"\begin{foo}a\end{foo}", true);
    assert_eq!(result.unwrap_err(), vec!["Unknown environment: foo"]);
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
fn test_ambiguous_over_reports_diagnostic() {
    let errors = texform_core::parser::parse(r"a \over b + 1 \over c", true)
        .expect_err("ambiguous repeated top-level infix should fail");
    let messages: Vec<_> = errors
        .iter()
        .map(|error| match error.reason() {
            RichReason::Custom(message) => message.clone(),
            RichReason::ExpectedFound { .. } => format!("{error}"),
        })
        .collect();

    assert!(
        messages
            .iter()
            .any(|message| message.ends_with("Ambiguous use of \\over")),
        "{:?}",
        messages
    );
}

#[test]
fn test_repeated_over_still_reports_ambiguous_infix_kind() {
    let output = test_context().parse(r"a \over b + 1 \over c", &ParseConfig::NONSTRICT_RECOVER);

    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message == "Ambiguous use of \\over")
        .unwrap_or_else(|| panic!("diagnostics: {:?}", output.diagnostics));
    assert_eq!(diagnostic.kind, Some(ParseDiagnosticKind::AmbiguousInfix));
}

#[test]
fn test_repeated_buildrel_over_parses_as_separate_infixes() {
    let ctx = ParseContext::from_packages(&["base"]);
    let src = r"\cdots\to K\buildrel f\over\longrightarrow K\buildrel f\over\longrightarrow K";

    for config in [
        ParseConfig::STRICT_NO_RECOVER,
        ParseConfig::NONSTRICT_RECOVER,
    ] {
        let output = ctx.parse(src, &config);
        assert!(
            output.diagnostics.is_empty(),
            "diagnostics for {:?}: {:?}",
            config,
            output.diagnostics
        );
        assert!(
            output.result.is_some(),
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
    .parse(r"\displaystyle \over x", &ParseConfig::NONSTRICT_RECOVER);

    assert!(
        output.diagnostics.is_empty(),
        "expected declarative-before-infix parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .result
        .expect("parse without diagnostics should produce a result")
        .node;

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
        &ParseConfig::NONSTRICT_RECOVER,
    );
    assert!(
        output.diagnostics.is_empty(),
        "Expected nested textstyle parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .result
        .expect("parse without diagnostics should produce a result")
        .node;

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
        assert_eq!(serialize_latex(&result), expected_serialized);

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
        .parse(r"\text{\underline{a^2}}", &ParseConfig::STRICT_NO_RECOVER)
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

    let output = ctx.parse(r"\text{\tiny{FP}}", &ParseConfig::NONSTRICT_RECOVER);
    assert!(
        output.diagnostics.is_empty(),
        "expected flat text declarative parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .result
        .expect("parse without diagnostics should produce a result")
        .node;

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
fn test_environment_name_mismatch() {
    let result = parse(r"\begin{matrix}a\end{align}", false);
    assert!(result.is_err());
}

#[test]
fn test_environment_missing_end_errors() {
    let errors = parse(r"\begin{matrix}", false).expect_err("expected missing end error");
    let debug = format!("{errors:?}");
    assert!(
        debug.contains("\\end{matrix}"),
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
        SyntaxNode::Root { children, .. } => {
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
                            children: vec![SyntaxNode::Char('\'')],
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
fn test_bare_prime_superscript_still_errors() {
    let diagnostics = parse(r"f^'", false).unwrap_err();

    assert_eq!(diagnostics, vec!["not a command"]);
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
    // "'x" -> prime with empty base then x
    let (result, _) = parse("'x", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => {
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
fn test_prime_nested_shapes() {
    let cases = [
        (
            r"x^{'^{'}}",
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Explicit,
                children: vec![SyntaxNode::Scripted {
                    base: Box::new(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![],
                    }),
                    subscript: None,
                    superscript: Some(Box::new(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![
                            SyntaxNode::Char('\''),
                            SyntaxNode::Group {
                                mode: ContentMode::Math,
                                kind: GroupKind::Explicit,
                                children: vec![SyntaxNode::Char('\'')],
                            },
                        ],
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
                        children: vec![SyntaxNode::Char('\'')],
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
                    base: Box::new(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![],
                    }),
                    subscript: None,
                    superscript: Some(Box::new(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![
                            SyntaxNode::Char('\''),
                            SyntaxNode::Group {
                                mode: ContentMode::Math,
                                kind: GroupKind::Explicit,
                                children: vec![SyntaxNode::Char('a')],
                            },
                        ],
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
        SyntaxNode::Root { children, .. } => {
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
    let output = ctx.parse(r"a \over \displaystyle b", &ParseConfig::NONSTRICT_RECOVER);
    assert!(
        output.diagnostics.is_empty(),
        "Expected declarative denominator parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .result
        .expect("parse without diagnostics should produce a result")
        .node;

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
    .parse(
        r"a \displaystyle b \over c",
        &ParseConfig::NONSTRICT_RECOVER,
    );

    assert!(
        output.diagnostics.is_empty(),
        "expected flat declarative infix parse without diagnostics, got {:?}",
        output.diagnostics
    );
    let result = output
        .result
        .expect("parse without diagnostics should produce a result")
        .node;

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

#[test]
fn test_dimension_with_spaces() {
    // "\hspace{1.5 cm}" - dimension with spaces between number and unit
    let (result, _) = parse(r"\hspace{1.5 cm}", false).unwrap();

    match result {
        SyntaxNode::Root { children, .. } => match &children[0] {
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

fn extract_first_command(node: SyntaxNode) -> (String, Vec<Option<Argument>>) {
    match node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => (name.clone(), args.clone()),
            other => panic!("Expected command node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

fn extract_command_args<'a>(node: &'a SyntaxNode, name: &str) -> Option<&'a [Option<Argument>]> {
    match node {
        SyntaxNode::Command {
            name: command_name,
            args,
            ..
        } if command_name == name => Some(args.as_slice()),
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => children
            .iter()
            .find_map(|child| extract_command_args(child, name)),
        SyntaxNode::Command { args, .. } => args.iter().find_map(|slot| {
            slot.as_ref().and_then(|arg| match &arg.value {
                ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
                    extract_command_args(node, name)
                }
                _ => None,
            })
        }),
        SyntaxNode::Declarative { args, .. } => args.iter().find_map(|slot| {
            slot.as_ref().and_then(|arg| match &arg.value {
                ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
                    extract_command_args(node, name)
                }
                _ => None,
            })
        }),
        SyntaxNode::Environment { args, body, .. } => args
            .iter()
            .find_map(|slot| {
                slot.as_ref().and_then(|arg| match &arg.value {
                    ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
                        extract_command_args(node, name)
                    }
                    _ => None,
                })
            })
            .or_else(|| extract_command_args(body, name)),
        SyntaxNode::Infix {
            args, left, right, ..
        } => args
            .iter()
            .find_map(|slot| {
                slot.as_ref().and_then(|arg| match &arg.value {
                    ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
                        extract_command_args(node, name)
                    }
                    _ => None,
                })
            })
            .or_else(|| extract_command_args(left, name))
            .or_else(|| extract_command_args(right, name)),
        _ => None,
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
        let (name, args) = extract_first_command(result);
        assert_eq!(name, "qty");
        assert_eq!(args.len(), 1);

        let arg = expect_arg(&args[0]);
        assert_eq!(arg.value, ArgumentValue::MathContent(SyntaxNode::Char('x')));
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
    let (name, args) = extract_first_command(result);
    assert_eq!(name, "qty");
    assert_eq!(args.len(), 1);
    assert!(args[0].is_none());
}

#[test]
fn test_arg_true_quantity_commands_require_braces() {
    let (pqty_ok, _) = parse(r"\pqty{x}", false).unwrap();
    let (name, args) = extract_first_command(pqty_ok);
    assert_eq!(name, "pqty");
    assert_eq!(args.len(), 2);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));

    let content = expect_arg(&args[1]);
    assert_eq!(
        content.value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
    match content.kind {
        ArgumentKind::Delimited { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("Expected brace-delimited argument, got {:?}", other),
    }

    let (abs_ok, _) = parse(r"\abs{x}", false).unwrap();
    let (name, args) = extract_first_command(abs_ok);
    assert_eq!(name, "abs");
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );

    assert!(parse(r"\pqty(x)", false).is_err());
    assert!(parse(r"\abs|x|", false).is_err());
}

#[test]
fn test_eval_uses_nonsymmetric_paired_delimiter() {
    let (result, _) = parse(r"\eval(x|", false).unwrap();
    let (name, args) = extract_first_command(result);
    assert_eq!(name, "eval");
    assert_eq!(args.len(), 2);

    let star = expect_arg(&args[0]);
    assert_eq!(star.kind, ArgumentKind::Star);
    assert_eq!(star.value, ArgumentValue::Boolean(false));

    let paired = expect_arg(&args[1]);
    assert_eq!(
        paired.value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
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
    let (name, args) = extract_first_command(dv_result);
    assert_eq!(name, "dv");
    assert_eq!(args.len(), 4);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Boolean(false),
        "star slot should always exist",
    );
    assert!(args[1].is_none(), "optional bracket slot should be None");
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('f'))
    );
    assert!(args[3].is_none(), "group slot should be None when absent");

    let (pdv_result, _) = parse(r"\pdv{f}{x}{y}", false).unwrap();
    let (name, args) = extract_first_command(pdv_result);
    assert_eq!(name, "pdv");
    assert_eq!(args.len(), 5);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
    assert!(args[1].is_none());
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('f'))
    );
    assert_eq!(expect_arg(&args[3]).kind, ArgumentKind::Group);
    assert_eq!(expect_arg(&args[4]).kind, ArgumentKind::Group);
}

#[test]
fn test_braket_optional_group_slot() {
    let (result_full, _) = parse(r"\braket{a}{b}", false).unwrap();
    let (_, args_full) = extract_first_command(result_full);
    assert_eq!(args_full.len(), 3);
    assert_eq!(
        expect_arg(&args_full[0]).value,
        ArgumentValue::Boolean(false)
    );
    assert_eq!(
        expect_arg(&args_full[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('a'))
    );
    assert_eq!(expect_arg(&args_full[2]).kind, ArgumentKind::Group);

    let (result_short, _) = parse(r"\braket{a}", false).unwrap();
    let (_, args_short) = extract_first_command(result_short);
    assert_eq!(args_short.len(), 3);
    assert!(args_short[2].is_none());
}

#[test]
fn test_exp_does_not_consume_star_without_s_slot() {
    let (result, _) = parse(r"\exp*", false).unwrap();
    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "exp");
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "exp");
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
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('['));
            assert_eq!(children[1], SyntaxNode::Char('a'));
            assert_eq!(children[2], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

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
        let output = ctx.parse(src, &ParseConfig::STRICT_NO_RECOVER);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let result = output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result for {src}"));
        let args = extract_command_args(&result.node, "\\")
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
        let output = ctx.parse(src, &ParseConfig::STRICT_NO_RECOVER);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let result = output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result for {src}"));
        let args = extract_command_args(&result.node, "\\")
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
fn test_package_loaded_non_alpha_math_commands_support_representative_forms() {
    let ctx = ParseContext::from_packages(&["ams", "base", "braket", "physics"]);

    for src in [
        r"a\,b", r"a\!b", r"a\;b", r"a\:b", r"a\>b", r"a\*b", r"a\ b",
    ] {
        let output = ctx.parse(src, &ParseConfig::STRICT_NO_RECOVER);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        assert!(output.result.is_some(), "expected parse result for {src}");
    }

    let output = ctx.parse(r"\bra{x}\|\ket{y}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for braket sample: {:?}",
        output.diagnostics
    );
    let result = output
        .result
        .as_ref()
        .expect("expected parse result for braket sample");
    assert!(
        extract_command_args(&result.node, "|").is_some(),
        "expected package-backed \\| command"
    );
}

#[test]
fn test_package_loaded_non_alpha_text_commands_support_representative_forms() {
    let ctx = ParseContext::from_packages(&["base", "textmacros"]);

    for src in [r"\text{a\,b}", r"\text{a\ b}"] {
        let output = ctx.parse(src, &ParseConfig::STRICT_NO_RECOVER);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        assert!(output.result.is_some(), "expected parse result for {src}");
    }

    for (src, command_name) in [
        (r"\text{\'e}", "'"),
        (r"\text{\~n}", "~"),
        (r#"\text{\"o}"#, "\""),
    ] {
        let output = ctx.parse(src, &ParseConfig::STRICT_NO_RECOVER);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let result = output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result for {src}"));
        assert!(
            extract_command_args(&result.node, command_name).is_some(),
            "expected package-backed command {command_name:?} in {src}"
        );
    }
}

#[test]
fn test_no_leading_space_after_single_token_m_for_optional_brackets() {
    let ctx = test_context_with_items([command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m !o",
    )]);

    let spaced = ctx.parse(r"\probe a [b]", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        spaced.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        spaced.diagnostics
    );
    let spaced_node = spaced
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match spaced_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 2);
                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('a'))
                    );
                    assert!(args[1].is_none(), "spaced !o slot should not match");
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('['));
            assert_eq!(children[2], SyntaxNode::Char('b'));
            assert_eq!(children[3], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }

    let tight = ctx.parse(r"\probe a[b]", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        tight.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        tight.diagnostics
    );
    let tight_node = tight
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match tight_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 2);
                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('a'))
                    );
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('b'))
                    );
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_no_leading_space_after_single_token_m_for_group_slot() {
    let ctx = test_context_with_items([command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "s o m !g",
    )]);

    let spaced = ctx.parse(r"\probe*[n]f {x}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        spaced.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        spaced.diagnostics
    );
    let spaced_node = spaced
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match spaced_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 4);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('n'))
                    );
                    assert_eq!(
                        expect_arg(&args[2]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('f'))
                    );
                    assert!(args[3].is_none(), "spaced !g slot should not match");
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
            match &children[1] {
                SyntaxNode::Group {
                    kind: GroupKind::Explicit,
                    children: group_children,
                    ..
                } => {
                    assert_eq!(group_children.len(), 1);
                    assert_eq!(group_children[0], SyntaxNode::Char('x'));
                }
                other => panic!("Expected trailing explicit group, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }

    let tight = ctx.parse(r"\probe*[n]f{x}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        tight.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        tight.diagnostics
    );
    let tight_node = tight
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match tight_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 4);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('n'))
                    );
                    assert_eq!(
                        expect_arg(&args[2]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('f'))
                    );
                    assert_eq!(expect_arg(&args[3]).kind, ArgumentKind::Group);
                    assert_eq!(
                        expect_arg(&args[3]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('x'))
                    );
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_required_group_form_enforces_braces() {
    let ctx = test_context_with_items([command_item(
        "reqgrp",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m{}",
    )]);

    let present = ctx.parse(r"\reqgrp{x}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        present.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        present.diagnostics
    );
    let present_node = present
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match present_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "reqgrp");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::MathContent(SyntaxNode::Char('x'))
                );
            }
            other => panic!("Expected reqgrp command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }

    let missing = ctx.parse(r"\reqgrp x", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        missing.result.is_none(),
        "missing required group should fail"
    );
    assert!(
        !missing.diagnostics.is_empty(),
        "missing required group should report diagnostics, got {:?}",
        missing.diagnostics
    );

    let wrong_form = ctx.parse(r"\reqgrp|x|", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        wrong_form.result.is_none(),
        "non-braced required group should fail"
    );
    assert!(
        !wrong_form.diagnostics.is_empty(),
        "non-braced required group should report diagnostics, got {:?}",
        wrong_form.diagnostics
    );
}

#[test]
fn test_group_form_supports_dimension_kind() {
    let ctx = test_context_with_items([command_item(
        "gdim",
        CommandKind::Prefix,
        AllowedMode::Math,
        "g:L",
    )]);

    let missing = ctx.parse(r"\gdim", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        missing.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        missing.diagnostics
    );
    let missing_node = missing
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match missing_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "gdim");
                assert_eq!(args.len(), 1);
                assert!(args[0].is_none(), "group slot should be None when absent");
            }
            other => panic!("Expected gdim command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }

    let present = ctx.parse(r"\gdim{1.5em}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        present.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        present.diagnostics
    );
    let present_node = present
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match present_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "gdim");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Dimension("1.5em".to_string())
                );
            }
            other => panic!("Expected gdim command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_required_group_form_composes_with_star_and_standard_slots() {
    let ctx = test_context_with_items([command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "s m{} m",
    )]);

    let basic = ctx.parse(r"\probe{A}B", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        basic.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        basic.diagnostics
    );
    let (name, args) = extract_first_command(
        basic
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(name, "probe");
    assert_eq!(args.len(), 3);
    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Star);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
    assert_eq!(expect_arg(&args[1]).kind, ArgumentKind::Group);
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('A'))
    );
    assert_eq!(expect_arg(&args[2]).kind, ArgumentKind::Mandatory);
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('B'))
    );

    let starred = ctx.parse(r"\probe*{A}B", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        starred.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        starred.diagnostics
    );
    let (_, starred_args) = extract_first_command(
        starred
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(
        expect_arg(&starred_args[0]).value,
        ArgumentValue::Boolean(true)
    );
    assert_eq!(expect_arg(&starred_args[1]).kind, ArgumentKind::Group);
    assert_eq!(expect_arg(&starred_args[2]).kind, ArgumentKind::Mandatory);

    let missing = ctx.parse(r"\probe B", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        missing.result.is_none(),
        "missing required group slot should fail"
    );
    assert!(
        !missing.diagnostics.is_empty(),
        "missing required group slot should report diagnostics, got {:?}",
        missing.diagnostics
    );
}

#[test]
fn test_group_form_supports_delimiter_kind() {
    let ctx = test_context_with_items([command_item(
        "gdelim",
        CommandKind::Prefix,
        AllowedMode::Math,
        "g:D",
    )]);

    let output = ctx.parse(r"\gdelim{|}", &ParseConfig::STRICT_NO_RECOVER);
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
                assert_eq!(name, "gdelim");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Delimiter(Delimiter::Char('|'))
                );
            }
            other => panic!("Expected gdelim command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_nullable_delimiter_argument_accepts_empty_required_group() {
    let ctx = test_context_with_items(vec![
        ContextItem::from(command_item(
            "ndelim",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:D?",
        )),
        ContextItem::from(command_item(
            "strictdelim",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:D",
        )),
        ContextItem::from(DelimiterControlItem::new("langle")),
    ]);

    let empty = ctx.parse(r"\ndelim{}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        empty.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        empty.diagnostics
    );
    let (_, empty_args) = extract_first_command(
        empty
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(
        expect_arg(&empty_args[0]).value,
        ArgumentValue::Delimiter(Delimiter::None)
    );

    let explicit = ctx.parse(r"\ndelim\langle", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        explicit.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        explicit.diagnostics
    );
    let (_, explicit_args) = extract_first_command(
        explicit
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(
        expect_arg(&explicit_args[0]).value,
        ArgumentValue::Delimiter(Delimiter::Control("langle"))
    );

    let strict_empty = ctx.parse(r"\strictdelim{}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        strict_empty.result.is_none(),
        "non-nullable delimiter should reject empty braces"
    );
    assert!(
        !strict_empty.diagnostics.is_empty(),
        "non-nullable delimiter should report diagnostics"
    );
}

#[test]
fn test_nullable_delimiter_group_accepts_empty_group() {
    let ctx = test_context_with_items([command_item(
        "gdelimnull",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m{}:D?",
    )]);

    let output = ctx.parse(r"\gdelimnull{}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = extract_first_command(
        output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Delimiter(Delimiter::None)
    );
}

#[test]
fn test_required_group_and_delimited_forms_have_distinct_ast_kinds() {
    let ctx = test_context_with_items([
        command_item("reqgrp", CommandKind::Prefix, AllowedMode::Math, "m{}"),
        command_item("reqdelim", CommandKind::Prefix, AllowedMode::Math, "r{}"),
    ]);

    let group = ctx.parse(r"\reqgrp{x}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        group.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        group.diagnostics
    );
    let (_, group_args) = extract_first_command(
        group
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(expect_arg(&group_args[0]).kind, ArgumentKind::Group);

    let delimited = ctx.parse(r"\reqdelim{x}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        delimited.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        delimited.diagnostics
    );
    let (_, delimited_args) = extract_first_command(
        delimited
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    match expect_arg(&delimited_args[0]).kind {
        ArgumentKind::Delimited { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("Expected delimited argument kind, got {:?}", other),
    }
}

#[test]
fn test_mqty_supports_star_plus_optional_paired_slot() {
    let (starred, _) = parse(r"\mqty*|x|", false).unwrap();
    let (name, args) = extract_first_command(starred);
    assert_eq!(name, "mqty");
    assert_eq!(args.len(), 2);
    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Star);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
    match expect_arg(&args[1]).kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('|'));
            assert_eq!(close, Delimiter::Char('|'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }

    let (missing, _) = parse(r"\mqty*", false).unwrap();
    let (_, missing_args) = extract_first_command(missing);
    assert_eq!(missing_args.len(), 2);
    assert!(missing_args[1].is_none(), "paired slot should be optional");
}

#[test]
fn test_dd_supports_optional_then_paired_slots() {
    let (basic, _) = parse(r"\dd{x}", false).unwrap();
    let (name, args) = extract_first_command(basic);
    assert_eq!(name, "dd");
    assert_eq!(args.len(), 2);
    assert!(args[0].is_none(), "optional bracket slot should be None");
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
    match expect_arg(&args[1]).kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }

    let (with_opt, _) = parse(r"\dd[y](x)", false).unwrap();
    let (_, args_with_opt) = extract_first_command(with_opt);
    assert_eq!(
        expect_arg(&args_with_opt[0]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('y'))
    );
    match expect_arg(&args_with_opt[1]).kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('('));
            assert_eq!(close, Delimiter::Char(')'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }

    let (unmatched, _) = parse(r"\dd[y]|x|", false).unwrap();
    match unmatched {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "dd");
                    assert_eq!(args.len(), 2);
                    assert!(
                        args[1].is_none(),
                        "non-candidate delimiter should not be consumed"
                    );
                }
                other => panic!("Expected dd command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('|'));
            assert_eq!(children[2], SyntaxNode::Char('x'));
            assert_eq!(children[3], SyntaxNode::Char('|'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_paired_form_required_vs_optional_semantics() {
    let ctx = test_context_with_items([
        command_item("mustpair", CommandKind::Prefix, AllowedMode::Math, "r<(,)>"),
        command_item(
            "maybepair",
            CommandKind::Prefix,
            AllowedMode::Math,
            "d<(,)>",
        ),
    ]);

    let required_ok = ctx.parse(r"\mustpair(x)", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        required_ok.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        required_ok.diagnostics
    );
    assert!(
        required_ok.result.is_some(),
        "required paired arg should parse"
    );

    let required_missing = ctx.parse(r"\mustpair", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        required_missing.result.is_none(),
        "missing required paired arg should fail"
    );
    assert!(
        !required_missing.diagnostics.is_empty(),
        "missing required paired arg should report diagnostics, got {:?}",
        required_missing.diagnostics
    );

    let optional_unmatched = ctx.parse(r"\maybepair[x]", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        optional_unmatched.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        optional_unmatched.diagnostics
    );
    let node = optional_unmatched
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "maybepair");
                    assert_eq!(args.len(), 1);
                    assert!(args[0].is_none(), "optional paired slot should stay empty");
                }
                other => panic!("Expected maybepair command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('['));
            assert_eq!(children[2], SyntaxNode::Char('x'));
            assert_eq!(children[3], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_newline_command_preserves_no_leading_space_behavior() {
    let ctx = test_context();

    let immediate = ctx.parse(r"\newline*[1cm]", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        immediate.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        immediate.diagnostics
    );
    let immediate_node = immediate
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
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

    let spaced = ctx.parse(r"\newline * [1cm]", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        spaced.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        spaced.diagnostics
    );
    let spaced_node = spaced
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
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

#[test]
fn test_environment_star_in_name_is_independent_from_s_arg_slot() {
    let ctx = test_context_with_items([
        environment_item("probenv", AllowedMode::Math, ContentMode::Math, "s"),
        environment_item("probenv*", AllowedMode::Math, ContentMode::Math, "s"),
    ]);

    let starred_name = ctx.parse(
        r"\begin{probenv*}x\end{probenv*}",
        &ParseConfig::STRICT_NO_RECOVER,
    );
    assert!(
        starred_name.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        starred_name.diagnostics
    );
    let starred_name_node = starred_name
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    assert!(
        matches!(starred_name_node, SyntaxNode::Root { .. }),
        "expected root node"
    );
    match starred_name_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Environment { name, args, .. } => {
                assert_eq!(name, "probenv*");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
            }
            other => panic!("Expected environment node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }

    let star_arg = ctx.parse(
        r"\begin{probenv}*x\end{probenv}",
        &ParseConfig::STRICT_NO_RECOVER,
    );
    assert!(
        star_arg.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        star_arg.diagnostics
    );
    let node = star_arg
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Environment {
                name, args, body, ..
            } => {
                assert_eq!(name, "probenv");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Star);
                assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
                match &**body {
                    SyntaxNode::Group { children, .. } => {
                        assert_eq!(children.len(), 1);
                        assert_eq!(children[0], SyntaxNode::Char('x'));
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

    let output = ctx.parse(r"\probe(\frac{1}{2})", &ParseConfig::STRICT_NO_RECOVER);
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
fn test_parse_context_isolation_for_custom_commands() {
    let ctx1 = test_context_with_items([command_item(
        "fooisolated",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m",
    )]);

    let out1_foo = ctx1.parse(r"\fooisolated{a}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(out1_foo.diagnostics.is_empty());
    assert!(out1_foo.result.is_some());

    let out1_bar = ctx1.parse(r"\barisolated{a}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(!out1_bar.diagnostics.is_empty());
    assert!(out1_bar.result.is_none());

    let ctx2 = test_context_with_items([command_item(
        "barisolated",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m",
    )]);

    let out2_bar = ctx2.parse(r"\barisolated{a}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(out2_bar.diagnostics.is_empty());
    assert!(out2_bar.result.is_some());

    let out2_foo = ctx2.parse(r"\fooisolated{a}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(!out2_foo.diagnostics.is_empty());
    assert!(out2_foo.result.is_none());
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
