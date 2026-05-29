use texform::{Document, ParseConfig, ParseDiagnostic, ParseResult, Parser, Span};

fn parser() -> Parser {
    Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build")
}

#[test]
fn parse_success_has_document_without_diagnostics() {
    let output = parser().parse_with(r"\frac{a}{b}", &ParseConfig::STRICT);

    assert!(output.document().is_some());
    assert!(output.diagnostics().is_empty());
    assert!(!output.has_errors());
    let (document, diagnostics) = output
        .try_into_document()
        .expect("document should be editable");
    assert!(!document.has_errors());
    assert!(diagnostics.is_empty());
}

#[test]
fn hard_failure_has_no_document_but_is_not_an_error_tree() {
    let output = parser().parse_with(r"\unknowncmd", &ParseConfig::STRICT);

    assert!(output.document().is_none());
    assert!(!output.diagnostics().is_empty());
    assert!(!output.has_errors());

    let error = output
        .try_into_document()
        .expect_err("strict failure should not produce a document");
    assert!(error.document().is_none());
    assert!(!error.diagnostics().is_empty());
}

#[test]
fn editable_tree_with_diagnostics_is_returned_with_diagnostics() {
    // The parser currently routes recoverable diagnostics through Error nodes.
    // Build this state directly to lock the public result contract: diagnostics
    // alone must not make an otherwise editable document read-only.
    let output = ParseResult::__from_parts_for_tests(
        Some(Document::new()),
        vec![ParseDiagnostic::new(
            "diagnostic-only warning",
            Span { start: 0, end: 0 },
            Vec::new(),
            None,
            Vec::new(),
        )],
    );

    assert!(output.document().is_some());
    assert!(!output.diagnostics().is_empty());
    assert!(!output.has_errors());

    let (document, diagnostics) = output
        .try_into_document()
        .expect("diagnostics alone should not block an editable tree");
    assert!(!document.has_errors());
    assert!(!diagnostics.is_empty());
}

#[test]
fn recovered_error_tree_is_read_only_and_returned_in_error() {
    let output = parser().parse_with("{", &ParseConfig::LENIENT);

    assert!(output.document().is_some());
    assert!(!output.diagnostics().is_empty());
    assert!(output.has_errors());

    let error = output
        .try_into_document()
        .expect_err("read-only error tree should not become editable");
    assert!(error.document().is_some());
    assert!(
        error
            .document()
            .expect("document should be retained")
            .has_errors()
    );
    assert!(!error.diagnostics().is_empty());
}
