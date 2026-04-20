use texform_core::api::parse_with_context_items;
use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, ParseContext, ParseContextBuilder,
    ParseOutput,
};
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, Delimiter, SyntaxNode,
};

fn command_item(
    name: &str,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    spec: &str,
) -> ContextItem {
    CommandItem::new(name, kind, allowed_mode, spec).into()
}

fn parse_single(items: &[ContextItem], src: &str, strict: bool) -> ParseOutput {
    let mut outputs = parse_with_context_items(items, &[src], None, strict);
    assert_eq!(outputs.len(), 1);
    outputs.remove(0).output
}

fn text_command_item() -> ContextItem {
    command_item("text", CommandKind::Prefix, AllowedMode::Math, "m:T")
}

fn frac_command_item() -> ContextItem {
    command_item("frac", CommandKind::Prefix, AllowedMode::Math, "m m")
}

fn underline_math_item() -> ContextItem {
    command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m")
}

fn underline_text_item() -> ContextItem {
    command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T")
}

fn content_test_context() -> ParseContext {
    ParseContextBuilder::new()
        .core_only()
        .insert_item(text_command_item())
        .insert_item(frac_command_item())
        .insert_item(underline_math_item())
        .insert_item(underline_text_item())
        .build()
        .expect("content test context should build")
}

fn expect_arg(slot: &Option<Argument>) -> &Argument {
    slot.as_ref()
        .unwrap_or_else(|| panic!("expected argument slot to be present"))
}

fn first_command(output: &ParseOutput) -> (&str, &Vec<Option<Argument>>) {
    let result = output
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"));
    match &result.node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => (name.as_str(), args),
            other => panic!("expected command node, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

fn collect_messages(output: &ParseOutput) -> Vec<&str> {
    output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

fn assert_first_diagnostic_span_eq(output: &ParseOutput, src: &str, expected: &str) {
    let diagnostic = output
        .diagnostics
        .first()
        .expect("expected at least one diagnostic");
    assert_eq!(&src[diagnostic.span.start..diagnostic.span.end], expected);
}

fn slot_contains_command_named(slot: &Option<Argument>, expected: &str) -> bool {
    slot.as_ref().is_some_and(|arg| match &arg.value {
        ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
            contains_command_named(node, expected)
        }
        _ => false,
    })
}

fn contains_command_named(node: &SyntaxNode, expected: &str) -> bool {
    match node {
        SyntaxNode::Command { name, .. } if name == expected => true,
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => children
            .iter()
            .any(|child| contains_command_named(child, expected)),
        SyntaxNode::Command { args, .. } => args
            .iter()
            .any(|slot| slot_contains_command_named(slot, expected)),
        SyntaxNode::Declarative { args, .. } => args
            .iter()
            .any(|slot| slot_contains_command_named(slot, expected)),
        SyntaxNode::Environment { args, body, .. } => {
            args.iter()
                .any(|slot| slot_contains_command_named(slot, expected))
                || contains_command_named(body, expected)
        }
        SyntaxNode::Infix {
            args, left, right, ..
        } => {
            args.iter()
                .any(|slot| slot_contains_command_named(slot, expected))
                || contains_command_named(left, expected)
                || contains_command_named(right, expected)
        }
        _ => false,
    }
}

fn slot_contains_error(slot: &Option<Argument>) -> bool {
    slot.as_ref().is_some_and(|arg| match &arg.value {
        ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
            contains_error_node(node)
        }
        _ => false,
    })
}

fn contains_error_node(node: &SyntaxNode) -> bool {
    match node {
        SyntaxNode::Error { .. } => true,
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => {
            children.iter().any(contains_error_node)
        }
        SyntaxNode::Command { args, .. } => args.iter().any(slot_contains_error),
        SyntaxNode::Declarative { args, .. } => args.iter().any(slot_contains_error),
        SyntaxNode::Environment { args, body, .. } => {
            args.iter().any(slot_contains_error) || contains_error_node(body)
        }
        SyntaxNode::Infix {
            args, left, right, ..
        } => {
            args.iter().any(slot_contains_error)
                || contains_error_node(left)
                || contains_error_node(right)
        }
        _ => false,
    }
}

#[test]
fn integer_argument_is_verified_via_public_parser() {
    let items = [command_item(
        "romannumeral",
        CommandKind::Prefix,
        AllowedMode::Both,
        "m:I",
    )];

    let valid = parse_single(&items, r"\romannumeral+42", true);
    assert!(
        valid.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        valid.diagnostics
    );
    let (name, args) = first_command(&valid);
    assert_eq!(name, "romannumeral");
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Integer("+42".to_string())
    );

    let invalid = parse_single(&items, r"\romannumeral+", true);
    assert!(invalid.result.is_none(), "invalid integer should fail");
    assert!(
        !invalid.diagnostics.is_empty(),
        "expected integer diagnostics"
    );
}

#[test]
fn dimension_argument_is_verified_via_public_parser() {
    let items = [command_item(
        "hspace",
        CommandKind::Prefix,
        AllowedMode::Both,
        "m:L",
    )];

    let valid = parse_single(&items, r"\hspace{1,5 em}", true);
    assert!(
        valid.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        valid.diagnostics
    );
    let (name, args) = first_command(&valid);
    assert_eq!(name, "hspace");
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Dimension("1.5em".to_string())
    );

    let invalid = parse_single(&items, r"\hspaceabc", true);
    assert!(invalid.result.is_none(), "invalid dimension should fail");
    assert!(
        !invalid.diagnostics.is_empty(),
        "expected dimension diagnostics"
    );
}

#[test]
fn keyval_argument_accepts_nested_and_escaped_shapes() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];
    let outputs = parse_with_context_items(
        &items,
        &[
            r"\includegraphics[key=val]{file}",
            r"\includegraphics[key={a,b},other=c]{file}",
            r"\includegraphics[key=\{,other=c]{file}",
        ],
        None,
        true,
    );

    let expected = ["key=val", "key={a,b},other=c", r"key=\{,other=c"];

    for (item, expected_keyval) in outputs.iter().zip(expected) {
        assert!(
            item.output.diagnostics.is_empty(),
            "unexpected diagnostics for {}: {:?}",
            item.input,
            item.output.diagnostics
        );
        let (_, args) = first_command(&item.output);
        assert_eq!(
            expect_arg(&args[0]).value,
            ArgumentValue::KeyVal(expected_keyval.to_string())
        );
    }
}

#[test]
fn keyval_argument_rejects_invalid_shapes() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];
    let outputs = parse_with_context_items(
        &items,
        &[
            r"\includegraphics[key=]{file}",
            r"\includegraphics[=value]{file}",
            r"\includegraphics[key={a]{file}",
        ],
        None,
        true,
    );

    for item in &outputs {
        assert!(item.output.result.is_none(), "{} should fail", item.input);
        assert!(
            !item.output.diagnostics.is_empty(),
            "expected diagnostics for {}",
            item.input
        );
    }
}

#[test]
fn keyval_argument_diagnostic_span_covers_bracket_argument() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];
    let src = r"\includegraphics[key=]{file}";

    let output = parse_single(&items, src, true);
    assert!(output.result.is_none(), "invalid keyval should fail");

    let diagnostic = output
        .diagnostics
        .first()
        .expect("expected keyval diagnostic");
    assert_eq!(diagnostic.message, "keyval missing value");
    assert_eq!(&src[diagnostic.span.start..diagnostic.span.end], "[key=]");
}

#[test]
fn optional_bracket_content_stops_at_top_level_closer() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];

    let output = parse_single(&items, r"\includegraphics[key={[[},width=1em]{file}", true);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = first_command(&output);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::KeyVal("key={[[},width=1em".to_string())
    );
}

#[test]
fn delimited_argument_collects_nested_content() {
    let items = [command_item(
        "reqdelim",
        CommandKind::Prefix,
        AllowedMode::Math,
        "r{}",
    )];

    let output = parse_single(&items, r"\reqdelim{a{b}c}", true);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = first_command(&output);
    match expect_arg(&args[0]).kind {
        ArgumentKind::Delimited { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("expected delimited argument kind, got {:?}", other),
    }
}

#[test]
fn mandatory_argument_normalizes_single_explicit_group() {
    let items = [command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m",
    )];

    let output = parse_single(&items, r"\probe{x}", true);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = first_command(&output);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
}

#[test]
fn text_content_generic_only_error_keeps_expected_found_diagnostic() {
    let items = [text_command_item()];

    let output = parse_single(&items, r"\text{$x}", true);

    assert!(
        output.result.is_none(),
        "strict mode should not keep a partial result"
    );
    assert_eq!(
        collect_messages(&output),
        vec!["found '$' expected something else, or end of input"]
    );
    assert!(
        !output.diagnostics[0].expected.is_empty(),
        "expected/found details should stay available"
    );
}

#[test]
fn strict_text_content_command_error_points_to_inner_command() {
    let src = r"\text{\frac{a}{b}}";
    let items = [text_command_item(), frac_command_item()];

    let output = parse_single(&items, src, true);

    assert_eq!(
        collect_messages(&output),
        vec![r"Command \frac is not allowed in text mode"]
    );
    assert_first_diagnostic_span_eq(&output, src, r"\frac");
}

#[test]
fn strict_text_content_command_error_has_no_partial_result() {
    let items = [text_command_item(), frac_command_item()];

    let output = parse_single(&items, r"\text{\frac{a}{b}}", true);

    assert!(
        output.result.is_none(),
        "strict content argument errors should not keep a partial result"
    );
    assert_eq!(
        collect_messages(&output),
        vec![r"Command \frac is not allowed in text mode"]
    );
}

#[test]
fn nonstrict_text_content_direct_error_survives_trailing_generic() {
    let ctx = content_test_context();
    let output = ctx.parse(r"\text{\underline{a^2}$}", false);

    assert_eq!(
        collect_messages(&output),
        vec!["Scripted syntax is not allowed in Text mode"]
    );

    let result = output
        .result
        .as_ref()
        .expect("non-strict direct error should keep a partial result");
    assert!(contains_command_named(&result.node, "text"));
    assert!(contains_command_named(&result.node, "underline"));
    assert!(contains_error_node(&result.node));
}
