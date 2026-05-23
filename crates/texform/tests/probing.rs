use texform::{AllowedMode, CommandItem, CommandKind, ParseConfig, Parser};

#[test]
fn parser_empty_knowledge_preserves_probing_isolation() {
    let parser = Parser::builder()
        .empty_knowledge()
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        ))
        .build()
        .expect("parser should build");

    let known = parser.parse(r"\probe{x}");
    assert!(known.diagnostics.is_empty());

    let unknown = parser.parse_with(r"\frac{x}{y}", &ParseConfig::STRICT_NO_RECOVER);
    assert!(
        unknown
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("frac"))
    );
}
