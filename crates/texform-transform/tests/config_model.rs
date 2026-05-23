use texform_core::parse::{ParseConfig, Parser};
use texform_core::serialize;
use texform_transform::{
    BuildConfig, FlattenGroupsConfig, Profile, TransformConfig, TransformContext,
};

#[test]
fn context_always_builds_a_plan_even_when_runtime_rewrite_is_disabled() {
    let parser = Parser::from_packages(&["base", "physics"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parser)
            .expect("transform context should build");

    let mut ast = parser
        .parse_to_ast(r"\quantity{x}", &ParseConfig::STRICT_NO_RECOVER)
        .expect("parse should succeed");
    let report = context
        .run_with(
            &mut ast,
            &parser,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                flatten_groups: FlattenGroupsConfig::DISABLED,
                max_iterations: 100,
            },
        )
        .expect("transform should run");

    assert_eq!(report.rewrite.iterations, 0);
    assert_eq!(serialize::serialize(&ast), r"\quantity { x }");
}

#[test]
fn profile_supplies_runtime_defaults_without_changing_parse_config() {
    assert!(
        Profile::Authoring
            .default_transform_config()
            .flatten_groups
            .preserve_empty_group
    );
    assert!(
        !Profile::Equiv
            .default_transform_config()
            .flatten_groups
            .preserve_empty_group
    );
}
