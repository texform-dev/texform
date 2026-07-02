use texform_argspec::{ContentMode, ValueKind};
use texform_knowledge::builtin::{ALL_PACKAGES, MANAGED_PACKAGE_IMPORT_ORDER, PackageName};
use texform_knowledge::specs::{AllowedMode, load_package_specs_from_str};

#[test]
fn package_name_import_order_matches_transform_order() {
    assert_eq!(
        MANAGED_PACKAGE_IMPORT_ORDER,
        &[
            PackageName::Base,
            PackageName::Ams,
            PackageName::Braket,
            PackageName::Physics,
            PackageName::Textmacros,
            PackageName::Bboldx,
            PackageName::Boldsymbol,
        ]
    );
    assert_eq!(
        MANAGED_PACKAGE_IMPORT_ORDER
            .iter()
            .map(|package| package.as_str())
            .collect::<Vec<_>>(),
        vec![
            "base",
            "ams",
            "braket",
            "physics",
            "textmacros",
            "bboldx",
            "boldsymbol",
        ]
    );
}

#[test]
fn package_name_lookup_and_package_access_use_generated_registry() {
    assert_eq!(PackageName::from_str("physics"), Some(PackageName::Physics));
    assert_eq!(PackageName::from_str("missing"), None);
    assert_eq!(PackageName::Physics.package().name, "physics");
}

#[test]
fn all_resource_specs_are_registered() {
    let names: Vec<&str> = ALL_PACKAGES.iter().map(|pkg| pkg.name).collect();
    assert_eq!(texform_knowledge::builtin::all_package_names(), names);
    assert_eq!(
        names,
        vec![
            "ams",
            "base",
            "bboldx",
            "boldsymbol",
            "braket",
            "physics",
            "textmacros"
        ]
    );
}

#[test]
fn registered_packages_expose_builtin_records() {
    for package in ALL_PACKAGES {
        let is_empty = package.characters.is_empty()
            && package.delimiters.is_empty()
            && package.commands.is_empty()
            && package.environments.is_empty();
        assert!(!is_empty, "package {} should not be empty", package.name);
    }
}

#[test]
fn non_alpha_commands_expose_representative_facades() {
    use texform_knowledge::builtin::{base, braket, textmacros};

    assert_eq!(base::cmd::_CONTROL_SPACE.name, " ");
    assert_eq!(base::cmd::_BACKSLASH.name, "\\");
    assert_eq!(braket::cmd::_VERTICAL_BAR.name, "|");
    assert_eq!(textmacros::cmd::_PERIOD.name, ".");
}

#[test]
fn test_load_package_specs_from_str() {
    let yaml = r#"
characters:
  - name: alpha
    allowed_mode: math
    unicode_value: α
    attributes:
      mathvariant: italic
  - name: beta
    allowed_mode: text
    unicode_value: β
    attributes: {}
commands:
  - name: frac
    kind: prefix
    allowed_mode: math
    tags: [discouraged]
    argspec: "m m:D"
  - name: text
    kind: prefix
    allowed_mode: both
    argspec: "m:T"
environments:
  - name: matrix
    allowed_mode: math
    body_mode: math
    tags: [matrix]
    argspec: "m:C"
delimiters:
  - name: langle
    is_control_sequence: true
    allowed_mode: math
    unicode_value: ⟨
    attributes:
      tex_class: OPEN
  - name: "|"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "|"
    attributes: {}
"#;

    let specs = load_package_specs_from_str(yaml, "test");
    assert_eq!(specs.characters.len(), 2);
    assert_eq!(specs.characters[0].name, "alpha");
    assert_eq!(specs.characters[0].allowed_mode, AllowedMode::Math);
    assert_eq!(specs.characters[0].unicode_value, "α");
    assert_eq!(
        specs.characters[0].attributes.mathvariant.as_deref(),
        Some("italic")
    );
    assert_eq!(specs.characters[1].name, "beta");
    assert_eq!(specs.characters[1].allowed_mode, AllowedMode::Text);
    assert_eq!(specs.characters[1].unicode_value, "β");
    assert_eq!(specs.characters[1].attributes.mathvariant, None);

    assert_eq!(specs.commands.len(), 2);
    assert_eq!(specs.commands[0].name, "frac");
    assert_eq!(specs.commands[0].allowed_mode, AllowedMode::Math);
    assert_eq!(specs.commands[0].argspec.args.len(), 2);
    assert_eq!(specs.commands[0].tags, vec!["discouraged"]);
    assert!(specs.commands[0].argspec.args[0].required);
    assert_eq!(
        specs.commands[0].argspec.args[0].kind,
        ValueKind::Content {
            mode: ContentMode::Math
        }
    );
    assert_eq!(specs.commands[0].argspec.args[1].kind, ValueKind::Delimiter);

    assert_eq!(specs.commands[1].name, "text");
    assert_eq!(specs.commands[1].allowed_mode, AllowedMode::Both);
    assert_eq!(specs.commands[1].argspec.args.len(), 1);
    assert!(specs.commands[1].tags.is_empty());
    assert_eq!(
        specs.commands[1].argspec.args[0].kind,
        ValueKind::Content {
            mode: ContentMode::Text
        }
    );

    assert_eq!(specs.environments.len(), 1);
    assert_eq!(specs.environments[0].name, "matrix");
    assert_eq!(specs.environments[0].allowed_mode, AllowedMode::Math);
    assert_eq!(specs.environments[0].argspec.args.len(), 1);
    assert_eq!(specs.environments[0].tags, vec!["matrix"]);
    assert_eq!(specs.delimiters.len(), 2);
    assert_eq!(specs.delimiters[0].name, "langle");
    assert!(specs.delimiters[0].is_control_sequence);
    assert_eq!(specs.delimiters[0].allowed_mode, AllowedMode::Math);
    assert_eq!(specs.delimiters[0].unicode_value, "⟨");
    assert_eq!(
        specs.delimiters[0].attributes.tex_class.as_deref(),
        Some("OPEN")
    );
    assert_eq!(specs.delimiters[1].name, "|");
    assert!(!specs.delimiters[1].is_control_sequence);
    assert_eq!(specs.delimiters[1].unicode_value, "|");
}

#[test]
fn test_parse_column_kind_from_yaml() {
    let yaml = r#"
commands:
  - name: colspec
    kind: prefix
    argspec: "m:C"
"#;

    let specs = load_package_specs_from_str(yaml, "column-kind");
    assert_eq!(specs.commands.len(), 1);
    assert_eq!(specs.commands[0].argspec.args.len(), 1);
    assert_eq!(specs.commands[0].argspec.args[0].kind, ValueKind::Column);
}

#[test]
fn test_command_allowed_mode_defaults_to_both() {
    let yaml = r#"
commands:
  - name: foo
    kind: prefix
"#;

    let specs = load_package_specs_from_str(yaml, "default-allowed-mode");
    assert_eq!(specs.commands.len(), 1);
    assert_eq!(specs.commands[0].allowed_mode, AllowedMode::Both);
    assert!(specs.commands[0].argspec.args.is_empty());
}

#[test]
#[should_panic(expected = "missing field `allowed_mode`")]
fn test_character_allowed_mode_is_required() {
    let yaml = r#"
characters:
  - name: alpha
    unicode_value: α
    attributes: {}
"#;

    let _ = load_package_specs_from_str(yaml, "character-allowed-mode-required");
}

#[test]
fn test_environment_body_mode_can_be_text() {
    let yaml = r#"
environments:
  - name: textenv
    allowed_mode: math
    body_mode: text
"#;

    let specs = load_package_specs_from_str(yaml, "test");
    assert_eq!(specs.environments.len(), 1);
    assert_eq!(specs.environments[0].name, "textenv");
    assert_eq!(specs.environments[0].body_mode, ContentMode::Text);
}

#[test]
#[should_panic(expected = "missing field `allowed_mode`")]
fn test_environment_allowed_mode_is_required() {
    let yaml = r#"
environments:
  - name: matrix
    body_mode: math
"#;

    let _ = load_package_specs_from_str(yaml, "environment-allowed-mode-required");
}

#[test]
fn test_allowed_mode_helpers() {
    assert!(AllowedMode::Math.allows(ContentMode::Math));
    assert!(!AllowedMode::Math.allows(ContentMode::Text));
    assert!(AllowedMode::Text.allows(ContentMode::Text));
    assert!(!AllowedMode::Text.allows(ContentMode::Math));
    assert!(AllowedMode::Both.allows(ContentMode::Math));
    assert!(AllowedMode::Both.allows(ContentMode::Text));

    assert_eq!(
        AllowedMode::Math.union(AllowedMode::Math),
        AllowedMode::Math
    );
    assert_eq!(
        AllowedMode::Text.union(AllowedMode::Text),
        AllowedMode::Text
    );
    assert_eq!(
        AllowedMode::Math.union(AllowedMode::Text),
        AllowedMode::Both
    );
    assert_eq!(
        AllowedMode::Text.union(AllowedMode::Math),
        AllowedMode::Both
    );
    assert_eq!(
        AllowedMode::Both.union(AllowedMode::Math),
        AllowedMode::Both
    );
}
