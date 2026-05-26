use texform_core::parse::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, AllowedMode, CommandItem,
    CommandKind, ContentMode, ParseContext, ParseContextBuilder,
};

#[test]
fn parse_context_exposes_raw_character_and_explicit_command_views() {
    let ctx = ParseContext::from_packages(&["base", "physics"]);

    let div: &ActiveCommandRecord = ctx
        .lookup_command("div", ContentMode::Math)
        .expect("expected active div command");
    assert_eq!(div.from_packages, &["physics"]);
    assert!(!div.argspec.is_empty());

    let explicit_div: &ActiveCommandRecord = ctx
        .lookup_explicit_command("div", ContentMode::Math)
        .expect("expected explicit div command");
    assert_eq!(explicit_div.from_packages, &["physics"]);
    assert!(!explicit_div.argspec.is_empty());

    let character_div: &ActiveCharacterRecord = ctx
        .lookup_character("div", ContentMode::Math)
        .expect("expected raw div character");
    assert_eq!(character_div.package, "base");
    assert_eq!(character_div.unicode_value, "÷");

    let aa: &ActiveCommandRecord = ctx
        .lookup_command("AA", ContentMode::Math)
        .expect("expected active AA command");
    assert_eq!(aa.from_packages, &["base"]);
    assert!(aa.argspec.is_empty());
    assert!(
        ctx.lookup_explicit_command("AA", ContentMode::Math)
            .is_none()
    );

    let character_aa: &ActiveCharacterRecord = ctx
        .lookup_character("AA", ContentMode::Math)
        .expect("expected raw AA character");
    assert_eq!(character_aa.package, "base");
    assert_eq!(character_aa.unicode_value, "Å");
}

#[test]
fn parse_context_lookup_env_returns_active_environment_record() {
    let ctx = ParseContext::from_packages(&["ams"]);

    let matrix: &ActiveEnvironmentRecord = ctx
        .lookup_env("matrix", ContentMode::Math)
        .expect("expected matrix environment");

    assert_eq!(matrix.name, "matrix");
}

#[test]
fn knows_character_name_checks_loaded_character_entries() {
    let bboldx = ParseContext::from_packages(&["bboldx"]);
    assert!(bboldx.knows_character_name("bbdotlessi"));
    assert!(bboldx.knows_character_name("txtbbdotlessi"));

    let base = ParseContext::from_packages(&["base"]);
    assert!(!base.knows_character_name("bbdotlessi"));
}

#[test]
fn runtime_text_only_command_only_enters_text_lane() {
    let ctx = ParseContextBuilder::empty()
        .insert_item(CommandItem::new(
            "textonly",
            CommandKind::Prefix,
            AllowedMode::Text,
            "m:T",
        ))
        .build()
        .expect("builder should build parse context");

    assert!(ctx.lookup_command("textonly", ContentMode::Math).is_none());

    let textonly = ctx
        .lookup_command("textonly", ContentMode::Text)
        .expect("expected text-only runtime command in text lane");
    assert_eq!(textonly.argspec.source, "m:T");
    assert_eq!(textonly.allowed_mode, AllowedMode::Text);
    assert!(ctx.knows_command_name("textonly"));
}
