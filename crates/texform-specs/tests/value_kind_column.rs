use texform_specs::specs::{ValueKind, load_package_specs_from_str};

#[test]
fn parse_column_kind_from_yaml() {
    let yaml = r#"
commands:
  - name: colspec
    kind: prefix
    args:
      - required: true
        kind: column
"#;

    let specs = load_package_specs_from_str(yaml, "column-kind");
    assert_eq!(specs.commands.len(), 1);
    assert_eq!(specs.commands[0].args.len(), 1);
    assert_eq!(specs.commands[0].args[0].kind, ValueKind::Column);
}
