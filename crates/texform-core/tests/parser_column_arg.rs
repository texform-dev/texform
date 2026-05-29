use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ParseContextBuilder, ParseResult,
};
use texform_interface::syntax_node::{ArgumentValue, SyntaxNode};

fn parse_inline_column_command(src: &str) -> ParseResult {
    let ctx = ParseContextBuilder::empty()
        .insert_item(CommandItem::new(
            "colspec",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:C",
        ))
        .build()
        .expect("colspec argspec should be valid");
    ctx.parse(src, &texform_core::parse::ParseConfig::default())
}

#[test]
fn parse_column_arg_success() {
    let output = parse_inline_column_command(r"\colspec{c|c|c}");
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let result = output
        .document()
        .expect("column argument parse should succeed");

    match &result.to_syntax() {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "colspec");
                assert_eq!(args.len(), 1);
                match &args[0]
                    .as_ref()
                    .unwrap_or_else(|| panic!("expected colspec argument"))
                    .value
                {
                    ArgumentValue::Column(value) => {
                        assert_eq!(value, "c|c|c");
                    }
                    _ => panic!("Expected Column argument"),
                }
            }
            _ => panic!("Expected Command node"),
        },
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn parse_column_arg_invalid_template_errors() {
    let output = parse_inline_column_command(r"\colspec{a}");
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn parse_column_arg_uses_shared_dimension_unit_set() {
    let output = parse_inline_column_command(r"\colspec{p{1mu}}");
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let invalid = parse_inline_column_command(r"\colspec{p{1zz}}");
    assert!(
        invalid.document().is_none(),
        "column argument with unknown dimension unit should fail"
    );
    assert!(
        !invalid.diagnostics.is_empty(),
        "expected diagnostics for invalid column dimension"
    );
}
