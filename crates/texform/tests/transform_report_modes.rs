//! Plain and report calls share one transform and do not keep report state.

use texform::{
    Document, Error, FlattenGroupsConfig, LowerAttributesConfig, NormalizeConfig, ParseConfig,
    Parser, Profile, TransformConfig, TransformEngine,
};
use texform_interface::syntax_node::SyntaxNode;
use texform_transform::FinalizeAstConfig;

fn engine() -> TransformEngine {
    TransformEngine::builder()
        .packages(&["base", "ams", "textmacros", "physics", "boldsymbol"])
        .profile(Profile::Corpus)
        .build()
        .expect("engine")
}

fn strict_engine() -> TransformEngine {
    TransformEngine::builder()
        .packages(&["base", "ams", "textmacros", "physics"])
        .profile(Profile::Faithful)
        .build()
        .expect("engine")
}

fn config_with(
    engine: &TransformEngine,
    edit: impl FnOnce(&mut TransformConfig),
) -> NormalizeConfig {
    let mut config = engine.default_normalize_config();
    edit(&mut config.transform);
    config
}

fn error_category(error: &Error) -> String {
    match error {
        Error::Parse(inner) => format!("parse:{inner}"),
        Error::IncompleteTree => "incomplete".to_string(),
        Error::ForeignDocument => "foreign".to_string(),
        Error::Transform(inner) => format!("transform:{}", inner.message()),
        other => format!("other:{other}"),
    }
}

fn assert_same_outcome(engine: &TransformEngine, src: &str, config: &NormalizeConfig) {
    let plain = engine
        .normalize_with(src, config)
        .unwrap_or_else(|error| panic!("plain normalize failed: {error}"));
    let reported = engine
        .normalize_with_report(src, config)
        .unwrap_or_else(|error| panic!("report normalize failed: {error}"));
    assert_eq!(plain, reported.normalized);

    let (plain_text, plain_syntax) = transformed(engine, src, config, false);
    let (report_text, report_syntax) = transformed(engine, src, config, true);
    assert_eq!(plain_text, report_text);
    assert_eq!(plain_syntax, report_syntax);
    assert_eq!(plain_text, plain);
}

fn transformed(
    engine: &TransformEngine,
    src: &str,
    config: &NormalizeConfig,
    collect_report: bool,
) -> (String, SyntaxNode) {
    let mut document = engine
        .parser()
        .parse_with(src, &config.parse)
        .try_into_document()
        .expect("parse")
        .0;
    if collect_report {
        engine
            .transform_with_report(&mut document, &config.transform)
            .expect("transform with report");
    } else {
        engine
            .transform_with(&mut document, &config.transform)
            .expect("transform");
    }
    (document.to_latex().expect("latex"), document.to_syntax())
}

fn assert_same_error(engine: &TransformEngine, src: &str, config: &NormalizeConfig) {
    let plain = engine
        .normalize_with(src, config)
        .expect_err("plain normalize should fail");
    let reported = engine
        .normalize_with_report(src, config)
        .expect_err("report normalize should fail");
    assert_eq!(error_category(&plain), error_category(&reported));
}

#[test]
fn report_collection_preserves_output_ast_and_errors() {
    let engine = engine();
    let faithful = strict_engine();
    let cases = [
        (
            r"{\bf a}+{\large \mathbf{x}}+\text{\textbf{y}}+\vb{b}",
            &engine,
        ),
        (r"f^{\prime\prime}+\text{a  b{}}", &engine),
        (r"\cos{+}", &faithful),
        (r"{+{a \over b}}", &faithful),
        (r"A\,\,\,\,\,\,\,\,\,\,\,\,B", &engine),
    ];
    for (src, engine) in cases {
        assert_same_outcome(engine, src, &engine.default_normalize_config());
    }

    let disabled = [
        config_with(&engine, |config| {
            config.lower_attributes = LowerAttributesConfig::DISABLED;
        }),
        config_with(&engine, |config| {
            config.rewrite.enabled = false;
        }),
        config_with(&engine, |config| {
            config.finalize_ast = FinalizeAstConfig::DISABLED;
        }),
        config_with(&engine, |config| {
            config.flatten_groups = FlattenGroupsConfig::DISABLED;
        }),
    ];
    let shared = r"{\bf a}+\vb{b}+f^{\prime\prime}+''{''}+\text{a{}b}+{{c}}+\cos{+}";
    for config in disabled {
        assert_same_outcome(&engine, shared, &config);
    }

    assert_same_error(&engine, "{", &engine.default_normalize_config());
    let authoring = TransformEngine::builder()
        .packages(&["base"])
        .profile(Profile::Authoring)
        .build()
        .expect("authoring");
    assert_same_error(
        &authoring,
        r"A \buildrel f \over = B",
        &authoring.default_normalize_config(),
    );
}

#[test]
fn one_engine_does_not_keep_report_state_across_calls() {
    let engine = strict_engine();
    let config = engine.default_normalize_config();
    let src = r"\cos{+}+{\bf x}+f^{\prime\prime}";

    let first = engine
        .normalize_with_report(src, &config)
        .expect("first report");
    let plain = engine.normalize_with(src, &config).expect("plain");
    assert_eq!(plain, first.normalized);

    let failed = engine
        .normalize_with_report("{", &config)
        .expect_err("parse failure");
    assert!(error_category(&failed).starts_with("parse:"));

    let authoring = TransformEngine::builder()
        .packages(&["base"])
        .profile(Profile::Authoring)
        .build()
        .expect("authoring");
    let contract = authoring
        .transform_with_report(
            &mut parsed(&authoring, r"A \buildrel f \over = B"),
            authoring.default_transform_config(),
        )
        .expect_err("contract failure");
    assert!(error_category(&contract).starts_with("transform:"));

    let second = engine
        .normalize_with_report(src, &config)
        .expect("second report");
    assert_eq!(second.normalized, first.normalized);
    assert_eq!(second.report, first.report);

    let mut again = parsed(&engine, src);
    engine
        .transform_with(&mut again, &config.transform)
        .expect("plain transform");
    let mut reported = parsed(&engine, src);
    let transform_report = engine
        .transform_with_report(&mut reported, &config.transform)
        .expect("reported transform");
    assert_eq!(again.to_syntax(), reported.to_syntax());
    assert_eq!(transform_report, first.report);
}

#[test]
fn report_calls_keep_source_and_completeness_checks() {
    let engine = engine();
    let config = *engine.default_transform_config();

    let mut incomplete = engine
        .parser()
        .parse_with("{", &ParseConfig::LENIENT)
        .into_parts()
        .0
        .expect("partial document");
    let before = incomplete.to_latex().expect("latex");
    let plain = engine
        .transform_with(&mut incomplete, &config)
        .expect_err("plain incomplete");
    let reported = engine
        .transform_with_report(&mut incomplete, &config)
        .expect_err("reported incomplete");
    assert_eq!(error_category(&plain), "incomplete");
    assert_eq!(error_category(&reported), "incomplete");
    assert_eq!(incomplete.to_latex().expect("latex"), before);

    let mut foreign = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser")
        .parse("x")
        .try_into_document()
        .expect("parse")
        .0;
    let foreign_before = foreign.to_latex().expect("latex");
    let plain_foreign = engine
        .transform_with(&mut foreign, &config)
        .expect_err("plain foreign");
    let reported_foreign = engine
        .transform_with_report(&mut foreign, &config)
        .expect_err("reported foreign");
    assert_eq!(error_category(&plain_foreign), "foreign");
    assert_eq!(error_category(&reported_foreign), "foreign");
    assert_eq!(foreign.to_latex().expect("latex"), foreign_before);

    let syntax = parsed(&engine, "x").to_syntax();
    let mut rebuilt = Document::from_syntax(&syntax).expect("rebuild");
    assert!(matches!(
        engine.transform(&mut rebuilt),
        Err(Error::ForeignDocument)
    ));
    assert!(matches!(
        engine.transform_with_report(&mut rebuilt, &config),
        Err(Error::ForeignDocument)
    ));
}

fn parsed(engine: &TransformEngine, src: &str) -> Document {
    engine
        .parser()
        .parse(src)
        .try_into_document()
        .expect("parse")
        .0
}
