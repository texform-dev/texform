use texform::{ParseConfig, Parser};

#[test]
fn count_targets_reports_minimal_command_counts() {
    let parser = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");

    let counts =
        texform::analysis::count_targets(&parser, r"\frac{a}{b}").expect("count should succeed");

    assert_eq!(counts["cmd:frac"], 1);
}

#[test]
fn count_targets_with_uses_supplied_parse_config() {
    let parser = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");

    let error =
        texform::analysis::count_targets_with(&parser, r"\unknowncmd", &ParseConfig::STRICT)
            .expect_err("strict unknown command should fail");

    match error {
        texform::ParseAstError::NoParseResult { diagnostics }
        | texform::ParseAstError::DiagnosticsPresent { diagnostics } => {
            assert!(!diagnostics.is_empty());
        }
    }
}
