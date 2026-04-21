use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContentMode, EnvironmentItem, ParseContext,
    ParseContextBuilder,
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

    assert!(ctx.lookup_command("foo", ContentMode::Math).is_some());
    assert!(ctx.lookup_command("frac", ContentMode::Math).is_none());
}

#[test]
fn builder_applies_insert_and_remove_environment_before_freezing() {
    let ctx = ParseContextBuilder::new()
        .empty()
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
fn convenience_factories_still_work_after_module_move() {
    let all = ParseContext::all_packages();
    assert!(all.lookup_command("frac", ContentMode::Math).is_some());

    let shared = ParseContext::all_packages_shared();
    assert!(shared.lookup_command("sqrt", ContentMode::Math).is_some());
}

#[test]
fn builder_compiles_distinct_math_and_text_kbs() {
    let ctx = ParseContextBuilder::new()
        .packages(&["base", "textmacros"])
        .build()
        .expect("builder should build parse context");

    let math_underline = ctx
        .lookup_command("underline", ContentMode::Math)
        .expect("expected math underline command");
    assert_eq!(math_underline.argspec.source, "m");
    assert_eq!(math_underline.allowed_mode, AllowedMode::Math);

    let text_underline = ctx
        .lookup_command("underline", ContentMode::Text)
        .expect("expected text underline command");
    assert_eq!(text_underline.argspec.source, "m:T");
    assert_eq!(text_underline.allowed_mode, AllowedMode::Text);

    assert!(ctx.knows_command_name("underline"));
}

#[test]
fn runtime_text_only_command_only_enters_text_lane() {
    let ctx = ParseContextBuilder::new()
        .empty()
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

#[test]
fn parser_uses_text_lane_for_nested_text_only_command() {
    let ctx = ParseContextBuilder::new()
        .empty()
        .insert_item(CommandItem::new(
            "text",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:T",
        ))
        .insert_item(CommandItem::new(
            "textonly",
            CommandKind::Prefix,
            AllowedMode::Text,
            "m:T",
        ))
        .insert_item(EnvironmentItem::new(
            "textenv",
            AllowedMode::Text,
            ContentMode::Text,
            "",
        ))
        .build()
        .expect("builder should build parse context");

    assert!(ctx.lookup_command("textonly", ContentMode::Math).is_none());
    assert!(ctx.lookup_command("textonly", ContentMode::Text).is_some());
    assert!(ctx.knows_command_name("textonly"));
    assert!(ctx.knows_env_name("textenv"));

    let output = ctx.parse(r"\text{\textonly{ab}}", false);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let result = output.result.expect("expected parse result");
    assert!(
        result
            .span_for("root.child.0.arg.0.content.arg.0")
            .is_some()
    );
}
