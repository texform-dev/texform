use texform::{Profile, TransformEngine, diagnostics::TransformWarning};

#[test]
fn all_profiles_keep_adjacent_unknown_arguments_and_report_the_input_names() {
    for profile in [
        Profile::Authoring,
        Profile::Faithful,
        Profile::Corpus,
        Profile::Equiv,
    ] {
        let engine = TransformEngine::builder().profile(profile).build().unwrap();
        for source in [
            r"\unknown{a}{}{bc}",
            r"\newcommand{\R}{\mathbb{R}}",
            r"\require{cancel}\cancel{x}",
        ] {
            let result = engine
                .normalize_with_report(source, &engine.default_normalize_config())
                .unwrap();
            assert_eq!(result.normalized, engine.normalize(source).unwrap());
            assert!(
                result
                    .report
                    .flatten_groups
                    .guard_hits
                    .unknown_command_arguments
                    > 0
            );
            assert!(!result.report.warnings.is_empty());
            assert_eq!(
                engine.normalize(&result.normalized).unwrap(),
                result.normalized
            );
        }
        assert_eq!(
            engine.normalize(r"\unknown{a}{}{bc}").unwrap(),
            r"\unknown { a } { } { b c }"
        );
        let mut strict = engine.default_normalize_config();
        strict.parse.reject_unknown = true;
        assert!(engine.normalize_with(r"\unknown{a}", &strict).is_err());
    }
}

#[test]
fn warnings_are_deduplicated_sorted_and_present_when_all_phases_are_disabled() {
    let engine = TransformEngine::builder()
        .profile(Profile::Corpus)
        .build()
        .unwrap();
    let mut config = engine.default_normalize_config();
    config.transform.lower_attributes.enabled = false;
    config.transform.rewrite.enabled = false;
    config.transform.finalize_ast.enabled = false;
    config.transform.flatten_groups.enabled = false;
    let result = engine
        .normalize_with_report(
            r"\zunknown{a}+\aunknown{b}+\zunknown{c}+\begin{unknownenv}x\end{unknownenv}",
            &config,
        )
        .unwrap();
    assert_eq!(
        result.report.warnings,
        vec![
            TransformWarning::UnknownCommand {
                name: "aunknown".into()
            },
            TransformWarning::UnknownCommand {
                name: "zunknown".into()
            },
            TransformWarning::UnknownEnvironment {
                name: "unknownenv".into()
            },
        ]
    );
    assert_eq!(
        result
            .report
            .flatten_groups
            .guard_hits
            .unknown_command_arguments,
        0
    );
}
