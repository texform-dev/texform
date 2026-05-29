use texform::{
    ContentMode, Engine, FlattenGroupsConfig, NormalizeConfig, ParseConfig, Parser, Profile,
    TransformConfig,
};

#[test]
fn engine_normalize_uses_build_time_profile_and_packages() {
    let engine = Engine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Authoring)
        .build()
        .expect("engine should build");

    let result = engine
        .normalize(r"\quantity{x}")
        .expect("normalize should succeed");

    assert_eq!(result.normalized, r"\qty { x }");
    assert!(!result.report.rewrite.applied.is_empty());
}

#[test]
fn normalize_with_can_disable_rewrite_without_rebuilding_plan() {
    let engine = Engine::builder()
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
fn document_transform_preserves_parse_once_workflow() {
    let engine = Engine::builder()
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
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform should succeed");
    let after = document.to_latex().expect("document should serialize");

    assert_eq!(before, "{ { x } }");
    assert_eq!(after, "x");
    assert_eq!(report.flatten_groups.replaced_single_child, 2);
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
    let engine = Engine::builder()
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
fn engine_empty_knowledge_preserves_strict_parse_default() {
    // Empty knowledge must not loosen the Engine parser's strict default.
    let engine = Engine::builder()
        .empty_knowledge()
        .profile(Profile::Equiv)
        .build()
        .expect("engine should build");

    let output = engine.parser().parse(r"\unknowncmd");

    assert!(
        !output.diagnostics().is_empty(),
        "empty_knowledge should not reset Engine parser default to lenient"
    );
}

#[test]
fn engine_empty_knowledge_preserves_explicit_parse_default() {
    // Empty knowledge should only change loaded knowledge, not caller-selected
    // parse defaults.
    let engine = Engine::builder()
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
    let error = match Engine::builder().packages(&["base"]).build() {
        Ok(_) => panic!("engine profile is required"),
        Err(error) => error,
    };

    assert!(matches!(error, texform::Error::MissingProfile));
}

#[test]
fn engine_builder_disables_rule_by_public_name() {
    let engine = Engine::builder()
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

    let unknown = Engine::builder()
        .profile(Profile::Authoring)
        .disable_rule_by_name("missing.rule");
    assert!(
        unknown.is_err(),
        "unknown rule names should fail at the facade"
    );
}
