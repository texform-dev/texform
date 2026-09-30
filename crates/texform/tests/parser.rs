mod support;

use support::kb;
use texform::{ParseConfig, Parser};

#[test]
fn parser_parse_returns_result_on_success() {
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let success = parser.parse(r"\frac{a}{b}");
    assert!(success.document().is_some(), "expected a parse result");
    assert!(success.diagnostics().is_empty(), "no diagnostics expected");

    let document = success.try_into_document().expect("expected document").0;
    assert_eq!(
        document
            .root()
            .span()
            .expect("root should have a span")
            .start,
        0
    );
    assert_eq!(
        document.root().span().expect("root should have a span").end,
        r"\frac{a}{b}".len()
    );
}

#[test]
fn parser_parse_document_serializes_syntax_for_consumers() {
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let output = parser.parse(r"\frac{a}{b}");
    let document = output.try_into_document().expect("expected parse result").0;
    let json = serde_json::to_value(document.to_syntax()).expect("syntax should serialize");
    assert_eq!(
        document
            .root()
            .span()
            .expect("root should have a span")
            .start,
        0
    );
    assert!(json.get("Command").is_some() || json.get("Root").is_some());
}

#[test]
fn parser_parse_exposes_diagnostics_to_callers() {
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let output = parser.parse_with("{", &ParseConfig::LENIENT);
    assert!(
        output.document().is_some(),
        "lenient parse keeps a partial tree"
    );
    assert!(!output.diagnostics().is_empty(), "diagnostics expected");
}

#[test]
fn parser_parse_with_strict_unknown_command_fails() {
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let failure = parser.parse_with(r"\unknowncmd", &ParseConfig::STRICT);
    assert!(
        failure.document().is_none(),
        "strict unknown command should fail"
    );
    assert!(!failure.diagnostics().is_empty(), "diagnostics expected");
}

#[test]
fn parser_parse_uses_non_strict_recover_default() {
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let output = parser.parse(r"\unknowncmd");

    assert!(
        output.document().is_some(),
        "unknown command should be preserved"
    );
    assert!(
        output.diagnostics().is_empty(),
        "non-strict default should not report unknown commands"
    );
}

#[test]
fn parser_parse_treats_carriage_return_as_whitespace() {
    // Public parse() must accept CR / CRLF as ordinary whitespace, not panic.
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let lf = parser.parse("x\ny");
    let cr = parser.parse("x\ry");
    let crlf = parser.parse("x\r\ny");

    let lf_doc = lf.try_into_document().expect("LF input should parse").0;
    let cr_doc = cr.try_into_document().expect("CR input should parse").0;
    let crlf_doc = crlf.try_into_document().expect("CRLF input should parse").0;

    assert_eq!(cr_doc.to_syntax(), lf_doc.to_syntax());
    assert_eq!(crlf_doc.to_syntax(), lf_doc.to_syntax());
}

#[test]
fn parser_parse_with_accepts_runtime_config() {
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let output = parser.parse_with(
        r"\unknowncmd {",
        &ParseConfig {
            abort_on_error: true,
            ..Default::default()
        },
    );

    assert!(
        output.document().is_none(),
        "recover=false should not keep a partial tree for malformed input"
    );
    assert!(!output.diagnostics().is_empty(), "diagnostics expected");
}

#[test]
fn try_into_document_returns_error_for_strict_failures() {
    let parser = Parser::builder().knowledge_base(kb(&["base"])).build();

    let error = parser
        .parse_with(r"\unknowncmd", &ParseConfig::STRICT)
        .try_into_document()
        .expect_err("strict parse should fail");
    assert!(error.document().is_none());
    assert!(!error.diagnostics().is_empty());
}
