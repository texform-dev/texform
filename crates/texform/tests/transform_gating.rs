use texform::{
    Error, FlattenGroupsConfig, NormalizeConfig, ParseConfig, Profile, TransformConfig,
    TransformEngine,
};
use texform_transform::FinalizeAstConfig;

fn engine() -> TransformEngine {
    TransformEngine::builder()
        .packages(&["base"])
        .profile(Profile::Equiv)
        .build()
        .expect("engine should build")
}

#[test]
fn transform_rejects_documents_with_parse_errors() {
    let engine = engine();
    let mut document = engine
        .parser()
        .parse_with("{", &ParseConfig::LENIENT)
        .into_parts()
        .0
        .expect("lenient parse should keep a partial document");

    let error = engine
        .transform(&mut document)
        .expect_err("error documents must not be transformed");
    assert!(matches!(error, Error::IncompleteTree));
}

#[test]
fn transform_updates_document_in_place() {
    let engine = engine();
    let mut document = engine
        .parser()
        .parse("{{x}}")
        .try_into_document()
        .expect("parse should succeed")
        .0;
    let root_id = document.root().id();

    let report = engine
        .transform_with(
            &mut document,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform should succeed");

    assert_eq!(document.to_latex().expect("document should serialize"), "x");
    assert_eq!(report.flatten_groups.actions.replaced_single_child, 2);

    let y = document.create_char('y').expect("char should be created");
    document
        .append_child(root_id, y)
        .expect("pre-transform root id should still belong to the document");
    assert_eq!(
        document.to_latex().expect("document should serialize"),
        "x y"
    );
}

#[test]
fn normalize_uses_finalize_ast_by_default() {
    let engine = engine();

    let result = engine
        .normalize(r"f^{\prime\prime}")
        .expect("normalize should succeed");

    assert_eq!(result.normalized, "f''");
    assert_eq!(
        result
            .report
            .finalize_ast
            .steps
            .merge_adjacent_primes
            .applied_count,
        1
    );
}

#[test]
fn normalize_can_disable_finalize_ast_explicitly() {
    let engine = engine();

    let result = engine
        .normalize_with(
            r"f^{\prime\prime}",
            &NormalizeConfig {
                parse: ParseConfig::STRICT,
                transform: TransformConfig {
                    rewrite_enabled: true,
                    lower_attributes_enabled: true,
                    finalize_ast: FinalizeAstConfig::DISABLED,
                    flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                    max_iterations: 100,
                },
            },
        )
        .expect("normalize should succeed");

    assert_eq!(result.normalized, r"f ^ { '' }");
    assert_eq!(
        result
            .report
            .finalize_ast
            .steps
            .merge_adjacent_primes
            .applied_count,
        0
    );
}
