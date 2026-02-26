use std::sync::Once;

use texform_core::knowledge::{self, AllowedMode, ArgSpec, CommandKind, ValueKind};
use texform_core::parser::parse;
use texform_interface::syntax_node::{ArgumentValue, SyntaxNode};

fn init_inline_column_command() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut builder = knowledge::KnowledgeBase::builder();
        for pkg in texform_specs::packages::ALL_PACKAGES {
            builder.import_package((pkg.load)());
        }
        builder.insert_or_override_command(
            "colspec",
            CommandKind::Prefix,
            false,
            AllowedMode::Math,
            vec![ArgSpec::new(true, ValueKind::Column)],
            vec![],
        );
        knowledge::init_with_builder(builder);
    });
}

#[test]
fn parse_column_arg_success() {
    init_inline_column_command();
    let (result, _) = parse(r"\colspec{c|c|c}", false).unwrap();

    match result {
        SyntaxNode::Group { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "colspec");
                assert_eq!(args.len(), 1);
                match &args[0].value {
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
    init_inline_column_command();
    let result = parse(r"\colspec{a}", false);
    assert!(result.is_err());
}
