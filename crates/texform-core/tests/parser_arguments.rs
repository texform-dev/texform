use texform_core::api::parse_with_context_items;
use texform_core::context::{AllowedMode, CommandItem, CommandKind, ContextItem, ParseOutput};
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
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args } => (name.as_str(), args),
            other => panic!("expected command node, got {:?}", other),
        },
        other => panic!("expected root group, got {:?}", other),
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
        ArgumentValue::Content(SyntaxNode::Char('x'))
    );
}
