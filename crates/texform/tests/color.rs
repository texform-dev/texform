use texform::{KnowledgeBase, Profile, TransformEngine};

#[test]
fn default_color_declarations_preserve_strings_and_local_scope_in_all_profiles() {
    for profile in [
        Profile::Authoring,
        Profile::Faithful,
        Profile::Corpus,
        Profile::Equiv,
    ] {
        let engine = TransformEngine::builder().profile(profile).build().unwrap();
        for (source, expected) in [
            (r"{\color{red}x}+y", r"{ \color {red} x } + y"),
            (r"\color{red}{x}+y", r"\color {red} x + y"),
            (
                r"{\color[RGB]{255,0,0}x}+y",
                r"{ \color [RGB] {255,0,0} x } + y",
            ),
            (
                r"{\color{red!50!blue}x}+y",
                r"{ \color {red!50!blue} x } + y",
            ),
            (r"\textcolor{red}{x}+y", r"\textcolor {red} { x } + y"),
            (
                r"\definecolor{accent}{rgb}{1,0,0}\color{accent}x",
                r"\definecolor {accent} {rgb} {1,0,0} \color {accent} x",
            ),
            (
                r"\colorbox{yellow}{hello $x$}",
                r"\colorbox {yellow} {hello $x$}",
            ),
            (
                r"\fcolorbox[RGB]{255,0,0}[rgb]{0,0,1}{hello $x$}",
                r"\fcolorbox [RGB] {255,0,0} [rgb] {0,0,1} {hello $x$}",
            ),
            (r"\text{\color{red}hello}", r"\text {\color{red}hello}"),
            (
                r"\text{\definecolor{accent}{rgb}{1,0,0}\color{accent}hello}",
                r"\text {\definecolor{accent}{rgb}{1,0,0}\color{accent}hello}",
            ),
            (
                r"\text{\textcolor{red}{hello $x$}}",
                r"\text {\textcolor{red}{hello $x$}}",
            ),
            (
                r"\text{\colorbox{yellow}{hello $x$}}",
                r"\text {\colorbox{yellow}{hello $x$}}",
            ),
            (
                r"\text{\fcolorbox{red}{yellow}{hello $x$}}",
                r"\text {\fcolorbox{red}{yellow}{hello $x$}}",
            ),
        ] {
            let output = engine.normalize(source).unwrap();
            let expected = if source == r"\color{red}{x}+y"
                && matches!(profile, Profile::Authoring | Profile::Faithful)
            {
                r"\color {red} { x } + y"
            } else {
                expected
            };
            assert_eq!(output, expected, "{profile:?}: {source}");
            assert_eq!(engine.normalize(&output).unwrap(), output);
            let report = engine
                .normalize_with_report(source, &engine.default_normalize_config())
                .unwrap();
            assert!(report.report.warnings.is_empty());
        }
    }
}

#[test]
fn color_is_an_independent_package_and_literal_strings_do_not_expand_macros() {
    let knowledge = KnowledgeBase::builder()
        .packages(&["base", "color"])
        .build()
        .unwrap();
    let engine = TransformEngine::builder()
        .profile(Profile::Corpus)
        .knowledge_base(knowledge)
        .build()
        .unwrap();
    assert!(engine.normalize(r"\color{\mycolor}x").is_err());
}
