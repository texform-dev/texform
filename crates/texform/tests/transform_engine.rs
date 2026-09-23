use texform::{
    ContentMode, FlattenGroupsConfig, LowerAttributesConfig, NormalizeConfig, ParseConfig, Parser,
    Profile, RewriteConfig, TransformConfig, TransformEngine, bindings::transform_report_to_dto,
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
        .normalize_with_report(r"\quantity{x}", &engine.default_normalize_config())
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
        .normalize_with_report(
            r"\quantity{x}",
            &NormalizeConfig {
                parse: ParseConfig::STRICT,
                transform: TransformConfig {
                    lower_attributes: LowerAttributesConfig::DISABLED,
                    rewrite: RewriteConfig {
                        enabled: false,
                        ..RewriteConfig::DEFAULT
                    },
                    finalize_ast: FinalizeAstConfig::ENABLED,
                    flatten_groups: FlattenGroupsConfig::DISABLED,
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
        (r"A^{'\alpha}", r"A ^ { { }' \alpha }"),
        (r"\vec A_\mu", r"\vec { A } _ { \mu }"),
        (r"\bar C^\mu", r"\bar { C } ^ { \mu }"),
        (r"f^{\prime\prime}", "f''"),
        (r"f^{\prime}", "f'"),
        (r"f^{'}", "f ^ { { }' }"),
        (r"f'^2", r"f ^ { \prime 2 }"),
        (r"\prime", r"\prime"),
    ];

    for (input, expected) in cases {
        let result = engine
            .normalize(input)
            .unwrap_or_else(|error| panic!("normalize should succeed for {input}: {error:?}"));
        assert_eq!(result, expected, "input: {input}");
    }
}

#[test]
fn prime_semantics_remain_stable_across_profiles() {
    let sources = [
        r"\prime",
        r"x\prime",
        r"\vec{r}\prime",
        r"x^2\prime",
        r"x^{a\prime}",
        r"b^{\star\prime}",
        "f' ' '",
        "' ' '",
        "f_n' ' '",
        "f' ' '^2",
        "f'^2",
        r"f^{'}",
        r"A^{'\alpha}",
        r"x{'}",
        r"\sum_n{'}",
        r"x^{a'}",
        r"\begin{array}{c}f^{\prime}\\f^{'}\end{array}",
        r"|E' ' '| \leq 2|V' ' '| - 2",
        r"G' ' '=(V' ' ',E' ' ')",
    ];
    for profile in [
        Profile::Authoring,
        Profile::Faithful,
        Profile::Corpus,
        Profile::Equiv,
    ] {
        let engine = TransformEngine::builder()
            .packages(&["base"])
            .profile(profile)
            .build()
            .unwrap();
        for source in sources {
            let first = engine.normalize(source).unwrap_or_else(|error| {
                panic!("first normalize failed for {source} in {profile:?}: {error:?}")
            });
            let second = engine.normalize(&first).unwrap_or_else(|error| {
                panic!(
                    "second normalize failed for {source} in {profile:?}: {error:?}; output={first}"
                )
            });
            assert_eq!(first, second, "{source} in {profile:?}");
        }
    }
}

#[test]
fn braket_normalize_emits_bare_middle_vert_except_authoring() {
    for profile in [Profile::Faithful, Profile::Corpus, Profile::Equiv] {
        let result = TransformEngine::builder()
            .packages(&["base", "physics"])
            .profile(profile)
            .build()
            .expect("engine should build")
            .normalize(r"\braket{a}{b}")
            .expect("normalize should succeed");

        assert!(
            result.contains(r"\middle \vert"),
            "{profile:?} output: {}",
            result
        );
        assert!(
            !result.contains(r"{\vert}"),
            "{profile:?} output: {}",
            result
        );
    }

    let authoring = TransformEngine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Authoring)
        .build()
        .expect("engine should build")
        .normalize(r"\braket{a}{b}")
        .expect("normalize should succeed");
    assert_eq!(authoring, r"\braket { a } { b }");

    let corpus = TransformEngine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Corpus)
        .build()
        .expect("engine should build")
        .normalize(r"\braket{a|b}")
        .expect("normalize should succeed");
    assert_eq!(
        corpus,
        r"\left \langle a | b \middle \vert a | b \right \rangle"
    );
}

#[test]
fn normalize_uses_prime_shorthand_inside_array_cells() {
    for profile in [
        Profile::Authoring,
        Profile::Faithful,
        Profile::Corpus,
        Profile::Equiv,
    ] {
        let result = TransformEngine::builder()
            .packages(&["base"])
            .profile(profile)
            .build()
            .expect("engine should build")
            .normalize(r"\begin{array}{c}f^{\prime}\end{array}")
            .expect("normalize should succeed");

        assert_eq!(
            result, r"\begin {array} {c} f' \end {array}",
            "profile: {profile:?}"
        );
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

    assert_eq!(result, r"\vec { A _ { \mu } }");
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
            result.contains(r"\displaylines"),
            "{profile:?} output: {}",
            result
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
        .transform_with_report(
            &mut document,
            &TransformConfig {
                lower_attributes: LowerAttributesConfig::DISABLED,
                rewrite: RewriteConfig {
                    enabled: false,
                    ..RewriteConfig::DEFAULT
                },
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
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

    assert_eq!(result, r"\quantity { x }");

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

    assert_eq!(result, r"\quantity { x }");
}

#[test]
fn normalize_report_dto_exposes_stable_phase_shape() {
    let engine = TransformEngine::builder()
        .packages(&["base", "physics"])
        .profile(Profile::Authoring)
        .build()
        .expect("engine should build");

    let result = engine
        .normalize_with_report(r"\quantity{{\bf x}}", &engine.default_normalize_config())
        .expect("normalize should succeed");
    let dto = transform_report_to_dto(&result.report);

    let quantity_rule = dto
        .rewrite
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
        dto.flatten_groups.guard_hits.empty_group,
        result.report.flatten_groups.guard_hits.empty_group
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
    assert!(json.get("rewrite").is_some());
    assert!(json.get("rules").is_none());
    assert!(json["rewrite"]["rules"][0].get("applied_count").is_some());
    assert!(json["rewrite"]["rules"][0].get("count").is_none());
    assert!(json["flatten_groups"].get("actions").is_some());
    assert!(json["flatten_groups"].get("guard_hits").is_some());
    assert!(json["flatten_groups"].get("guards").is_none());
    assert!(
        json["flatten_groups"]["guard_hits"]
            .get("declarative_scope")
            .is_some()
    );
    assert!(json["lower_attributes"].get("attributes").is_some());
    assert!(json["finalize_ast"].get("prime_run_merges").is_some());
    assert!(json["finalize_ast"].get("steps").is_none());
}
