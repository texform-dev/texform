use texform_core::parse::{ContentMode, DelimiterControlItem, ParseContext, ParseContextBuilder};

fn assert_from_packages(actual: &[&str], expected: &[&str]) {
    assert_eq!(actual, expected);
}

#[test]
fn core_only_context_includes_core_command() {
    let ctx = ParseContext::core_only();
    let linebreak = ctx
        .lookup_command("\\", ContentMode::Math)
        .expect("expected core linebreak command");
    assert_from_packages(linebreak.from_packages, &["core"]);
}

#[test]
fn context_can_insert_and_remove_delimiter_controls() {
    let ctx = ParseContextBuilder::new()
        .empty()
        .insert_item(DelimiterControlItem::new("langle"))
        .remove_delimiter_control("langle")
        .build()
        .expect("parse context should build");
    assert!(ctx.lookup_delimiter_control("langle").is_none());
}

#[test]
fn context_exposes_raw_character_and_explicit_command_views() {
    let ctx = ParseContext::from_packages(&["base", "physics"]);

    let div = ctx
        .lookup_command("div", ContentMode::Math)
        .expect("expected active div command");
    assert_from_packages(div.from_packages, &["physics"]);
    assert!(!div.argspec.is_empty());

    let explicit_div = ctx
        .lookup_explicit_command("div", ContentMode::Math)
        .expect("expected explicit div command");
    assert_from_packages(explicit_div.from_packages, &["physics"]);
    assert!(!explicit_div.argspec.is_empty());

    let character_div = ctx
        .lookup_character("div", ContentMode::Math)
        .expect("expected raw div character");
    assert_eq!(character_div.package, "base");
    assert_eq!(character_div.unicode_value, "÷");

    let aa = ctx
        .lookup_command("AA", ContentMode::Math)
        .expect("expected active AA command");
    assert_from_packages(aa.from_packages, &["base"]);
    assert!(aa.argspec.is_empty());
    assert!(
        ctx.lookup_explicit_command("AA", ContentMode::Math)
            .is_none()
    );

    let character_aa = ctx
        .lookup_character("AA", ContentMode::Math)
        .expect("expected raw AA character");
    assert_eq!(character_aa.package, "base");
    assert_eq!(character_aa.unicode_value, "Å");
}
