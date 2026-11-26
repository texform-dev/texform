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
fn validate_argspec_reports_control_sequence_and_real_kinds() {
    let result = validate_argspec("m:CS m:R? m:C");

    assert!(result.valid);
    let parsed = result
        .parsed
        .expect("valid argspec should include parsed slots");
    let kinds: Vec<_> = parsed
        .iter()
        .map(|slot| serde_json::to_value(&slot.kind).unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            serde_json::json!({ "type": "controlsequence" }),
            serde_json::json!({ "type": "real" }),
            serde_json::json!({ "type": "column" }),
        ]
    );
    assert!(parsed[1].nullable);
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
    let mut report = texform::diagnostics::TransformReport::default();
    report.finalize_ast.prime_run_merges = 1;
    report.finalize_ast.text_normalizations = 3;
    report.rewrite.iterations = 2;

    let value = serde_json::to_value(texform::bindings::transform_report_to_dto(&report)).unwrap();

    assert!(value.get("finalize_ast").is_some());
    assert!(value.get("finalizeAst").is_none());
    assert!(value.get("rewrite").is_some());
    assert!(value.get("iterations").is_none());
    assert_eq!(value["finalize_ast"]["prime_run_merges"], 1);
    assert_eq!(value["finalize_ast"]["text_normalizations"], 3);
    assert_eq!(value["rewrite"]["iterations"], 2);
    assert!(value["flatten_groups"].get("guard_hits").is_some());
    assert!(value["flatten_groups"].get("guards").is_none());
}

#[test]
fn fragmented_latex_dto_uses_snake_case_byte_offsets_and_closed_values() {
    let parser = texform::Parser::builder().build();
    let document = parser.parse(r"\text{\%𝒜}").try_into_document().unwrap().0;
    let result = document.to_latex_fragments().unwrap();
    let value = serde_json::to_value(texform::bindings::fragmented_latex_to_dto(result)).unwrap();

    assert!(
        value["fragments"]
            .as_array()
            .is_some_and(|fragments| !fragments.is_empty())
    );
    assert!(value["fragments"][0].get("start_byte").is_some());
    assert!(value["fragments"][0].get("startByte").is_none());
    assert!(value["fragments"][0].get("node_path").is_some());
    assert!(value["fragments"][0].get("nodePath").is_none());
    assert!(value["fragments"][0].get("slot").is_some());
    assert_eq!(value["fragments"][0]["slot"], serde_json::Value::Null);
    assert_eq!(
        value["fragments"][1]["slot"],
        serde_json::json!({ "kind": "arg", "index": 0 })
    );
    assert!(
        value["fragments"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fragment| {
                let slot_valid = fragment["slot"].is_null()
                    || (matches!(
                        fragment["slot"]["kind"].as_str(),
                        Some(
                            "child"
                                | "arg"
                                | "script_base"
                                | "subscript"
                                | "superscript"
                                | "infix_left"
                                | "infix_right"
                                | "env_body"
                        )
                    ) && (fragment["slot"]["index"].is_null()
                        || fragment["slot"]["index"].is_u64()));
                matches!(
                    fragment["kind"].as_str(),
                    Some("control_sequence" | "character" | "delimiter" | "text" | "raw" | "error")
                ) && matches!(
                    fragment["role"].as_str(),
                    Some(
                        "head"
                            | "open"
                            | "close"
                            | "name"
                            | "operator"
                            | "delimiter"
                            | "value"
                            | "leaf"
                    )
                ) && fragment["node_path"]
                    .as_str()
                    .is_some_and(|path| document.node_at(path).is_ok())
                    && slot_valid
                    && matches!(fragment["mode"].as_str(), Some("math" | "text"))
            })
    );
    let unicode = value["fragments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fragment| fragment["text"] == "𝒜")
        .expect("unicode fragment should be present");
    assert_eq!(
        unicode["end_byte"].as_u64().unwrap() - unicode["start_byte"].as_u64().unwrap(),
        4
    );
}

#[test]
fn lookup_info_dto_reuses_stable_argspec_slots() {
    let parser = texform::Parser::builder().build();
    let record = parser
        .knowledge_base()
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

        assert_eq!(result, "\\unknown", "profile: {profile:?}");
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

#[test]
fn default_packages_are_what_an_unconfigured_knowledge_base_loads() {
    let defaults = texform::default_packages();
    assert_eq!(defaults, texform::KnowledgeBase::default().packages());
    assert!(!defaults.contains(&"physics"), "physics should be opt-in");
}

#[test]
fn list_rules_reports_metadata_for_disableable_keys() {
    let rules = texform::list_rules();
    let over = rules
        .iter()
        .find(|rule| rule.key.to_string() == "base/over-to-frac")
        .expect("base/over-to-frac should be listed");

    assert_eq!(over.level, texform::RuleLevel::Authoring);
    assert_eq!(over.enabled_by_packages, ["base"]);
    assert!(!over.summary.is_empty());
    for rule in &rules {
        let name = rule.key.to_string();
        assert_eq!(texform::rule_key_from_name(&name), Some(rule.key), "{name}");
    }
}
