use super::*;
use texform_interface::syntax_node::ContentMode;
use texform_specs::argspec;

fn assert_from_packages(actual: &[&str], expected: &[&str]) {
    assert_eq!(actual, expected);
}

#[test]
fn test_lookup_command() {
    let kb = KnowledgeBase::build_from_packages(&["base", "textmacros"]);
    let linebreak = kb.lookup_command("\\").unwrap();
    assert_eq!(linebreak.name, "\\");
    assert_eq!(linebreak.kind, CommandKind::Prefix);
    assert_eq!(linebreak.allowed_mode, AllowedMode::Both);
    assert_from_packages(linebreak.from_packages, &["base", "textmacros"]);
    assert_eq!(linebreak.argspec.len(), 2);
    assert_eq!(linebreak.argspec[0].kind, ValueKind::Star);
    assert_eq!(linebreak.argspec[1].kind, ValueKind::Dimension);

    assert!(kb.lookup_command("unknown").is_none());
}

#[test]
fn test_lookup_env() {
    let kb = build_default_kb(Some(&["ams"]));
    let align = kb.lookup_env("align").unwrap();
    assert_eq!(align.name, "align");
    assert_eq!(align.allowed_mode, AllowedMode::Math);
    assert_eq!(align.body_mode, ContentMode::Math);
    assert_from_packages(align.from_packages, &["ams"]);

    assert!(kb.lookup_env("unknown").is_none());
}

#[test]
fn test_arg_spec_helpers() {
    let mandatory_math = ArgSpec::mandatory(ContentMode::Math);
    assert!(mandatory_math.required);
    assert_eq!(
        mandatory_math.kind,
        ValueKind::Content {
            mode: ContentMode::Math
        }
    );

    let optional_text = ArgSpec::optional(ContentMode::Text);
    assert!(!optional_text.required);
    assert_eq!(
        optional_text.kind,
        ValueKind::Content {
            mode: ContentMode::Text
        }
    );
}

#[test]
fn test_delimiter_controls() {
    let kb = KnowledgeBase::empty();
    assert!(kb.lookup_delimiter_control("langle").is_none());
    assert!(kb.lookup_delimiter_control("notadelim").is_none());

    let mut kb = KnowledgeBase::new();
    kb.import_package_with_name(
        "inline-delims",
        PackageSpecs {
            characters: vec![],
            commands: vec![],
            environments: vec![],
            delimiters: vec![
                DelimiterSpec {
                    name: "langle".to_string(),
                    is_control_sequence: true,
                    allowed_mode: AllowedMode::Math,
                    unicode_value: "⟨".to_string(),
                    attributes: CharacterAttributes {
                        mathvariant: None,
                        ..CharacterAttributes::default()
                    },
                },
                DelimiterSpec {
                    name: "|".to_string(),
                    is_control_sequence: false,
                    allowed_mode: AllowedMode::Math,
                    unicode_value: "|".to_string(),
                    attributes: CharacterAttributes {
                        mathvariant: None,
                        ..CharacterAttributes::default()
                    },
                },
            ],
        },
    );
    assert_eq!(kb.lookup_delimiter_control("langle"), Some("langle"));
    assert_eq!(
        kb.lookup_delimiter("langle", true)
            .map(|item| item.unicode_value.as_str()),
        Some("⟨")
    );
    assert_eq!(
        kb.lookup_delimiter("|", false)
            .map(|item| item.unicode_value.as_str()),
        Some("|")
    );
}

#[test]
fn test_builder_import_overrides_by_order() {
    let mut kb = KnowledgeBase::new();
    kb.insert_or_override_command(CommandSpec {
        name: "foo".to_string(),
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m").into(),
        tags: vec![],
    });

    kb.import_package(texform_specs::specs::PackageSpecs {
        characters: vec![],
        commands: vec![texform_specs::specs::CommandSpec {
            name: "foo".to_string(),
            kind: CommandKind::Prefix,
            allowed_mode: AllowedMode::Text,
            argspec: argspec!("").into(),
            tags: vec![],
        }],
        environments: vec![],
        delimiters: vec![],
    });

    let foo = kb.lookup_command("foo").unwrap();
    assert_eq!(foo.allowed_mode, AllowedMode::Text);
    assert!(foo.argspec.is_empty());
    assert_from_packages(foo.from_packages, &[UNKNOWN_PACKAGE_NAME]);
}

#[test]
fn test_character_import_preserves_allowed_mode() {
    let mut kb = KnowledgeBase::new();
    kb.import_package(texform_specs::specs::PackageSpecs {
        characters: vec![texform_specs::specs::CharacterSpec {
            name: "alpha".to_string(),
            allowed_mode: AllowedMode::Text,
            unicode_value: "α".to_string(),
            attributes: CharacterAttributes {
                mathvariant: Some("italic".to_string()),
                ..CharacterAttributes::default()
            },
        }],
        commands: vec![],
        environments: vec![],
        delimiters: vec![],
    });

    let alpha = kb.lookup_command("alpha").unwrap();
    assert_eq!(alpha.kind, CommandKind::Prefix);
    assert_eq!(alpha.allowed_mode, AllowedMode::Text);
    assert!(alpha.argspec.is_empty());
    assert!(kb.lookup_explicit_command("alpha").is_none());
    assert_from_packages(alpha.from_packages, &[UNKNOWN_PACKAGE_NAME]);

    let alpha_character = kb.lookup_character("alpha").unwrap();
    assert_eq!(alpha_character.allowed_mode, AllowedMode::Text);
    assert_eq!(alpha_character.unicode_value, "α");
    assert_eq!(
        alpha_character.attributes.mathvariant.as_deref(),
        Some("italic")
    );
    assert_eq!(alpha_character.package, UNKNOWN_PACKAGE_NAME);
}

#[test]
fn test_later_character_can_override_active_explicit_command_without_removing_raw_command() {
    let mut kb = KnowledgeBase::new();
    kb.insert_or_override_command(CommandSpec {
        name: "foo".to_string(),
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m").into(),
        tags: vec![],
    });
    kb.import_package(PackageSpecs {
        characters: vec![CharacterSpec {
            name: "foo".to_string(),
            allowed_mode: AllowedMode::Text,
            unicode_value: "ƒ".to_string(),
            attributes: CharacterAttributes::default(),
        }],
        commands: vec![],
        environments: vec![],
        delimiters: vec![],
    });

    let active = kb
        .lookup_command("foo")
        .expect("expected active foo command");
    assert_eq!(active.kind, CommandKind::Prefix);
    assert_eq!(active.allowed_mode, AllowedMode::Text);
    assert!(active.argspec.is_empty());
    assert_from_packages(active.from_packages, &[UNKNOWN_PACKAGE_NAME]);

    let explicit = kb
        .lookup_explicit_command("foo")
        .expect("expected raw explicit foo command");
    assert_eq!(explicit.allowed_mode, AllowedMode::Math);
    assert_eq!(explicit.argspec.len(), 1);
    assert_from_packages(explicit.from_packages, &[UNKNOWN_PACKAGE_NAME]);

    let character = kb
        .lookup_character("foo")
        .expect("expected raw foo character");
    assert_eq!(character.allowed_mode, AllowedMode::Text);
    assert_eq!(character.unicode_value, "ƒ");
}

#[test]
fn test_later_explicit_command_overrides_active_character() {
    let mut kb = KnowledgeBase::new();
    kb.import_package(PackageSpecs {
        characters: vec![CharacterSpec {
            name: "foo".to_string(),
            allowed_mode: AllowedMode::Text,
            unicode_value: "ƒ".to_string(),
            attributes: CharacterAttributes::default(),
        }],
        commands: vec![],
        environments: vec![],
        delimiters: vec![],
    });
    kb.insert_or_override_command(CommandSpec {
        name: "foo".to_string(),
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m").into(),
        tags: vec![],
    });

    let active = kb
        .lookup_command("foo")
        .expect("expected active foo command");
    assert_eq!(active.allowed_mode, AllowedMode::Math);
    assert_eq!(active.argspec.len(), 1);
    assert_from_packages(active.from_packages, &[UNKNOWN_PACKAGE_NAME]);

    let explicit = kb
        .lookup_explicit_command("foo")
        .expect("expected raw explicit foo command");
    assert_eq!(explicit.allowed_mode, AllowedMode::Math);
    assert_eq!(explicit.argspec.len(), 1);
    assert_from_packages(explicit.from_packages, &[UNKNOWN_PACKAGE_NAME]);

    let character = kb
        .lookup_character("foo")
        .expect("expected raw foo character");
    assert_eq!(character.allowed_mode, AllowedMode::Text);
    assert_eq!(character.unicode_value, "ƒ");
}

#[test]
fn test_remove_command_suppresses_character_only_active_name() {
    let mut kb = KnowledgeBase::new();
    kb.import_package(PackageSpecs {
        characters: vec![CharacterSpec {
            name: "alpha".to_string(),
            allowed_mode: AllowedMode::Math,
            unicode_value: "α".to_string(),
            attributes: CharacterAttributes::default(),
        }],
        commands: vec![],
        environments: vec![],
        delimiters: vec![],
    });

    // kb already mutable
    assert!(kb.lookup_command("alpha").is_some());
    assert!(kb.remove_item(CommandItem::new(
        "alpha",
        CommandKind::Prefix,
        AllowedMode::Math,
        ""
    )));
    assert!(kb.lookup_command("alpha").is_none());
    assert!(kb.lookup_explicit_command("alpha").is_none());

    let character = kb
        .lookup_character("alpha")
        .expect("expected raw alpha character to remain");
    assert_eq!(character.unicode_value, "α");
}

#[test]
fn test_remove_command_does_not_fallback_to_shadowed_character() {
    let mut kb = KnowledgeBase::new();
    kb.import_package(PackageSpecs {
        characters: vec![CharacterSpec {
            name: "alpha".to_string(),
            allowed_mode: AllowedMode::Math,
            unicode_value: "α".to_string(),
            attributes: CharacterAttributes::default(),
        }],
        commands: vec![],
        environments: vec![],
        delimiters: vec![],
    });
    kb.insert_or_override_command(CommandSpec {
        name: "alpha".to_string(),
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Text,
        argspec: argspec!("m:T").into(),
        tags: vec![],
    });

    // kb already mutable
    let alpha = kb
        .lookup_command("alpha")
        .expect("expected active alpha command");
    assert_eq!(alpha.allowed_mode, AllowedMode::Text);
    assert_eq!(alpha.argspec.len(), 1);

    assert!(kb.remove_item(CommandItem::new(
        "alpha",
        CommandKind::Prefix,
        AllowedMode::Text,
        "m"
    )));
    assert!(kb.lookup_command("alpha").is_none());
    assert!(kb.lookup_explicit_command("alpha").is_none());

    let character = kb
        .lookup_character("alpha")
        .expect("expected raw alpha character to remain");
    assert_eq!(character.allowed_mode, AllowedMode::Math);
    assert_eq!(character.unicode_value, "α");
}

#[test]
fn test_remove_command_by_name_suppresses_active_name_without_touching_character_record() {
    let mut kb = KnowledgeBase::new();
    kb.import_package(PackageSpecs {
        characters: vec![CharacterSpec {
            name: "alpha".to_string(),
            allowed_mode: AllowedMode::Math,
            unicode_value: "α".to_string(),
            attributes: CharacterAttributes::default(),
        }],
        commands: vec![],
        environments: vec![],
        delimiters: vec![],
    });

    assert!(kb.remove_command_by_name("alpha"));
    assert!(!kb.remove_command_by_name("alpha"));
    assert!(kb.lookup_command("alpha").is_none());
    assert!(kb.lookup_explicit_command("alpha").is_none());
    assert_eq!(
        kb.lookup_character("alpha")
            .expect("expected raw alpha character to remain")
            .unicode_value,
        "α"
    );
}

#[test]
fn test_insert_command_clears_suppression_and_reactivates_name() {
    let mut kb = KnowledgeBase::new();
    kb.import_package(PackageSpecs {
        characters: vec![CharacterSpec {
            name: "alpha".to_string(),
            allowed_mode: AllowedMode::Math,
            unicode_value: "α".to_string(),
            attributes: CharacterAttributes::default(),
        }],
        commands: vec![],
        environments: vec![],
        delimiters: vec![],
    });

    // kb already mutable
    assert!(kb.remove_item(CommandItem::new(
        "alpha",
        CommandKind::Prefix,
        AllowedMode::Math,
        ""
    )));
    assert!(kb.lookup_command("alpha").is_none());

    kb.insert_command(CommandItem::new(
        "alpha",
        CommandKind::Prefix,
        AllowedMode::Text,
        "m",
    ))
    .expect("expected runtime explicit command insertion");

    let alpha = kb
        .lookup_command("alpha")
        .expect("expected active alpha command");
    assert_from_packages(alpha.from_packages, &[RUNTIME_PACKAGE_NAME]);
    assert_eq!(alpha.allowed_mode, AllowedMode::Text);
    assert_eq!(alpha.argspec.len(), 1);

    let explicit = kb
        .lookup_explicit_command("alpha")
        .expect("expected raw explicit alpha command");
    assert_from_packages(explicit.from_packages, &[RUNTIME_PACKAGE_NAME]);
    assert_eq!(explicit.allowed_mode, AllowedMode::Text);

    let character = kb
        .lookup_character("alpha")
        .expect("expected raw alpha character to remain");
    assert_eq!(character.allowed_mode, AllowedMode::Math);
}

#[test]
fn test_insert_env_accepts_text_body_mode() {
    let mut kb = KnowledgeBase::new();
    kb.insert_or_override_environment(EnvironmentSpec {
        name: "textenv".to_string(),
        allowed_mode: AllowedMode::Text,
        argspec: argspec!("").into(),
        body_mode: ContentMode::Text,
        tags: vec![],
    });

    let env = kb.lookup_env("textenv").unwrap();
    assert_eq!(env.body_mode, ContentMode::Text);
    assert_eq!(env.allowed_mode, AllowedMode::Text);
}

#[test]
fn test_remove_environment_by_name_reports_presence() {
    let mut kb = KnowledgeBase::new();
    kb.insert_or_override_environment(EnvironmentSpec {
        name: "textenv".to_string(),
        allowed_mode: AllowedMode::Text,
        argspec: argspec!("").into(),
        body_mode: ContentMode::Text,
        tags: vec![],
    });

    assert!(kb.remove_environment_by_name("textenv"));
    assert!(!kb.remove_environment_by_name("textenv"));
    assert!(kb.lookup_env("textenv").is_none());
}

#[test]
fn test_all_packages_default_includes_registered_package_entries() {
    let package_names = texform_specs::packages::all_package_names();
    let kb = build_default_kb(Some(package_names.as_slice()));
    assert!(kb.lookup_command("\\").is_some());
    assert!(kb.lookup_command("over").is_some());
    assert!(kb.lookup_delimiter_control("langle").is_some());
}

#[test]
fn test_explicit_base_package_includes_base_entries() {
    let kb = build_default_kb(Some(&["base"]));
    let above = kb.lookup_command("above").expect("expected base command");
    assert_from_packages(above.from_packages, &["base"]);
    assert_eq!(above.kind, CommandKind::Infix);
}

#[test]
fn test_exact_order_path_preserves_override_order_for_non_mergeable_conflicts() {
    let base_then_physics =
        try_build_kb_from_exact_packages(&["base", "physics"]).expect("expected exact build");
    let arccos = base_then_physics
        .lookup_command("arccos")
        .expect("expected arccos after loading base and physics");
    assert_from_packages(arccos.from_packages, &["physics"]);
    assert_eq!(arccos.argspec.len(), 1);

    let physics_then_base =
        try_build_kb_from_exact_packages(&["physics", "base"]).expect("expected exact build");
    let arccos = physics_then_base
        .lookup_command("arccos")
        .expect("expected arccos after loading physics and base");
    assert_from_packages(arccos.from_packages, &["base"]);
    assert!(arccos.argspec.is_empty());
}

#[test]
fn test_exact_empty_packages_still_build_empty_kb() {
    let kb = try_build_kb_from_exact_packages(&[]).expect("empty build should succeed");
    assert!(kb.lookup_command("\\").is_none());
    assert!(kb.lookup_delimiter_control("langle").is_none());
}

#[test]
fn test_canonical_package_import_order_includes_braket() {
    assert_eq!(
        canonical_package_import_order(&["physics", "braket", "base", "custom"]),
        vec!["base", "braket", "physics", "custom"]
    );
}

#[test]
fn test_default_package_names_exclude_braket() {
    assert_eq!(
        crate::parse::default_package_names(),
        &[
            "base",
            "ams",
            "physics",
            "textmacros",
            "bboldx",
            "boldsymbol"
        ]
    );
}

#[test]
fn test_explicit_package_can_override_package_linebreak_command() {
    let mut kb = try_build_kb_from_exact_packages(&["base", "textmacros"])
        .expect("package build should succeed");
    kb.import_package_with_name(
        "override",
        PackageSpecs {
            characters: vec![],
            commands: vec![CommandSpec {
                name: "\\".to_string(),
                kind: CommandKind::Prefix,
                allowed_mode: AllowedMode::Math,
                argspec: argspec!("").into(),
                tags: vec![],
            }],
            environments: vec![],
            delimiters: vec![],
        },
    );

    let linebreak = kb.lookup_command("\\").expect("expected linebreak command");
    assert_from_packages(linebreak.from_packages, &["override"]);
    assert_eq!(linebreak.allowed_mode, AllowedMode::Math);
}

#[test]
fn test_public_package_loading_merges_allowed_modes_in_canonical_order() {
    let base_then_textmacros = build_default_kb(Some(&["base", "textmacros"]));
    let textmacros_then_base = build_default_kb(Some(&["textmacros", "base"]));

    for kb in [&base_then_textmacros, &textmacros_then_base] {
        let textbf = kb
            .lookup_command("textbf")
            .expect("expected merged textbf command");
        assert_eq!(textbf.allowed_mode, AllowedMode::Both);
        assert_eq!(textbf.argspec.source, "m:T");
        assert_from_packages(textbf.from_packages, &["base", "textmacros"]);
    }
}

#[test]
fn test_public_package_loading_merges_tags_stably() {
    let kb = build_default_kb(Some(&["textmacros", "base"]));
    let tiny = kb
        .lookup_command("Tiny")
        .expect("expected merged Tiny command");

    assert_eq!(tiny.allowed_mode, AllowedMode::Both);
    assert_eq!(
        tiny.tags,
        &["discouraged", "mathjax-extension", "presentation"]
    );
    assert_from_packages(tiny.from_packages, &["base", "textmacros"]);
}

#[test]
fn test_exact_duplicate_records_collect_all_source_packages() {
    let kb = build_default_kb(Some(&["physics", "ams", "bboldx", "base"]));

    let frac = kb.lookup_command("frac").expect("expected merged frac");
    assert_from_packages(frac.from_packages, &["base", "ams"]);

    let mathbb = kb.lookup_command("mathbb").expect("expected merged mathbb");
    assert_from_packages(mathbb.from_packages, &["base", "bboldx"]);

    let smallmatrix = kb
        .lookup_env("smallmatrix")
        .expect("expected merged smallmatrix");
    assert_from_packages(smallmatrix.from_packages, &["ams", "physics"]);
}

#[test]
fn test_physics_denylist_commands_do_not_merge() {
    let kb = build_default_kb(Some(&["base", "physics"]));

    for name in ["Pr", "det", "exp"] {
        let command = kb
            .lookup_command(name)
            .expect("expected denylisted command");
        assert_from_packages(command.from_packages, &["physics"]);
    }
}

#[test]
fn test_spec_mismatch_commands_do_not_merge_under_public_loading() {
    let kb = build_default_kb(Some(&["textmacros", "physics", "base"]));

    let arccos = kb
        .lookup_command("arccos")
        .expect("expected arccos command");
    assert_eq!(arccos.argspec.source, "o");
    assert_eq!(arccos.argspec.len(), 1);
    assert_from_packages(arccos.from_packages, &["physics"]);

    let bbb = kb.lookup_command("Bbb").expect("expected Bbb command");
    assert_eq!(bbb.argspec.source, "m:T");
    assert_eq!(bbb.allowed_mode, AllowedMode::Text);
    assert_from_packages(bbb.from_packages, &["textmacros"]);

    let smash = kb.lookup_command("smash").expect("expected smash command");
    assert_eq!(smash.argspec.source, "O{}:N m:T");
    assert_eq!(smash.allowed_mode, AllowedMode::Text);
    assert_from_packages(smash.from_packages, &["textmacros"]);

    let underline = kb
        .lookup_command("underline")
        .expect("expected underline command");
    assert_eq!(underline.argspec.source, "m:T");
    assert_eq!(underline.allowed_mode, AllowedMode::Text);
    assert_from_packages(underline.from_packages, &["textmacros"]);
}

#[test]
fn test_spec_mismatch_commands_split_by_target_mode() {
    let math_kb = KnowledgeBase::try_build_from_packages_for_mode(
        &["textmacros", "physics", "base"],
        ContentMode::Math,
    )
    .expect("expected math kb build");
    let text_kb = KnowledgeBase::try_build_from_packages_for_mode(
        &["textmacros", "physics", "base"],
        ContentMode::Text,
    )
    .expect("expected text kb build");

    let math_underline = math_kb
        .lookup_command("underline")
        .expect("expected math underline command");
    assert_eq!(math_underline.argspec.source, "m");
    assert_eq!(math_underline.allowed_mode, AllowedMode::Math);
    assert_from_packages(math_underline.from_packages, &["base"]);

    let text_underline = text_kb
        .lookup_command("underline")
        .expect("expected text underline command");
    assert_eq!(text_underline.argspec.source, "m:T");
    assert_eq!(text_underline.allowed_mode, AllowedMode::Text);
    assert_from_packages(text_underline.from_packages, &["textmacros"]);

    let math_bbb = math_kb.lookup_command("Bbb").expect("expected math Bbb");
    assert_eq!(math_bbb.argspec.source, "m");
    assert_eq!(math_bbb.allowed_mode, AllowedMode::Math);
    assert_from_packages(math_bbb.from_packages, &["base"]);

    let text_bbb = text_kb.lookup_command("Bbb").expect("expected text Bbb");
    assert_eq!(text_bbb.argspec.source, "m:T");
    assert_eq!(text_bbb.allowed_mode, AllowedMode::Text);
    assert_from_packages(text_bbb.from_packages, &["textmacros"]);
}
