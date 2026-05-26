use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContentMode, DelimiterControlItem, EnvironmentItem,
    ParseContext, ParseContextBuilder,
};
use texform_knowledge::builtin::PackageName;

#[test]
fn empty_context_starts_empty() {
    let ctx = ParseContext::empty();
    assert!(ctx.lookup_command("\\", ContentMode::Math).is_none());
    assert!(ctx.lookup_command("text", ContentMode::Math).is_none());
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
fn builder_applies_insert_and_remove_before_freezing() {
    let ctx = ParseContextBuilder::default()
        .packages(&["base"])
        .insert_item(CommandItem::new(
            "foo",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        ))
        .remove_command("frac")
        .build()
        .expect("builder should build parse context");

    assert!(ctx.lookup_command("foo", ContentMode::Math).is_some());
    assert!(ctx.lookup_command("frac", ContentMode::Math).is_none());
}

#[test]
fn builder_applies_insert_and_remove_environment_before_freezing() {
    let ctx = ParseContextBuilder::empty()
        .insert_item(EnvironmentItem::new(
            "tempenv",
            AllowedMode::Math,
            ContentMode::Math,
            "m",
        ))
        .remove_environment("tempenv")
        .build()
        .expect("builder should build parse context");

    assert!(ctx.lookup_env("tempenv", ContentMode::Math).is_none());
}

#[test]
fn parse_context_debug_omits_mutation_summary() {
    let debug = format!("{:?}", ParseContext::empty());
    assert!(debug.contains("ParseContext"));
    assert!(debug.contains("kb"));
    assert!(!debug.contains("mutation_summary"));
}

#[test]
fn convenience_factories_use_default_runtime_packages() {
    let default_ctx = ParseContext::default();
    let shared = ParseContext::shared();

    let expected_packages = &[
        PackageName::Base,
        PackageName::Ams,
        PackageName::Physics,
        PackageName::Textmacros,
        PackageName::Bboldx,
        PackageName::Boldsymbol,
    ];
    assert_eq!(default_ctx.enabled_packages(), expected_packages);
    assert_eq!(shared.enabled_packages(), expected_packages);

    assert!(
        default_ctx
            .lookup_command("frac", ContentMode::Math)
            .is_some()
    );
    assert!(shared.lookup_command("sqrt", ContentMode::Math).is_some());
    assert!(
        default_ctx
            .lookup_command("braket", ContentMode::Math)
            .is_some()
    );
    assert!(shared.lookup_command("braket", ContentMode::Math).is_some());
}

#[test]
fn shared_returns_the_same_default_context_instance() {
    let left = ParseContext::shared() as *const ParseContext;
    let right = ParseContext::shared() as *const ParseContext;
    assert_eq!(left, right);
}
