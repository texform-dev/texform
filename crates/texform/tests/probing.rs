use texform::{AllowedMode, CommandItem, CommandKind, Parser};

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

    let unknown = parser.parse(r"\frac{x}{y}");
    assert!(
        unknown
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("frac"))
    );
}
