use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ParseContextBuilder, ParseOutput,
};
use texform_interface::syntax_node::{ArgumentValue, SyntaxNode};

fn parse_inline_column_command(src: &str) -> ParseOutput {
    let ctx = ParseContextBuilder::new()
        .core_only()
        .insert_item(CommandItem::new(
            "colspec",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:C",
        ))
        .build()
        .expect("colspec argspec should be valid");
    ctx.parse(src, false)
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
        .result
        .as_ref()
        .expect("column argument parse should succeed");

    match &result.node {
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
