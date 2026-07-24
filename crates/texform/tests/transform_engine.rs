use texform::{
    ContentMode, FlattenGroupsConfig, NormalizeConfig, ParseConfig, Parser, Profile,
    TransformConfig, TransformEngine, bindings::transform_report_to_dto,
};
use texform_transform::FinalizeAstConfig;

#[test]
fn engine_normalize_uses_build_time_profile_and_packages() {
    let engine = TransformEngine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Authoring)
        .build()
        .expect("engine should build");

    let result = engine
        .normalize(r"\quantity{x}")
        .expect("normalize should succeed");

    assert_eq!(result.normalized, r"\qty { x }");
    assert!(!result.report.rewrite.rules.is_empty());
}

#[test]
fn normalize_with_can_disable_rewrite_without_rebuilding_plan() {
    let engine = TransformEngine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Equiv)
        .build()
        .expect("engine should build");

    let result = engine
        .normalize_with(
            r"\quantity{x}",
            &NormalizeConfig {
                parse: ParseConfig::STRICT,
                transform: TransformConfig {
                    rewrite_enabled: false,
                    lower_attributes_enabled: false,
                    finalize_ast: FinalizeAstConfig::ENABLED,
                    flatten_groups: FlattenGroupsConfig::DISABLED,
                    max_iterations: 100,
                },
            },
        )
        .expect("normalize should succeed");

    assert_eq!(result.normalized, r"\quantity { x }");
    assert_eq!(result.report.rewrite.iterations, 0);
}

#[test]
fn corpus_normalize_preserves_prime_and_prefix_shorthand_contracts() {
    let engine = TransformEngine::builder()
        .packages(&["base"])
        .profile(Profile::Corpus)
        .build()
        .expect("engine should build");

    let cases = [
        ("U'", "U'"),
        ("H'", "H'"),
        (r"A^{'\alpha}", r"A ^ { ' \alpha }"),
        (r"\vec A_\mu", r"\vec { A } _ { \mu }"),
        (r"\bar C^\mu", r"\bar { C } ^ { \mu }"),
        (r"f^{\prime\prime}", "f''"),
        (r"f^{'}", "f'"),
        (r"f'^2", r"f ^ { ' 2 }"),
        (r"\prime", "'"),
    ];

    for (input, expected) in cases {
        let result = engine
            .normalize(input)
            .unwrap_or_else(|error| panic!("normalize should succeed for {input}: {error:?}"));
        assert_eq!(result.normalized, expected, "input: {input}");
    }
}

#[test]
fn corpus_normalize_keeps_braced_prefix_argument_scope() {
    let engine = TransformEngine::builder()
        .packages(&["base"])
        .profile(Profile::Corpus)
        .build()
        .expect("engine should build");

    let result = engine
        .normalize(r"\vec{A_\mu}")
        .expect("normalize should succeed");

    assert_eq!(result.normalized, r"\vec { A _ { \mu } }");
}

#[test]
fn displaylines_is_preserved_by_all_profiles() {
    let input = r"\displaylines{a \cr b}";

    for profile in [
        Profile::Authoring,
        Profile::Faithful,
        Profile::Corpus,
        Profile::Equiv,
    ] {
        let result = TransformEngine::builder()
            .packages(&["base", "ams"])
            .profile(profile)
            .build()
            .expect("engine should build")
            .normalize(input)
            .expect("normalize should succeed");
        assert!(
            result.normalized.contains(r"\displaylines"),
            "{profile:?} output: {}",
            result.normalized
        );
    }
}

#[test]
fn document_transform_preserves_parse_once_workflow() {
    let engine = TransformEngine::builder()
        .packages(&["base"])
        .profile(Profile::Equiv)
        .build()
        .expect("engine should build");

    let mut document = engine
        .parser()
        .parse("{{x}}")
        .try_into_document()
        .expect("parse should succeed")
        .0;
    let before = document.to_latex().expect("document should serialize");

    let report = engine
        .transform_with(
            &mut document,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform should succeed");
    let after = document.to_latex().expect("document should serialize");

    assert_eq!(before, "{ { x } }");
    assert_eq!(after, "x");
    assert_eq!(report.flatten_groups.actions.replaced_single_child, 2);
}

#[test]
fn parser_is_parse_only_and_needs_no_profile() {
    let parser = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");

    let output = parser.parse(r"\frac{a}{b}");
    assert!(output.diagnostics().is_empty());
}

#[test]
fn engine_exposes_parser_metadata_queries() {
    let engine = TransformEngine::builder()
        .packages(&["base"])
        .profile(Profile::Authoring)
        .build()
        .expect("engine should build");

    let parser = engine.parser();
    assert!(parser.lookup_command("frac", ContentMode::Math).is_some());
    assert!(
        parser
            .lookup_explicit_command("frac", ContentMode::Math)
            .is_some()
    );
    assert!(parser.lookup_env("array", ContentMode::Math).is_some());
    assert!(parser.lookup_character("le", ContentMode::Math).is_some());
    assert!(parser.is_delimiter_control("lbrace"));
    assert!(parser.knows_command_name("frac"));
    assert!(parser.knows_env_name("array"));
    assert!(parser.knows_character_name("le"));
}

#[test]
fn engine_empty_knowledge_preserves_default_parse_config() {
    // Empty knowledge must not change either default parse-config axis.
    let engine = TransformEngine::builder()
        .empty_knowledge()
        .profile(Profile::Equiv)
        .build()
        .expect("engine should build");

    let unknown = engine.parser().parse(r"\unknowncmd");
    let malformed = engine.parser().parse("{");

    assert!(
        unknown.diagnostics().is_empty(),
        "default config should preserve unknown commands"
    );
    assert!(
        malformed.document().is_some(),
        "default config should retain a recovery tree"
    );
    assert!(!malformed.diagnostics().is_empty(), "diagnostics expected");
}

#[test]
fn engine_empty_knowledge_preserves_explicit_parse_default() {
    // Empty knowledge should only change loaded knowledge, not caller-selected
    // parse defaults.
    let engine = TransformEngine::builder()
        .default_parse_config(ParseConfig::LENIENT)
        .empty_knowledge()
        .profile(Profile::Equiv)
        .build()
        .expect("engine should build");

    let output = engine.parser().parse(r"\unknowncmd");

    assert!(
        output.diagnostics().is_empty(),
        "empty_knowledge should preserve default_parse_config set earlier"
    );
}

#[test]
fn engine_builder_requires_profile() {
    let error = match TransformEngine::builder().packages(&["base"]).build() {
        Ok(_) => panic!("engine profile is required"),
        Err(error) => error,
    };

    assert!(matches!(error, texform::Error::MissingProfile));
}

#[test]
fn engine_builder_disable_rule_without_profile_reports_error() {
    let error = match TransformEngine::builder()
        .disable_rule_by_name("physics/quantity-to-qty")
        .expect("known rule should resolve")
        .build()
    {
        Ok(_) => panic!("engine profile is required"),
        Err(error) => error,
    };

    assert!(matches!(error, texform::Error::MissingProfile));
}

#[test]
fn engine_builder_disables_rule_by_public_name() {
    let engine = TransformEngine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Authoring)
        .disable_rule_by_name("physics/quantity-to-qty")
        .expect("known rule should resolve")
        .build()
        .expect("engine should build");

    let result = engine
        .normalize(r"\quantity{x}")
        .expect("normalize should succeed");

    assert_eq!(result.normalized, r"\quantity { x }");

    let unknown = TransformEngine::builder()
        .profile(Profile::Authoring)
        .disable_rule_by_name("missing.rule");
    assert!(
        unknown.is_err(),
        "unknown rule names should fail at the facade"
    );
}

#[test]
fn engine_builder_disable_rule_can_precede_profile() {
    let engine = TransformEngine::builder()
        .packages(&["base", "physics"])
        .disable_rule_by_name("physics/quantity-to-qty")
        .expect("known rule should resolve")
        .profile(Profile::Authoring)
        .build()
        .expect("engine should build");

    let result = engine
        .normalize(r"\quantity{x}")
        .expect("normalize should succeed");

    assert_eq!(result.normalized, r"\quantity { x }");
}

#[test]
fn normalize_report_dto_exposes_stable_phase_shape() {
    let engine = TransformEngine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Authoring)
        .build()
        .expect("engine should build");

    let result = engine
        .normalize(r"\quantity{{\bf x}}")
        .expect("normalize should succeed");
    let dto = transform_report_to_dto(&result.report);

    let quantity_rule = dto
        .rules
        .iter()
        .find(|rule| rule.key == "physics/quantity-to-qty")
        .expect("rewrite rules should expose stable rule entries");
    assert_eq!(quantity_rule.applied_count, 1);
    assert_eq!(quantity_rule.skipped_count, 0);

    assert_eq!(
        dto.flatten_groups.actions.replaced_single_child,
        result.report.flatten_groups.actions.replaced_single_child
    );
    assert_eq!(
        dto.flatten_groups.guards.preserve_empty_group,
        result.report.flatten_groups.guards.preserve_empty_group
    );

    let math_font = dto
        .lower_attributes
        .attributes
        .iter()
        .find(|attribute| attribute.attr == "math_font" && attribute.value == "bold")
        .expect("lower attributes should expose stable attribute entries");
    assert_eq!(math_font.consumed.declaratives, 1);
    assert!(math_font.emitted.prefixes > 0);

    let json = serde_json::to_value(&dto).expect("report DTO should serialize");
    assert!(json.get("rules").is_some());
    assert!(json.get("applied").is_none());
    assert!(json["rules"][0].get("applied_count").is_some());
    assert!(json["rules"][0].get("count").is_none());
    assert!(json["flatten_groups"].get("actions").is_some());
    assert!(json["flatten_groups"].get("guards").is_some());
    assert!(
        json["flatten_groups"]
            .get("preserved_group_containing_declarative_command")
            .is_none()
    );
    assert!(
        json["flatten_groups"]["guards"]
            .get("preserve_group_containing_declarative_command")
            .is_some()
    );
    assert!(json["lower_attributes"].get("attributes").is_some());
}
