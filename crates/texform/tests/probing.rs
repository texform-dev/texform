use texform::{AllowedMode, CommandItem, CommandKind, ParseConfig, Parser};

#[test]
fn parser_empty_packages_preserves_probing_isolation() {
    let kb = texform::KnowledgeBase::builder()
        .packages(&[])
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        ))
        .build()
        .expect("parser should build");
    let parser = Parser::builder().knowledge_base(kb).build();

    let known = parser.parse(r"\probe{x}");
    assert!(known.diagnostics().is_empty());

    let unknown = parser.parse_with(r"\frac{x}{y}", &ParseConfig::STRICT);
    assert!(
        unknown
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("frac"))
    );
}
