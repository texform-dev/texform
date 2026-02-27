use std::borrow::Cow;

use texform_specs::specs::{
    AllowedMode, ArgForm, ArgSpec, ContentMode, DelimiterToken, ValueKind,
    load_package_specs_from_str, parse_arg_specs,
};

#[test]
fn test_parse_arg_specs_xparse_style() {
    let specs = parse_arg_specs("s o m g", "xparse");
    assert_eq!(specs.len(), 4);

    assert_eq!(specs[0].form, ArgForm::Star);
    assert_eq!(specs[0].kind, ValueKind::Star);
    assert!(!specs[0].required);

    assert_eq!(specs[1], ArgSpec::optional(ContentMode::Math));
    assert_eq!(specs[2], ArgSpec::mandatory(ContentMode::Math));
    assert_eq!(specs[3].form, ArgForm::Group);
    assert_eq!(
        specs[3].kind,
        ValueKind::Content {
            mode: ContentMode::Math
        }
    );
}

#[test]
fn test_parse_arg_specs_pairs_and_delimited() {
    let specs = parse_arg_specs(
        "P<(,)><[,]><{,}><|,|> p<(,|><[,|><{,}> r() d[] r{}",
        "pairs",
    );
    assert_eq!(specs.len(), 5);

    match &specs[0].form {
        ArgForm::Paired { pairs } => {
            assert_eq!(pairs.len(), 4);
            assert_eq!(
                pairs[0],
                (DelimiterToken::Char('('), DelimiterToken::Char(')'))
            );
            assert_eq!(
                pairs[1],
                (DelimiterToken::Char('['), DelimiterToken::Char(']'))
            );
            assert_eq!(
                pairs[2],
                (DelimiterToken::Char('{'), DelimiterToken::Char('}'))
            );
            assert_eq!(
                pairs[3],
                (DelimiterToken::Char('|'), DelimiterToken::Char('|'))
            );
        }
        other => panic!("expected paired form, got {:?}", other),
    }

    match &specs[1].form {
        ArgForm::Paired { pairs } => {
            assert_eq!(pairs.len(), 3);
            assert_eq!(
                pairs[0],
                (DelimiterToken::Char('('), DelimiterToken::Char('|'))
            );
            assert_eq!(
                pairs[1],
                (DelimiterToken::Char('['), DelimiterToken::Char('|'))
            );
            assert_eq!(
                pairs[2],
                (DelimiterToken::Char('{'), DelimiterToken::Char('}'))
            );
        }
        other => panic!("expected paired form, got {:?}", other),
    }

    match &specs[2].form {
        ArgForm::Delimited { open, close } => {
            assert_eq!(open, &DelimiterToken::Char('('));
            assert_eq!(close, &DelimiterToken::Char(')'));
        }
        other => panic!("expected delimited form, got {:?}", other),
    }

    match &specs[3].form {
        ArgForm::Delimited { open, close } => {
            assert_eq!(open, &DelimiterToken::Char('['));
            assert_eq!(close, &DelimiterToken::Char(']'));
        }
        other => panic!("expected delimited form, got {:?}", other),
    }

    match &specs[4].form {
        ArgForm::Delimited { open, close } => {
            assert_eq!(open, &DelimiterToken::Char('{'));
            assert_eq!(close, &DelimiterToken::Char('}'));
        }
        other => panic!("expected delimited form, got {:?}", other),
    }
}

#[test]
fn test_parse_arg_specs_type_annotations() {
    let specs = parse_arg_specs("m:T o:K m:L m:I m:C m:D !s !o:L", "types");
    assert_eq!(specs.len(), 8);

    assert_eq!(
        specs[0].kind,
        ValueKind::Content {
            mode: ContentMode::Text
        }
    );
    assert_eq!(specs[1].kind, ValueKind::KeyVal);
    assert_eq!(specs[2].kind, ValueKind::Dimension);
    assert_eq!(specs[3].kind, ValueKind::Integer);
    assert_eq!(specs[4].kind, ValueKind::Column);
    assert_eq!(specs[5].kind, ValueKind::Delimiter);

    assert!(specs[6].no_leading_space);
    assert_eq!(specs[6].form, ArgForm::Star);
    assert_eq!(specs[6].kind, ValueKind::Star);

    assert!(specs[7].no_leading_space);
    assert_eq!(specs[7].kind, ValueKind::Dimension);
    assert_eq!(specs[7].form, ArgForm::Standard);
}

#[test]
fn test_runtime_argspec_uses_owned_storage_for_dynamic_tokens() {
    let specs = parse_arg_specs(r"p<\langle,\rangle>", "owned-runtime");
    assert_eq!(specs.len(), 1);

    match &specs[0].form {
        ArgForm::Paired { pairs } => {
            assert!(matches!(pairs, Cow::Owned(_)));
            assert_eq!(pairs.len(), 1);
            assert!(matches!(
                &pairs[0].0,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "langle"
            ));
            assert!(matches!(
                &pairs[0].1,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "rangle"
            ));
        }
        other => panic!("expected paired form, got {:?}", other),
    }
}

#[test]
#[should_panic(expected = "`!` prefix is only valid for optional argument forms")]
fn test_parse_arg_specs_rejects_required_with_no_space_prefix() {
    let _ = parse_arg_specs("!m", "invalid");
}

#[test]
#[should_panic(expected = "delimiter kind cannot use delimited/paired form")]
fn test_parse_arg_specs_rejects_delimiter_kind_with_paired() {
    let _ = parse_arg_specs("d():D", "invalid");
}

#[test]
#[should_panic(expected = "group form only supports content kind")]
fn test_parse_arg_specs_rejects_group_non_content_kind() {
    let _ = parse_arg_specs("g:L", "invalid");
}

#[test]
#[should_panic(expected = "`s` does not accept value type annotation")]
fn test_parse_arg_specs_rejects_star_annotation() {
    let _ = parse_arg_specs("s:T", "invalid");
}

#[test]
fn test_load_package_specs_from_str() {
    let yaml = r#"
characters:
  - name: alpha
    allowed_mode: math
  - name: beta
    allowed_mode: text
commands:
  - name: frac
    kind: prefix
    allowed_mode: math
    tags: [discouraged]
    spec: "m m:D"
  - name: text
    kind: prefix
    allowed_mode: both
    spec: "m:T"
environments:
  - name: matrix
    allowed_mode: math
    body_mode: math
    tags: [matrix]
    spec: "m:C"
delimiter_controls: [langle]
"#;

    let specs = load_package_specs_from_str(yaml, "test");
    assert_eq!(specs.characters.len(), 2);
    assert_eq!(specs.characters[0].name, "alpha");
    assert_eq!(specs.characters[0].allowed_mode, AllowedMode::Math);
    assert_eq!(specs.characters[1].name, "beta");
    assert_eq!(specs.characters[1].allowed_mode, AllowedMode::Text);

    assert_eq!(specs.commands.len(), 2);
    assert_eq!(specs.commands[0].name, "frac");
    assert_eq!(specs.commands[0].allowed_mode, AllowedMode::Math);
    assert_eq!(specs.commands[0].args.len(), 2);
    assert_eq!(specs.commands[0].tags, vec!["discouraged"]);
    assert_eq!(specs.commands[0].args[0].required, true);
    assert_eq!(
        specs.commands[0].args[0].kind,
        ValueKind::Content {
            mode: ContentMode::Math
        }
    );
    assert_eq!(specs.commands[0].args[1].kind, ValueKind::Delimiter);

    assert_eq!(specs.commands[1].name, "text");
    assert_eq!(specs.commands[1].allowed_mode, AllowedMode::Both);
    assert_eq!(specs.commands[1].args.len(), 1);
    assert!(specs.commands[1].tags.is_empty());
    assert_eq!(
        specs.commands[1].args[0].kind,
        ValueKind::Content {
            mode: ContentMode::Text
        }
    );

    assert_eq!(specs.environments.len(), 1);
    assert_eq!(specs.environments[0].name, "matrix");
    assert_eq!(specs.environments[0].allowed_mode, AllowedMode::Math);
    assert_eq!(specs.environments[0].args.len(), 1);
    assert_eq!(specs.environments[0].tags, vec!["matrix"]);
    assert_eq!(specs.delimiter_controls, vec!["langle"]);
}

#[test]
fn test_parse_column_kind_from_yaml() {
    let yaml = r#"
commands:
  - name: colspec
    kind: prefix
    spec: "m:C"
"#;

    let specs = load_package_specs_from_str(yaml, "column-kind");
    assert_eq!(specs.commands.len(), 1);
    assert_eq!(specs.commands[0].args.len(), 1);
    assert_eq!(specs.commands[0].args[0].kind, ValueKind::Column);
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
    assert!(specs.commands[0].args.is_empty());
}

#[test]
#[should_panic(expected = "missing field `allowed_mode`")]
fn test_character_allowed_mode_is_required() {
    let yaml = r#"
characters:
  - name: alpha
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

    assert_eq!(AllowedMode::Math.to_string(), "math");
    assert_eq!(AllowedMode::Text.to_string(), "text");
    assert_eq!(AllowedMode::Both.to_string(), "both");
}
