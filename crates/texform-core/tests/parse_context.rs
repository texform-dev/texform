use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ParseContext, ParseContextBuilder,
};

#[test]
fn builder_applies_insert_and_remove_before_freezing() {
    let ctx = ParseContextBuilder::new()
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

    assert!(ctx.lookup_command("foo").is_some());
    assert!(ctx.lookup_command("frac").is_none());
}

#[test]
fn convenience_factories_still_work_after_module_move() {
    let all = ParseContext::all_packages();
    assert!(all.lookup_command("frac").is_some());

    let shared = ParseContext::all_packages_shared();
    assert!(shared.lookup_command("sqrt").is_some());
}
