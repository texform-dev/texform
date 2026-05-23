use texform_core::parse::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, AllowedMode, CommandItem,
    CommandKind, ContentMode, DelimiterControlItem, EnvironmentItem, ParseContext,
    ParseContextBuilder,
};

fn assert_from_packages(actual: &[&str], expected: &[&str]) {
    assert_eq!(actual, expected);
}

#[test]
fn empty_context_starts_empty() {
    let ctx = ParseContext::empty();
    assert!(ctx.lookup_command("\\", ContentMode::Math).is_none());
    assert!(ctx.lookup_command("text", ContentMode::Math).is_none());
}

#[test]
fn package_context_loads_linebreak_from_base_and_textmacros() {
    let ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let math_linebreak = ctx
        .lookup_command("\\", ContentMode::Math)
        .expect("expected math linebreak command from base knowledge");
    assert_eq!(math_linebreak.argspec.source, "!s !o:L");
    assert_eq!(
        math_linebreak.allowed_mode,
        texform_core::parse::AllowedMode::Math
    );
    assert_from_packages(math_linebreak.from_packages, &["base"]);

    let text_linebreak = ctx
        .lookup_command("\\", ContentMode::Text)
        .expect("expected text linebreak command from textmacros knowledge");
    assert_eq!(text_linebreak.argspec.source, "!s !o:L");
    assert_eq!(
        text_linebreak.allowed_mode,
        texform_core::parse::AllowedMode::Text
    );
    assert_from_packages(text_linebreak.from_packages, &["textmacros"]);
}

#[test]
fn parse_context_builder_can_insert_and_remove_delimiter_controls() {
    let ctx = ParseContextBuilder::empty()
        .insert_item(DelimiterControlItem::new("langle"))
        .remove_delimiter_control("langle")
        .build()
        .expect("parse context should build");
    assert!(ctx.lookup_delimiter_control("langle").is_none());
}

#[test]
fn parse_context_builder_insert_then_remove_items_keeps_final_view_clean() {
    let ctx = ParseContextBuilder::empty()
        .insert_item(CommandItem::new(
            "tempcmd",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        ))
        .insert_item(EnvironmentItem::new(
            "tempenv",
            AllowedMode::Math,
            ContentMode::Math,
            "m",
        ))
        .remove_command("tempcmd")
        .remove_environment("tempenv")
        .build()
        .expect("parse context should build");

    assert!(ctx.lookup_command("tempcmd", ContentMode::Math).is_none());
    assert!(ctx.lookup_env("tempenv", ContentMode::Math).is_none());
}

#[test]
fn parse_context_exposes_raw_character_and_explicit_command_views() {
    let ctx = ParseContext::from_packages(&["base", "physics"]);

    let div: &ActiveCommandRecord = ctx
        .lookup_command("div", ContentMode::Math)
        .expect("expected active div command");
    assert_from_packages(div.from_packages, &["physics"]);
    assert!(!div.argspec.is_empty());

    let explicit_div: &ActiveCommandRecord = ctx
        .lookup_explicit_command("div", ContentMode::Math)
        .expect("expected explicit div command");
    assert_from_packages(explicit_div.from_packages, &["physics"]);
    assert!(!explicit_div.argspec.is_empty());

    let character_div: &ActiveCharacterRecord = ctx
        .lookup_character("div", ContentMode::Math)
        .expect("expected raw div character");
    assert_eq!(character_div.package, "base");
    assert_eq!(character_div.unicode_value, "÷");

    let aa: &ActiveCommandRecord = ctx
        .lookup_command("AA", ContentMode::Math)
        .expect("expected active AA command");
    assert_from_packages(aa.from_packages, &["base"]);
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
