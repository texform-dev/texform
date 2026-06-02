use texform::{Error, FlattenGroupsConfig, ParseConfig, Profile, TransformConfig, TransformEngine};

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
