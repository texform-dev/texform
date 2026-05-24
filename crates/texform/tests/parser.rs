use texform::{
    AllowedMode, CommandItem, CommandKind, ContentMode, DelimiterControlItem, EnvironmentItem,
    ParseConfig, Parser, SyntaxNode,
};

#[test]
fn parser_parse_returns_result_and_diagnostics() {
    let parser = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");

    let success = parser.parse(r"\frac{a}{b}");
    assert!(success.result.is_some(), "expected a parse result");
    assert!(success.diagnostics.is_empty(), "no diagnostics expected");

    let result = success.result.unwrap();
    assert_eq!(result.span.start, 0);
    assert_eq!(result.span.end, r"\frac{a}{b}".len());
    let json = serde_json::to_value(&result).expect("parse result should serialize");
    assert!(json.get("node").is_some());
    assert!(json.get("span").is_some());

    let failure = parser.parse_with(r"\unknowncmd", &ParseConfig::STRICT);
    assert!(
        failure.result.is_none(),
        "strict unknown command should fail"
    );
    assert!(!failure.diagnostics.is_empty(), "diagnostics expected");
}

#[test]
fn parser_parse_uses_non_strict_recover_default() {
    let parser = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");

    let output = parser.parse(r"\unknowncmd");

    assert!(
        output.result.is_some(),
        "unknown command should be preserved"
    );
    assert!(
        output.diagnostics.is_empty(),
        "non-strict default should not report unknown commands"
    );
}

#[test]
fn parser_parse_with_accepts_runtime_config() {
    let parser = Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");

    let output = parser.parse_with(
        r"\unknowncmd {",
        &ParseConfig {
            abort_on_error: true,
            ..Default::default()
        },
    );

    assert!(
        output.result.is_none(),
        "recover=false should not keep a partial tree for malformed input"
    );
    assert!(!output.diagnostics.is_empty(), "diagnostics expected");
}

#[test]
fn parser_builder_items_cover_commands_environments_and_delimiters() {
    let parser = Parser::builder()
        .empty_knowledge()
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:D",
        ))
        .item(EnvironmentItem::new(
            "probeenv",
            AllowedMode::Math,
            ContentMode::Math,
            "",
        ))
        .item(DelimiterControlItem::new("langle"))
        .item(DelimiterControlItem::new("rangle"))
        .build()
        .expect("parser should build");

    for src in [
        r"\probe\langle",
        r"\begin{probeenv}a\end{probeenv}",
        r"\left\langle x\right\rangle",
    ] {
        let output = parser.parse(src);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        assert!(output.result.is_some(), "expected parse result for {src}");
    }
}

#[test]
fn parser_builder_reports_invalid_items_and_packages() {
    let invalid_item = Parser::builder()
        .empty_knowledge()
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "s:T",
        ))
        .build();
    assert!(invalid_item.is_err(), "invalid argspec should fail build");

    let unknown_package = Parser::builder().packages(&["missing-package"]).build();
    assert!(
        unknown_package.is_err(),
        "unknown package should fail build"
    );
}

#[test]
fn parser_builder_packages_use_canonical_loading_order() {
    let parser = Parser::builder()
        .packages(&["physics", "base"])
        .build()
        .expect("parser should build");

    let output = parser.parse(r"\div{a}");
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let result = output.result.expect("expected parse result");
    let children = match result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };
    match &children[0] {
        SyntaxNode::Command { name, args, .. } => {
            assert_eq!(name, "div");
            assert_eq!(
                args.len(),
                1,
                "physics command should remain active after canonical package loading"
            );
        }
        other => panic!("expected command node, got {:?}", other),
    }
}
