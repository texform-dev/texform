use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContentMode, EnvironmentItem, ParseContextBuilder,
};

#[test]
fn builder_compiles_distinct_math_and_text_kbs() {
    let ctx = ParseContextBuilder::default()
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
fn parse_context_uses_text_lane_for_nested_text_only_command() {
    let ctx = ParseContextBuilder::empty()
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

    let output = ctx.parse(
        r"\text{\textonly{ab}}",
        &texform_core::parse::ParseConfig::default(),
    );
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let result = output.try_into_document().expect("expected parse result").0;
    assert!(
        result
            .find_commands("textonly")
            .next()
            .and_then(|node| node.span())
            .is_some()
    );
}
