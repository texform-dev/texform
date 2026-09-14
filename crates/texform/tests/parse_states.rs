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

#[test]
fn recovered_argument_errors_do_not_expose_internal_diagnostic_tags() {
    let parser = Parser::builder().build().expect("parser should build");
    for source in [r"\frac{a}", r"\text{$x}"] {
        let output = parser.parse_with(source, &ParseConfig::LENIENT);
        assert!(!output.diagnostics().is_empty());
        let rendered = format!(
            "{:?} {:?}",
            output.diagnostics(),
            output.document().map(Document::to_syntax)
        );
        assert!(!rendered.contains("__texform_diagnostic_kind"));
        assert!(!rendered.contains("texform-kind:"));
    }
}

#[test]
fn nested_command_mode_diagnostics_identify_the_rejected_command() {
    let parser = Parser::builder().build().expect("parser should build");
    for source in [
        r"\frac{\text{\frac{a}{b}}}{c}",
        r"α+\frac{\text{β\frac{a}{b}γ}}{c}+z",
    ] {
        for reject_unknown in [false, true] {
            for abort_on_error in [false, true] {
                let output = parser.parse_with(
                    source,
                    &ParseConfig {
                        reject_unknown,
                        abort_on_error,
                        ..ParseConfig::default()
                    },
                );
                let diagnostics = output.diagnostics();
                assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
                assert_eq!(
                    diagnostics[0].kind,
                    Some(texform::ParseDiagnosticKind::CommandModeError)
                );
                let start = source.rfind(r"\frac").expect("inner command");
                assert_eq!(
                    diagnostics[0].span,
                    Span {
                        start,
                        end: start + 5
                    }
                );
                if !abort_on_error && source.ends_with("+z") {
                    assert_final_z(&output);
                }
            }
        }
    }
}

#[test]
fn recovered_environment_mode_diagnostics_keep_nested_name_spans() {
    let parser = Parser::builder().build().expect("parser should build");
    for source in [
        r"\text{\begin{matrix}a\end{matrix}}",
        r"\text{\begin{matrix}a\end{matrix}}+z",
        r"α+\frac{\text{β\begin{matrix}a\end{matrix}γ}}{c}+z",
    ] {
        for reject_unknown in [false, true] {
            let output = parser.parse_with(
                source,
                &ParseConfig {
                    reject_unknown,
                    ..ParseConfig::LENIENT
                },
            );
            let diagnostics = output.diagnostics();
            assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
            assert_eq!(
                diagnostics[0].kind,
                Some(texform::ParseDiagnosticKind::EnvironmentModeError)
            );
            let start = source.find("{matrix}").expect("environment name");
            assert_eq!(
                diagnostics[0].span,
                Span {
                    start,
                    end: start + 8
                }
            );
            if source.ends_with("+z") {
                assert_final_z(&output);
            }
        }
    }
}

#[test]
fn text_script_diagnostics_keep_unicode_byte_offsets() {
    let parser = Parser::builder().build().expect("parser should build");
    let source = r"α+\text{β^γ}+z";
    for reject_unknown in [false, true] {
        for abort_on_error in [false, true] {
            let output = parser.parse_with(
                source,
                &ParseConfig {
                    reject_unknown,
                    abort_on_error,
                    ..ParseConfig::default()
                },
            );
            let diagnostics = output.diagnostics();
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            assert_eq!(
                diagnostics[0].kind,
                Some(texform::ParseDiagnosticKind::TextScriptError)
            );
            let start = source.find('^').expect("script marker");
            assert_eq!(
                diagnostics[0].span,
                Span {
                    start,
                    end: start + 1
                }
            );
            if !abort_on_error {
                assert_final_z(&output);
            }
        }
    }
}

fn assert_final_z(output: &ParseResult) {
    let syntax = output.document().expect("recovered document").to_syntax();
    let texform::SyntaxNode::Root { children, .. } = syntax else {
        panic!("expected root");
    };
    assert_eq!(children.last(), Some(&texform::SyntaxNode::Char('z')));
}
