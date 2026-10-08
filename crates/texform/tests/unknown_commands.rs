use texform::{Profile, TransformEngine};

#[test]
fn all_profiles_keep_adjacent_unknown_arguments() {
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
