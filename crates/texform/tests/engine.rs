use texform::{
    Engine, FlattenGroupsConfig, NormalizeConfig, ParseConfig, Parser, Profile, TransformConfig,
    serialize,
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
                parse: ParseConfig::STRICT_NO_RECOVER,
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
fn ast_level_transform_preserves_parse_once_workflow() {
    let engine = Engine::builder()
        .packages(&["base"])
        .profile(Profile::Equiv)
        .build()
        .expect("engine should build");

    let ast = engine
        .parse_to_ast("{{x}}")
        .expect("parse_to_ast should succeed");
    let before = serialize(&ast).expect("ast should serialize");

    let mut transformed = ast.clone();
    engine
        .transform_ast_with(
            &mut transformed,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform should succeed");
    let after = serialize(&transformed).expect("ast should serialize");

    assert_ne!(before, after);
}

#[test]
fn parser_is_parse_only_and_needs_no_profile() {
    let parser = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");

    let output = parser.parse(r"\frac{a}{b}");
    assert!(output.diagnostics.is_empty());
}
