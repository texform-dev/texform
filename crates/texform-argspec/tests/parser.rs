use std::borrow::Cow;

use texform_argspec::{ArgForm, ArgSpec, ContentMode, DelimiterToken, ValueKind, parse_arg_specs};

#[test]
fn test_parse_arg_specs_xparse_style() {
    let specs = parse_arg_specs("s o m g", "xparse").expect("s o m g should be valid");
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
fn test_parse_arg_specs_supports_required_group_form() {
    let specs = parse_arg_specs("m m{} m{}:T m{}:L", "required-group")
        .expect("required group argspec should be valid");
    assert_eq!(specs.len(), 4);

    assert_eq!(specs[0], ArgSpec::mandatory(ContentMode::Math));

    assert!(specs[1].required);
    assert_eq!(specs[1].form, ArgForm::Group);
    assert_eq!(
        specs[1].kind,
        ValueKind::Content {
            mode: ContentMode::Math
        }
    );

    assert!(specs[2].required);
    assert_eq!(specs[2].form, ArgForm::Group);
    assert_eq!(
        specs[2].kind,
        ValueKind::Content {
            mode: ContentMode::Text
        }
    );

    assert!(specs[3].required);
    assert_eq!(specs[3].form, ArgForm::Group);
    assert_eq!(specs[3].kind, ValueKind::Dimension);
}

#[test]
fn test_parse_arg_specs_pairs_and_delimited() {
    let specs = parse_arg_specs(
        "r<(,)><[,]><{,}><|,|> d<(,|><[,|><{,}> r() d[] r{}",
        "pairs",
    )
    .expect("paired/delimited argspec should be valid");
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
    let specs = parse_arg_specs("m:T o:K m:L m:I m:C m:D m:D? !s !o:L", "types")
        .expect("typed argspec should be valid");
    assert_eq!(specs.len(), 9);

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
    assert_eq!(specs[6].kind, ValueKind::Delimiter);
    assert!(specs[6].nullable);

    assert!(specs[7].no_leading_space);
    assert_eq!(specs[7].form, ArgForm::Star);
    assert_eq!(specs[7].kind, ValueKind::Star);

    assert!(specs[8].no_leading_space);
    assert_eq!(specs[8].kind, ValueKind::Dimension);
    assert_eq!(specs[8].form, ArgForm::Standard);
}

#[test]
fn test_parse_arg_specs_nullable_delimiter_annotation() {
    let specs = parse_arg_specs("m:D? o:D? g:D? m{}:D?", "nullable-delimiter")
        .expect("nullable delimiter argspec should be valid");
    assert_eq!(specs.len(), 4);

    for spec in specs {
        assert_eq!(spec.kind, ValueKind::Delimiter);
        assert!(spec.nullable);
    }
}

#[test]
fn test_parse_arg_specs_accepts_nullable_dimension_annotation() {
    let specs = parse_arg_specs("m:L?", "nullable-dim").expect("m:L? should be valid");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].kind, ValueKind::Dimension);
    assert!(specs[0].nullable);
    assert!(specs[0].required);
}

#[test]
fn test_parse_arg_specs_accepts_nullable_for_all_non_star_non_column_kinds() {
    // All value kinds except Star and Column support nullable
    for spec_str in &["m:I?", "m:N?", "m:K?", "m:T?"] {
        let specs = parse_arg_specs(spec_str, "nullable-generalized")
            .unwrap_or_else(|_| panic!("{} should be valid", spec_str));
        assert_eq!(specs.len(), 1);
        assert!(specs[0].nullable);
    }
}

#[test]
fn test_parse_arg_specs_rejects_nullable_star_and_column() {
    let err = parse_arg_specs("m:C?", "invalid").expect_err("m:C? should be invalid");
    assert!(
        err.to_string()
            .contains("`?` is not supported for star or column annotations")
    );
}

#[test]
fn test_parse_arg_specs_supports_csname_annotation() {
    let specs = parse_arg_specs("m:N o:N g:N m{}:N", "csname-types")
        .expect("CSName argspec should be valid");
    assert_eq!(specs.len(), 4);

    assert_eq!(specs[0].kind, ValueKind::CSName);
    assert_eq!(specs[0].form, ArgForm::Standard);
    assert!(specs[0].required);

    assert_eq!(specs[1].kind, ValueKind::CSName);
    assert_eq!(specs[1].form, ArgForm::Standard);
    assert!(!specs[1].required);

    assert_eq!(specs[2].kind, ValueKind::CSName);
    assert_eq!(specs[2].form, ArgForm::Group);
    assert!(!specs[2].required);

    assert_eq!(specs[3].kind, ValueKind::CSName);
    assert_eq!(specs[3].form, ArgForm::Group);
    assert!(specs[3].required);
}

#[test]
fn test_parse_arg_specs_uppercase_default_variants() {
    let specs = parse_arg_specs(
        r"!O{1cm}:L !G{a=b}:K D<(,)><[,]>{10}:I R\langle\rangle{fallback}:T",
        "uppercase-default-variants",
    )
    .expect("uppercase variants with defaults should be valid");
    assert_eq!(specs.len(), 4);

    assert!(!specs[0].required);
    assert!(specs[0].no_leading_space);
    assert_eq!(specs[0].form, ArgForm::Standard);
    assert_eq!(specs[0].kind, ValueKind::Dimension);

    assert!(!specs[1].required);
    assert!(specs[1].no_leading_space);
    assert_eq!(specs[1].form, ArgForm::Group);
    assert_eq!(specs[1].kind, ValueKind::KeyVal);

    assert!(!specs[2].required);
    assert_eq!(specs[2].kind, ValueKind::Integer);
    match &specs[2].form {
        ArgForm::Paired { pairs } => {
            assert_eq!(pairs.len(), 2);
            assert_eq!(
                pairs[0],
                (DelimiterToken::Char('('), DelimiterToken::Char(')'))
            );
            assert_eq!(
                pairs[1],
                (DelimiterToken::Char('['), DelimiterToken::Char(']'))
            );
        }
        other => panic!("expected paired form, got {:?}", other),
    }

    assert!(specs[3].required);
    assert_eq!(
        specs[3].kind,
        ValueKind::Content {
            mode: ContentMode::Text
        }
    );
    match &specs[3].form {
        ArgForm::Delimited { open, close } => {
            assert!(matches!(
                open,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "langle"
            ));
            assert!(matches!(
                close,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "rangle"
            ));
        }
        other => panic!("expected delimited form, got {:?}", other),
    }
}

#[test]
fn test_parse_arg_specs_uppercase_defaults_allow_nested_braces() {
    let specs = parse_arg_specs(r"O{#1{\alpha}\{x\}}", "uppercase-nested-default")
        .expect("nested default value should be ignored");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0], ArgSpec::optional(ContentMode::Math));
}

#[test]
fn test_parse_arg_specs_rejects_uppercase_without_default_block() {
    let err = parse_arg_specs("O", "invalid").expect_err("O without default should be invalid");
    assert!(
        err.to_string()
            .contains("`O` requires a default block like `{...}`")
    );

    let err = parse_arg_specs("D()", "invalid").expect_err("D() without default should be invalid");
    assert!(
        err.to_string()
            .contains("`D` requires a default block like `{...}`")
    );
}

#[test]
fn test_runtime_argspec_uses_owned_storage_for_dynamic_tokens() {
    let specs = parse_arg_specs(r"d<\langle,\rangle>", "owned-runtime")
        .expect("control-seq delimiter argspec should be valid");
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
fn test_parse_arg_specs_rejects_required_with_no_space_prefix() {
    let err = parse_arg_specs("!m", "invalid").expect_err("!m should be invalid");
    assert!(
        err.to_string()
            .contains("`!` prefix is only valid for optional argument forms")
    );

    let err = parse_arg_specs("!m{}", "invalid").expect_err("!m{} should be invalid");
    assert!(
        err.to_string()
            .contains("`!` prefix is only valid for optional argument forms")
    );
}

#[test]
fn test_parse_arg_specs_rejects_delimiter_kind_with_paired() {
    let err = parse_arg_specs("d():D", "invalid").expect_err("d():D should be invalid");
    assert!(
        err.to_string()
            .contains("delimiter kind cannot use delimited/paired form")
    );
}

#[test]
fn test_parse_arg_specs_accepts_group_with_non_content_kind() {
    let specs = parse_arg_specs("g:L g:I g:K g:C g:D", "group-kinds")
        .expect("group form should allow non-content kinds");
    assert_eq!(specs.len(), 5);
    assert_eq!(specs[0].form, ArgForm::Group);
    assert_eq!(specs[0].kind, ValueKind::Dimension);
    assert_eq!(specs[1].kind, ValueKind::Integer);
    assert_eq!(specs[2].kind, ValueKind::KeyVal);
    assert_eq!(specs[3].kind, ValueKind::Column);
    assert_eq!(specs[4].kind, ValueKind::Delimiter);
}

#[test]
fn test_parse_arg_specs_rejects_malformed_required_group_form() {
    let err = parse_arg_specs("m{foo}", "invalid").expect_err("m{foo} should be invalid");
    assert!(
        err.to_string()
            .contains("`m` only supports required braced group syntax `m{}`")
    );

    let err = parse_arg_specs("m {}", "invalid").expect_err("m {} should be invalid");
    assert!(err.to_string().contains("unsupported argument token `{`"));
}

#[test]
fn test_parse_arg_specs_rejects_star_annotation() {
    let err = parse_arg_specs("s:T", "invalid").expect_err("s:T should be invalid");
    assert!(
        err.to_string()
            .contains("`s` does not accept value type annotation")
    );
}

#[test]
fn test_parse_arg_specs_required_flags_for_paired_and_delimited_forms() {
    let specs = parse_arg_specs("r<(,)> d<(,)> r{} d[]", "required-flags")
        .expect("paired and delimited argspec should be valid");
    assert_eq!(specs.len(), 4);

    assert!(specs[0].required, "r<...> should be required");
    assert!(!specs[1].required, "d<...> should be optional");
    assert!(specs[2].required, "r should be required");
    assert!(!specs[3].required, "d should be optional");

    assert!(matches!(specs[0].form, ArgForm::Paired { .. }));
    assert!(matches!(specs[1].form, ArgForm::Paired { .. }));
    assert!(matches!(specs[2].form, ArgForm::Delimited { .. }));
    assert!(matches!(specs[3].form, ArgForm::Delimited { .. }));
}

#[test]
fn test_parse_arg_specs_supports_control_sequence_delimited_form() {
    let specs = parse_arg_specs(r"r\langle\rangle d\lvert\rvert", "control-delimited")
        .expect("control-sequence delimited argspec should be valid");
    assert_eq!(specs.len(), 2);

    match &specs[0].form {
        ArgForm::Delimited { open, close } => {
            assert!(matches!(
                open,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "langle"
            ));
            assert!(matches!(
                close,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "rangle"
            ));
        }
        other => panic!("expected delimited form, got {:?}", other),
    }

    match &specs[1].form {
        ArgForm::Delimited { open, close } => {
            assert!(matches!(
                open,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "lvert"
            ));
            assert!(matches!(
                close,
                DelimiterToken::ControlSeq(Cow::Owned(name)) if name == "rvert"
            ));
        }
        other => panic!("expected delimited form, got {:?}", other),
    }
}

#[test]
fn test_parse_arg_specs_rejects_empty_pair_list() {
    let err =
        parse_arg_specs("r<>", "invalid").expect_err("r with empty pair list should be invalid");
    assert!(
        err.to_string()
            .contains("`<`, `>`, `,` are reserved in pair syntax")
    );
}

#[test]
fn test_parse_arg_specs_rejects_whitespace_pair_delimiter() {
    let err = parse_arg_specs("d< ,)>", "invalid").expect_err("whitespace pair delimiter");
    assert!(
        err.to_string()
            .contains("pair delimiter cannot be whitespace")
    );
}

#[test]
fn test_parse_arg_specs_rejects_required_paired_with_no_space_prefix() {
    let err = parse_arg_specs("!r<(,)>", "invalid").expect_err("!r<...> should be invalid");
    assert!(
        err.to_string()
            .contains("`!` prefix is only valid for optional argument forms")
    );
}
