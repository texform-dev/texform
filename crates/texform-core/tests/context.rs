use texform_core::context::{DelimiterControlItem, KnowledgeBase, ParseContext};

fn assert_from_packages(actual: &[&str], expected: &[&str]) {
    assert_eq!(actual, expected);
}

#[test]
fn core_only_context_includes_core_command() {
    let ctx = ParseContext::core_only();
    let linebreak = ctx
        .lookup_command("\\")
        .expect("expected core linebreak command");
    assert_from_packages(linebreak.from_packages, &["core"]);
}

#[test]
fn context_can_insert_and_remove_delimiter_controls() {
    let mut kb = KnowledgeBase::empty();
    assert!(kb.lookup_delimiter_control("langle").is_none());

    kb.insert_item(DelimiterControlItem::new("langle"))
        .expect("delimiter control item should be valid");
    assert!(kb.lookup_delimiter_control("langle").is_some());
    assert_eq!(kb.lookup_delimiter_control("langle"), Some("langle"));

    assert!(kb.remove_item(DelimiterControlItem::new("langle")));
    assert!(kb.lookup_delimiter_control("langle").is_none());
    assert_eq!(kb.lookup_delimiter_control("langle"), None);

    let ctx = ParseContext::new(kb);
    assert!(ctx.lookup_delimiter_control("langle").is_none());
}

#[test]
fn context_exposes_raw_character_and_explicit_command_views() {
    let ctx = ParseContext::from_packages(&["base", "physics"]);

    let div = ctx
        .lookup_command("div")
        .expect("expected active div command");
    assert_from_packages(div.from_packages, &["physics"]);
    assert!(!div.argspec.is_empty());

    let explicit_div = ctx
        .lookup_explicit_command("div")
        .expect("expected explicit div command");
    assert_from_packages(explicit_div.from_packages, &["physics"]);
    assert!(!explicit_div.argspec.is_empty());

    let character_div = ctx
        .lookup_character("div")
        .expect("expected raw div character");
    assert_eq!(character_div.package, "base");
    assert_eq!(character_div.unicode_value, "÷");

    let aa = ctx
        .lookup_command("AA")
        .expect("expected active AA command");
    assert_from_packages(aa.from_packages, &["base"]);
    assert!(aa.argspec.is_empty());
    assert!(ctx.lookup_explicit_command("AA").is_none());

    let character_aa = ctx
        .lookup_character("AA")
        .expect("expected raw AA character");
    assert_eq!(character_aa.package, "base");
    assert_eq!(character_aa.unicode_value, "Å");
}
