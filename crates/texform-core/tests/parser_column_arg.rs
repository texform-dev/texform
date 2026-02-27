use std::sync::Once;

use texform_core::knowledge::{self, AllowedMode, ArgSpec, CommandKind, ValueKind};
use texform_core::parser::parse;
use texform_interface::syntax_node::{ArgumentValue, SyntaxNode};

fn init_inline_column_command() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut builder = knowledge::KnowledgeBase::builder();
        for &pkg_name in texform_specs::packages::TEST_DEFAULT_PACKAGES {
            let pkg = texform_specs::packages::get(pkg_name)
                .unwrap_or_else(|| panic!("unknown package: {}", pkg_name));
            builder.import_package((pkg.load)());
        }
        builder.insert_or_override_command(
            "colspec",
            CommandKind::Prefix,
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
    init_inline_column_command();
    let result = parse(r"\colspec{a}", false);
    assert!(result.is_err());
}
