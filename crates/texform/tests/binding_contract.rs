use texform::{
    ArgSpecFormInfo, ArgSpecKindInfo, DelimiterTokenInfo, RuntimeContentModeInfo, TransformEngine,
    validate_argspec,
};

#[test]
fn validate_argspec_success_returns_stable_slot_shape() {
    let result = validate_argspec("m:T? !s r() R<(,)>{x} m:D?");

    assert!(result.valid);
    assert_eq!(result.error, None);
    assert_eq!(result.arg_count, Some(5));
    let parsed = result
        .parsed
        .expect("valid argspec should include parsed slots");
    assert_eq!(parsed.len(), 5);

    assert!(parsed[0].required);
    assert!(!parsed[0].no_leading_space);
    assert!(parsed[0].nullable);
    assert_eq!(
        parsed[0].kind,
        ArgSpecKindInfo::Content {
            mode: RuntimeContentModeInfo::Text,
        }
    );
    assert_eq!(parsed[0].form, ArgSpecFormInfo::Standard);

    assert!(!parsed[1].required);
    assert!(parsed[1].no_leading_space);
    assert_eq!(parsed[1].kind, ArgSpecKindInfo::Star);
    assert_eq!(parsed[1].form, ArgSpecFormInfo::Star);

    let ArgSpecFormInfo::Delimited { open, close } = &parsed[2].form else {
        panic!("expected delimited form");
    };
    assert_eq!(open, &DelimiterTokenInfo::Char { value: '(' });
    assert_eq!(close, &DelimiterTokenInfo::Char { value: ')' });

    assert_eq!(
        parsed[3].kind,
        ArgSpecKindInfo::Content {
            mode: RuntimeContentModeInfo::Math,
        }
    );
    let ArgSpecFormInfo::Paired { pairs } = &parsed[3].form else {
        panic!("expected paired form");
    };
    assert_eq!(pairs.len(), 1);

    assert_eq!(parsed[4].kind, ArgSpecKindInfo::Delimiter);
    assert!(parsed[4].nullable);

    let star_slot = serde_json::to_value(&parsed[1]).unwrap();
    assert_eq!(star_slot["kind"], serde_json::json!({ "type": "star" }));
    assert_eq!(star_slot["form"], serde_json::json!({ "type": "star" }));
}

#[test]
fn validate_argspec_reports_operator_name_kind() {
    let result = validate_argspec("m:O");

    assert!(result.valid);
    let parsed = result
        .parsed
        .expect("valid argspec should include parsed slots");
    assert_eq!(parsed[0].kind, ArgSpecKindInfo::OperatorName);

    let slot = serde_json::to_value(&parsed[0]).unwrap();
    assert_eq!(slot["kind"], serde_json::json!({ "type": "operatorname" }));
}

#[test]
fn validate_argspec_failure_keeps_all_fields_present() {
    let result = validate_argspec("m?");

    assert!(!result.valid);
    assert!(
        result
            .error
            .as_deref()
            .is_some_and(|message| message.contains("invalid argspec"))
    );
    assert_eq!(result.arg_count, None);
    assert_eq!(result.parsed, None);
}

#[test]
fn transform_report_dto_serializes_as_snake_case() {
    let mut report = texform::TransformReport::default();
    report
        .finalize_ast
        .steps
        .merge_adjacent_primes
        .applied_count = 1;
    report
        .finalize_ast
        .steps
        .normalize_text_sequences
        .applied_count = 3;

    let value = serde_json::to_value(texform::bindings::transform_report_to_dto(&report)).unwrap();

    assert!(value.get("finalize_ast").is_some());
    assert!(value.get("finalizeAst").is_none());
    assert_eq!(
        value["finalize_ast"]["steps"]["merge_adjacent_primes"]["applied_count"],
        1
    );
    assert_eq!(
        value["finalize_ast"]["steps"]["normalize_text_sequences"]["applied_count"],
        3
    );
}

#[test]
fn tokenized_latex_dto_uses_snake_case_byte_offsets_and_closed_values() {
    let parser = texform::Parser::builder().build().unwrap();
    let document = parser.parse(r"\text{\%𝒜}").try_into_document().unwrap().0;
    let result = document.to_tokenized_latex().unwrap();
    let value = serde_json::to_value(texform::bindings::tokenized_latex_to_dto(result)).unwrap();

    assert!(
        value["tokens"]
            .as_array()
            .is_some_and(|tokens| !tokens.is_empty())
    );
    assert!(value["tokens"][0].get("start_byte").is_some());
    assert!(value["tokens"][0].get("startByte").is_none());
    assert!(value["tokens"].as_array().unwrap().iter().all(|token| {
        matches!(
            token["kind"].as_str(),
            Some("control_sequence" | "character" | "delimiter" | "text" | "raw" | "error")
        ) && matches!(token["mode"].as_str(), Some("math" | "text"))
    }));
    let unicode = value["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|token| token["text"] == "𝒜")
        .expect("unicode token should be present");
    assert_eq!(
        unicode["end_byte"].as_u64().unwrap() - unicode["start_byte"].as_u64().unwrap(),
        4
    );
}

#[test]
fn lookup_info_dto_reuses_stable_argspec_slots() {
    let parser = texform::Parser::builder().build().unwrap();
    let record = parser
        .lookup_command("frac", texform::ContentMode::Math)
        .expect("default parser should know frac");

    let value = serde_json::to_value(texform::bindings::command_info_to_dto(record)).unwrap();

    assert_eq!(value["name"], "frac");
    assert_eq!(value["allowed_mode"], "math");
    assert!(
        value["spec_string"]
            .as_str()
            .is_some_and(|spec| !spec.is_empty())
    );
    assert!(
        value["from_packages"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        value["args"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(value["args"][0].get("no_leading_space").is_some());
}

#[test]
fn normalize_defaults_preserve_unknown_commands_in_all_profiles() {
    for profile in [
        texform::Profile::Authoring,
        texform::Profile::Faithful,
        texform::Profile::Corpus,
        texform::Profile::Equiv,
    ] {
        let engine = TransformEngine::builder().profile(profile).build().unwrap();
        let result = engine
            .normalize("\\unknown")
            .expect("default normalize should preserve unknown commands");

        assert_eq!(result.normalized, "\\unknown", "profile: {profile:?}");
    }
}

#[test]
fn normalize_error_maps_parse_failure_to_binding_error_parts() {
    let engine = TransformEngine::builder()
        .profile(texform::Profile::Authoring)
        .build()
        .unwrap();

    let error = match engine.normalize("{") {
        Ok(_) => panic!("default normalize should reject malformed input"),
        Err(error) => error,
    };
    let parts = texform::bindings::normalize_error_to_parts(error);

    assert_eq!(parts.error.kind, "parse");
    assert!(!parts.error.diagnostics.is_empty());
}

#[test]
fn list_packages_reports_known_packages_with_counts() {
    let packages = texform::list_packages();
    assert!(!packages.is_empty());

    for expected in ["base", "ams", "physics"] {
        let info = packages
            .iter()
            .find(|info| info.name == expected)
            .unwrap_or_else(|| panic!("package {expected} should be listed"));
        assert!(info.commands > 0, "{expected} should have commands");
    }

    let base = packages.iter().find(|info| info.name == "base").unwrap();
    assert!(base.environments > 0, "base should have environments");
}
