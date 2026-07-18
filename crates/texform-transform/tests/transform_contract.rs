use texform_core::ast::Ast;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_interface::syntax_node::{ContentMode, SyntaxNode};
use texform_knowledge::builtin::base;
use texform_transform::{
    BuildConfig, Profile, RewriteError, RewriteReport, RuleTarget, RuleTargetKey, TransformContext,
    TransformError, collect_eliminated_violations,
};

fn parse_to_ast(parse_ctx: &ParseContext, src: &str) -> Ast {
    let document = parse_ctx
        .parse(src, &ParseConfig::default())
        .try_into_document()
        .expect("source should parse")
        .0;
    Ast::from_syntax_root(&document.to_syntax())
}

#[test]
fn transform_contract_collector_reports_all_eliminated_form_violations() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let ast = parse_to_ast(&parse_ctx, r"\break x \goodbreak y");
    let eliminated_forms = [
        RuleTarget::Command(&base::cmd::BREAK).key(),
        RuleTarget::Command(&base::cmd::GOODBREAK).key(),
    ];

    let violations = collect_eliminated_violations(&ast, &parse_ctx, &eliminated_forms);

    assert_eq!(violations.len(), 2);
    assert_eq!(
        violation_target_names(&violations),
        vec!["break", "goodbreak"]
    );
    assert_eq!(violations[0].node_name.as_deref(), Some("break"));
    assert_eq!(violations[1].node_name.as_deref(), Some("goodbreak"));
    assert!(violations[0].to_string().contains("command `break`"));
}

#[test]
fn transform_contract_engine_reports_violation_after_full_pipeline() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let mut ast = parse_to_ast(&parse_ctx, r"A \buildrel f \over = B");
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");

    let err = context
        .run(&mut ast, &parse_ctx)
        .expect_err("full transform should fail on an uneliminated over infix");

    assert_contract_error(
        err,
        RuleTarget::Command(&base::cmd::OVER).key(),
        Some("over"),
    );
}

#[test]
fn transform_contract_rewrite_phase_does_not_run_eliminated_form_check() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let mut ast = parse_to_ast(&parse_ctx, r"A \buildrel f \over = B");
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");
    let mut report = RewriteReport::default();

    texform_transform::rewrite::run(
        &mut ast,
        &parse_ctx,
        context.rewrite_plan(),
        100,
        &mut report,
    )
    .expect("rewrite alone should not fail the eliminated-form contract");

    let violations =
        collect_eliminated_violations(&ast, &parse_ctx, context.rewrite_plan().eliminated_forms());
    assert_eq!(violation_target_names(&violations), vec!["over"]);
}

#[test]
fn transform_contract_does_not_globally_eliminate_cr_line_breaks() {
    let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), &parse_ctx)
            .expect("transform context should build");

    let eliminated_names = context
        .rewrite_plan()
        .eliminated_forms()
        .iter()
        .map(|target| target.name)
        .collect::<Vec<_>>();

    assert!(
        !eliminated_names.contains(&"cr"),
        "plain-TeX matrix rules only consume \\cr inside their wrapper command bodies"
    );
}

#[test]
fn builtin_rule_fidelity_meets_level_floor() {
    for rule in texform_transform::rewrite::all_rules() {
        let meta = rule.meta();
        assert!(
            meta.fidelity >= meta.level.min_fidelity(),
            "{} declares {:?} fidelity below {:?} floor ({:?})",
            meta.key,
            meta.fidelity,
            meta.level,
            meta.level.min_fidelity()
        );
    }
}

#[test]
fn fixed_delimiter_size_rules_are_enabled_only_by_the_equiv_profile() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let corpus =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), &parse_ctx)
            .expect("corpus transform context should build");
    let equiv =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("equiv transform context should build");
    for rule_name in [
        "big-delimiter-size-drop",
        "Big-delimiter-size-drop",
        "bigg-delimiter-size-drop",
        "Bigg-delimiter-size-drop",
    ] {
        assert!(
            corpus
                .rewrite_plan()
                .rules()
                .iter()
                .all(|rule| rule.meta().key.name != rule_name),
            "{rule_name} should not be enabled by the corpus profile"
        );
        assert!(
            equiv
                .rewrite_plan()
                .rules()
                .iter()
                .any(|rule| rule.meta().key.name == rule_name),
            "{rule_name} should be enabled by the equiv profile"
        );
    }
}

#[test]
fn limit_placement_rules_are_enabled_only_by_the_equiv_profile() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let corpus =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), &parse_ctx)
            .expect("corpus transform context should build");
    let equiv =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("equiv transform context should build");

    for rule_name in ["limits-drop", "nolimits-drop"] {
        assert!(
            corpus
                .rewrite_plan()
                .rules()
                .iter()
                .all(|rule| rule.meta().key.name != rule_name),
            "{rule_name} should not be enabled by the corpus profile"
        );
        assert!(
            equiv
                .rewrite_plan()
                .rules()
                .iter()
                .any(|rule| rule.meta().key.name == rule_name),
            "{rule_name} should be enabled by the equiv profile"
        );
    }
}

#[test]
fn fraction_style_equiv_rules_are_enabled_only_by_the_equiv_profile() {
    let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
    let corpus =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), &parse_ctx)
            .expect("corpus transform context should build");
    let equiv =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("equiv transform context should build");

    for rule_name in ["cfrac-to-frac", "dbinom-and-tbinom-to-binom"] {
        assert!(
            corpus
                .rewrite_plan()
                .rules()
                .iter()
                .all(|rule| rule.meta().key.name != rule_name),
            "{rule_name} should not be enabled by the corpus profile"
        );
        assert!(
            equiv
                .rewrite_plan()
                .rules()
                .iter()
                .any(|rule| rule.meta().key.name == rule_name),
            "{rule_name} should be enabled by the equiv profile"
        );
    }
}

#[test]
fn spacing_drop_targets_are_eliminated_only_by_equiv_profile() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let corpus =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), &parse_ctx)
            .expect("corpus transform context should build");
    let equiv =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("equiv transform context should build");
    let spacing_targets = [
        RuleTarget::Command(&base::cmd::ENSPACE).key(),
        RuleTarget::Command(&base::cmd::QUAD).key(),
        RuleTarget::Command(&base::cmd::QQUAD).key(),
        RuleTarget::Command(&base::cmd::_COMMA).key(),
        RuleTarget::Command(&base::cmd::_COLON).key(),
        RuleTarget::Command(&base::cmd::_SEMICOLON).key(),
        RuleTarget::Command(&base::cmd::_EXCLAMATION).key(),
    ];

    for target in spacing_targets {
        assert!(!corpus.rewrite_plan().eliminated_forms().contains(&target));
        assert!(equiv.rewrite_plan().eliminated_forms().contains(&target));
    }
}

#[test]
fn transform_contract_accepts_prime_after_authoring_rewrite() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let mut ast = parse_to_ast(&parse_ctx, r"\prime");
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");

    context
        .run(&mut ast, &parse_ctx)
        .expect("prime should be eliminated before the contract check");

    assert_eq!(
        ast.to_syntax_root(),
        SyntaxNode::Root {
            mode: ContentMode::Math,
            children: vec![SyntaxNode::Prime { count: 1 }],
        }
    );
}

fn violation_target_names(
    violations: &[texform_transform::ContractViolation],
) -> Vec<&'static str> {
    violations
        .iter()
        .map(|violation| violation.target.name)
        .collect()
}

fn assert_contract_error(
    err: TransformError,
    expected_target: RuleTargetKey,
    expected_node_name: Option<&str>,
) {
    let TransformError::Rewrite(RewriteError::ContractViolation { target, node_name }) = err else {
        panic!("expected rewrite contract violation, got {err:?}");
    };

    assert_eq!(target, expected_target);
    assert_eq!(node_name.as_deref(), expected_node_name);
}
