use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContentMode, EnvironmentItem, Parser, ParserBuilder,
};
use texform_specs::builtin::PackageName;

#[test]
fn builder_applies_insert_and_remove_before_freezing() {
    let ctx = ParserBuilder::default()
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
    let ctx = ParserBuilder::empty()
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
fn explicit_parser_exposes_enabled_packages_in_import_order() {
    let ctx = Parser::from_packages(&["physics", "base", "braket"]);
    assert_eq!(
        ctx.enabled_packages(),
        &[PackageName::Base, PackageName::Braket, PackageName::Physics]
    );
    assert!(ctx.has_enabled_package(PackageName::Physics));
    assert!(!ctx.has_enabled_package(PackageName::Ams));
}

#[test]
fn empty_parser_exposes_no_enabled_packages() {
    let ctx = Parser::empty();
    assert!(ctx.enabled_packages().is_empty());
    assert!(!ctx.has_enabled_package(PackageName::Base));
}

#[test]
fn parser_debug_omits_mutation_summary() {
    let debug = format!("{:?}", Parser::empty());
    assert!(debug.contains("Parser"));
    assert!(debug.contains("kb"));
    assert!(!debug.contains("mutation_summary"));
}

#[test]
fn convenience_factories_use_default_runtime_packages() {
    let default_ctx = Parser::default();
    let shared = Parser::shared();

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
    let left = Parser::shared() as *const Parser;
    let right = Parser::shared() as *const Parser;
    assert_eq!(left, right);
}

#[test]
fn explicit_all_packages_include_braket() {
    let package_names = texform_specs::builtin::all_package_names();
    let ctx = Parser::from_packages(package_names.as_slice());
    assert!(ctx.lookup_command("braket", ContentMode::Math).is_some());
}

#[test]
fn builder_compiles_distinct_math_and_text_kbs() {
    let ctx = ParserBuilder::default()
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
fn knows_character_name_checks_loaded_character_entries() {
    let bboldx = Parser::from_packages(&["bboldx"]);
    assert!(bboldx.knows_character_name("bbdotlessi"));
    assert!(bboldx.knows_character_name("txtbbdotlessi"));

    let base = Parser::from_packages(&["base"]);
    assert!(!base.knows_character_name("bbdotlessi"));
}

#[test]
fn runtime_text_only_command_only_enters_text_lane() {
    let ctx = ParserBuilder::empty()
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
    let ctx = ParserBuilder::empty()
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

    let result = output.result.expect("expected parse result");
    assert!(
        result
            .span_for("root.child.0.arg.0.content.arg.0")
            .is_some()
    );
}
