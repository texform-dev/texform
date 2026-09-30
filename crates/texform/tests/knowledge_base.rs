mod support;

use support::kb;
use texform::{
    AllowedMode, CommandItem, CommandKind, ContentMode, DelimiterControlItem, Document,
    EnvironmentItem, Error, KnowledgeBase, KnowledgeBaseBuildError, Parser, Profile, SyntaxNode,
    TransformEngine,
};

#[test]
fn defaults_share_one_knowledge_base_across_parsers_engines_and_documents() {
    let kb = KnowledgeBase::default();
    let parser = Parser::builder().build();
    let engine = TransformEngine::builder()
        .profile(Profile::Equiv)
        .build()
        .unwrap();
    let mut document = Document::new();
    let mut text_document = Document::with_mode(ContentMode::Text);
    for other in [
        parser.knowledge_base(),
        engine.knowledge_base(),
        document.knowledge_base(),
        text_document.knowledge_base(),
    ] {
        assert!(KnowledgeBase::ptr_eq(&kb, other));
    }
    engine.transform(&mut document).unwrap();
    engine.transform(&mut text_document).unwrap();
    let mut parsed = parser.parse("{{x}}").try_into_document().unwrap().0;
    engine.transform(&mut parsed).unwrap();
    let mut rebuilt = Document::from_syntax(&parsed.to_syntax()).unwrap();
    engine.transform(&mut rebuilt).unwrap();
}

#[test]
fn explicit_shared_knowledge_base_survives_document_clone_and_syntax_rebuild() {
    let kb = kb(&["base"]);
    let parser = Parser::builder().knowledge_base(kb.clone()).build();
    let engine = TransformEngine::builder()
        .knowledge_base(kb.clone())
        .profile(Profile::Equiv)
        .build()
        .unwrap();
    let parsed = parser.parse("{{x}}").try_into_document().unwrap().0;
    let original = parsed.to_syntax();
    let mut cloned = parsed.clone();
    let mut rebuilt = Document::from_syntax_with(&kb, &parsed.to_syntax()).unwrap();
    let mut constructed = Document::with_knowledge_base(&kb, ContentMode::Math);
    assert_ne!(parsed.root().id(), cloned.root().id());
    for document in [&mut cloned, &mut rebuilt, &mut constructed] {
        assert!(KnowledgeBase::ptr_eq(&kb, document.knowledge_base()));
        engine.transform(document).unwrap();
    }
    assert_eq!(parsed.to_syntax(), original);
    assert_eq!(cloned.to_latex().unwrap(), "x");
}

#[test]
fn independent_builds_with_identical_packages_are_incompatible() {
    let first = kb(&["base"]);
    let second = kb(&["base"]);
    assert!(!KnowledgeBase::ptr_eq(&first, &second));
    assert!(KnowledgeBase::ptr_eq(&first, &first.clone()));
    let engine = TransformEngine::builder()
        .knowledge_base(first)
        .profile(Profile::Equiv)
        .build()
        .unwrap();
    let mut document = Document::with_knowledge_base(&second, ContentMode::Math);
    assert!(matches!(
        engine.transform(&mut document),
        Err(Error::KnowledgeBaseMismatch)
    ));
}

#[test]
fn empty_packages_load_no_builtin_records() {
    let kb = kb(&[]);
    assert!(kb.packages().is_empty());
    for mode in [ContentMode::Math, ContentMode::Text] {
        assert!(kb.commands(mode).is_empty());
        assert!(kb.environments(mode).is_empty());
        assert!(kb.characters(mode).is_empty());
        assert!(kb.lookup_command("frac", mode).is_none());
    }
    assert!(kb.delimiters().is_empty());
}

#[test]
fn enumeration_is_sorted_and_matches_mode_specific_lookup() {
    let kb = kb(&["physics", "base", "textmacros"]);
    for mode in [ContentMode::Math, ContentMode::Text] {
        let commands = kb.commands(mode);
        assert!(commands.windows(2).all(|pair| pair[0].name < pair[1].name));
        for record in commands {
            assert_eq!(kb.lookup_command(record.name, mode), Some(record));
        }
        let environments = kb.environments(mode);
        assert!(
            environments
                .windows(2)
                .all(|pair| pair[0].name < pair[1].name)
        );
        for record in environments {
            assert_eq!(kb.lookup_env(record.name, mode), Some(record));
        }
        let characters = kb.characters(mode);
        assert!(
            characters
                .windows(2)
                .all(|pair| pair[0].name < pair[1].name)
        );
        for record in characters {
            assert_eq!(kb.lookup_character(&record.name, mode), Some(record));
        }
    }
    assert!(
        kb.commands(ContentMode::Math)
            .iter()
            .any(|record| record.name == "frac")
    );
    let delimiters = kb.delimiters();
    assert!(
        delimiters
            .windows(2)
            .all(|pair| (pair[0].name, pair[0].is_control_sequence)
                < (pair[1].name, pair[1].is_control_sequence))
    );
    assert!(!delimiters.is_empty());
    for record in delimiters {
        if record.is_control_sequence {
            assert!(kb.is_delimiter_control(record.name));
        }
    }
}

#[test]
fn knowledge_base_builder_items_cover_commands_environments_and_delimiters() {
    let kb = KnowledgeBase::builder()
        .packages(&[])
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:D",
        ))
        .item(EnvironmentItem::new(
            "probeenv",
            AllowedMode::Math,
            ContentMode::Math,
            "",
        ))
        .item(DelimiterControlItem::new("langle"))
        .item(DelimiterControlItem::new("rangle"))
        .build()
        .unwrap();
    let parser = Parser::builder().knowledge_base(kb).build();

    for src in [
        r"\probe\langle",
        r"\begin{probeenv}a\end{probeenv}",
        r"\left\langle x\right\rangle",
    ] {
        let output = parser.parse(src);
        assert!(
            output.diagnostics().is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics()
        );
        assert!(
            output.document().is_some(),
            "expected parse result for {src}"
        );
    }
}

#[test]
fn knowledge_base_builder_remove_methods_hide_runtime_items() {
    let kb = KnowledgeBase::builder()
        .packages(&[])
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "",
        ))
        .item(EnvironmentItem::new(
            "probeenv",
            AllowedMode::Math,
            ContentMode::Math,
            "",
        ))
        .item(DelimiterControlItem::new("langle"))
        .remove_command("probe")
        .remove_environment("probeenv")
        .remove_delimiter_control("langle")
        .build()
        .unwrap();

    assert!(kb.lookup_command("probe", ContentMode::Math).is_none());
    assert!(kb.lookup_env("probeenv", ContentMode::Math).is_none());
    assert!(!kb.is_delimiter_control("langle"));
    assert!(kb.commands(ContentMode::Math).is_empty());
    assert!(kb.environments(ContentMode::Math).is_empty());
    assert!(kb.delimiters().is_empty());
}

#[test]
fn knowledge_base_builder_reports_invalid_items_and_packages() {
    let build_with = |item: CommandItem| KnowledgeBase::builder().packages(&[]).item(item).build();
    assert!(matches!(
        build_with(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "s:T"
        )),
        Err(KnowledgeBaseBuildError::InvalidContextItem { .. })
    ));
    assert!(matches!(
        build_with(CommandItem::new("x1", CommandKind::Prefix, AllowedMode::Math, "")),
        Err(KnowledgeBaseBuildError::InvalidName { name }) if name == "x1"
    ));
    assert!(matches!(
        KnowledgeBase::builder()
            .packages(&["missing-package"])
            .build(),
        Err(KnowledgeBaseBuildError::PackageLoad(_))
    ));
}

#[test]
fn knowledge_base_builder_packages_use_canonical_loading_order() {
    // Callers can pass packages in any order, with duplicates, and still get
    // canonical package merge behavior.
    let parser = Parser::builder()
        .knowledge_base(kb(&["physics", "base", "physics"]))
        .build();

    assert_eq!(parser.knowledge_base().packages(), ["base", "physics"]);

    let output = parser.parse(r"\div{a}");
    assert!(
        output.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics()
    );

    let result = output.try_into_document().expect("expected parse result").0;
    let children = match result.to_syntax() {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };
    match &children[0] {
        SyntaxNode::Command { name, args, .. } => {
            assert_eq!(name, "div");
            assert_eq!(
                args.len(),
                1,
                "physics command should remain active after canonical package loading"
            );
        }
        other => panic!("expected command node, got {:?}", other),
    }
}
