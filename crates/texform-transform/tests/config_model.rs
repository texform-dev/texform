use texform_core::parse::{ParseConfig, ParseContext};
use texform_core::serialize;
use texform_transform::{
    BuildConfig, FinalizeAstConfig, FlattenGroupsConfig, Profile, RuleLevel, TransformConfig,
    TransformContext,
};

#[test]
fn profile_rule_levels_are_strictly_cumulative() {
    let profiles = [
        (Profile::Authoring, &[RuleLevel::Authoring][..]),
        (
            Profile::Faithful,
            &[RuleLevel::Authoring, RuleLevel::Faithful][..],
        ),
        (
            Profile::Corpus,
            &[RuleLevel::Authoring, RuleLevel::Faithful, RuleLevel::Corpus][..],
        ),
        (
            Profile::Equiv,
            &[
                RuleLevel::Authoring,
                RuleLevel::Faithful,
                RuleLevel::Corpus,
                RuleLevel::Equiv,
            ][..],
        ),
    ];

    for (profile, enabled) in profiles {
        let levels = profile.rule_levels();
        for level in [
            RuleLevel::Authoring,
            RuleLevel::Faithful,
            RuleLevel::Corpus,
            RuleLevel::Equiv,
        ] {
            assert_eq!(
                levels.contains(level),
                enabled.contains(&level),
                "{profile:?}"
            );
        }
    }
}

#[test]
fn context_always_builds_a_plan_even_when_runtime_rewrite_is_disabled() {
    let parser = ParseContext::from_packages(&["base", "physics"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parser)
            .expect("transform context should build");

    let document = parser
        .parse(r"\quantity{x}", &ParseConfig::STRICT)
        .try_into_document()
        .expect("parse should succeed")
        .0;
    let mut ast = texform_core::ast::Ast::from_syntax_root(&document.to_syntax());
    let report = context
        .run_with(
            &mut ast,
            &parser,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::DISABLED,
                flatten_groups: FlattenGroupsConfig::DISABLED,
                max_iterations: 100,
            },
        )
        .expect("transform should run");

    assert_eq!(report.rewrite.iterations, 0);
    assert_eq!(serialize::serialize(&ast), r"\quantity { x }");
}
