use texform_core::document::NodeKind;
use texform_core::document::{Arg, ArgRef, ConformanceRule, Document, EditError};
use texform_core::parse::{ContentMode, KnowledgeBase};

fn rule(error: EditError) -> ConformanceRule {
    match error {
        EditError::Conformance(error) => error.rule,
        other => panic!("expected conformance failure, got {other:?}"),
    }
}

#[test]
fn source_arguments_follow_signature_and_optional_star_defaults() {
    let mut doc = Document::new();
    let sqrt = doc.create_command("sqrt", vec!["x+1".into()]).unwrap();
    assert!(doc.node(sqrt).unwrap().arg(0).is_none());
    let op = doc
        .create_command("operatorname", vec!["sn".into()])
        .unwrap();
    assert!(matches!(
        doc.node(op).unwrap().arg(0),
        Some(ArgRef::Boolean(false))
    ));
    doc.append_child(doc.root().id(), sqrt).unwrap();
    doc.append_child(doc.root().id(), op).unwrap();
    let text = doc
        .create_command("text", vec!["hello $x$".into()])
        .unwrap();
    doc.append_child(doc.root().id(), text).unwrap();
    let output = doc.to_latex().unwrap();
    let parsed = doc.knowledge_base().parse(&output, &Default::default());
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
}

#[test]
fn absent_star_input_matches_the_parsed_unstarred_slot() {
    let mut doc = Document::new();
    let op = doc
        .create_command("operatorname", [Arg::Absent, "sn".into()])
        .unwrap();
    doc.append_child(doc.root().id(), op).unwrap();
    let parsed = doc
        .knowledge_base()
        .parse(r"\operatorname{sn}", &Default::default())
        .try_into_document()
        .unwrap()
        .0;
    assert_eq!(doc.to_syntax(), parsed.to_syntax());
}

#[test]
fn failed_construction_preserves_detached_inputs() {
    let mut doc = Document::new();
    let x = doc.create_char('x').unwrap();
    assert!(matches!(
        doc.create_command("frac", vec![x.into(), "{".into()]),
        Err(EditError::InvalidSource(_))
    ));
    assert!(doc.node(x).unwrap().parent().is_none());
    assert_eq!(
        doc.create_group(ContentMode::Math, [x.into(), x.into()]),
        Err(EditError::DuplicateChild)
    );
    doc.append_child(doc.root().id(), x).unwrap();
    assert_eq!(doc.to_latex().unwrap(), "x");
}

#[test]
fn fixed_mode_constructors_reject_conflicting_explicit_contexts() {
    let mut doc = Document::with_mode(ContentMode::Text);
    let prime = doc.create_prime(2).unwrap();
    assert_eq!(
        rule(doc.in_mode(ContentMode::Text).create_prime(2).unwrap_err()),
        ConformanceRule::ModeMismatch
    );
    assert_eq!(
        rule(doc.append_child(doc.root().id(), prime).unwrap_err()),
        ConformanceRule::ModeMismatch
    );
    let math = doc.create_inline_math([prime.into()]).unwrap();
    doc.append_child(doc.root().id(), math).unwrap();
    let fragment = doc.parse_fragment("hello $x$", None).unwrap();
    doc.append_child(doc.root().id(), fragment).unwrap();
}

#[test]
fn scripted_and_environment_source_inputs_build_complete_subtrees() {
    let mut doc = Document::new();
    let scripted = doc
        .create_scripted("x", Some("i".into()), Some("2".into()))
        .unwrap();
    let env = doc
        .create_environment_with_children("matrix", vec![], vec![scripted.into()])
        .unwrap();
    doc.append_child(doc.root().id(), env).unwrap();
    assert!(doc.to_latex().unwrap().contains("matrix"));
    let empty = doc.create_environment("matrix", [], Arg::Absent).unwrap();
    assert_eq!(
        doc.node(empty)
            .unwrap()
            .env_body()
            .unwrap()
            .children()
            .count(),
        0
    );
    assert_eq!(
        rule(doc.create_scripted("x", None, None).unwrap_err()),
        ConformanceRule::ScriptedWithoutScript
    );
    assert_eq!(
        rule(
            doc.create_command("unknowncommand", vec!["x".into()])
                .unwrap_err()
        ),
        ConformanceRule::UnknownWithArguments
    );
}

#[test]
fn group_child_sources_splice_their_parsed_nodes() {
    let mut doc = Document::new();
    let env = doc
        .create_environment_with_children("matrix", [], vec!["a".into(), "&".into(), "b".into()])
        .unwrap();
    doc.append_child(doc.root().id(), env).unwrap();
    let parsed = doc
        .knowledge_base()
        .parse(r"\begin{matrix}a&b\end{matrix}", &Default::default())
        .try_into_document()
        .unwrap()
        .0;
    assert_eq!(doc.to_syntax(), parsed.to_syntax());

    let x = doc.create_char('x').unwrap();
    let group = doc
        .create_group(
            ContentMode::Math,
            ["a+b".into(), x.into(), "&".into(), "c".into()],
        )
        .unwrap();
    let kinds: Vec<_> = doc
        .node(group)
        .unwrap()
        .children()
        .map(|node| node.kind())
        .collect();
    assert_eq!(
        kinds,
        [
            NodeKind::Char,
            NodeKind::Char,
            NodeKind::Char,
            NodeKind::Char,
            NodeKind::AlignmentTab,
            NodeKind::Char
        ]
    );
    // Single-node slots still receive the whole fragment as one group.
    let sqrt = doc.create_command("sqrt", vec!["a+b".into()]).unwrap();
    let arg = doc.node(sqrt).unwrap().arg(1).unwrap().as_node().unwrap();
    assert_eq!(arg.kind(), NodeKind::Group);
}

#[test]
fn constructed_arguments_preserve_tight_optional_adjacency() {
    use texform_core::parse::{AllowedMode, CommandItem, CommandKind};
    let kb = KnowledgeBase::builder()
        .packages(&["base"])
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m !o",
        ))
        .build()
        .unwrap();
    let mut doc = Document::with_knowledge_base(&kb, ContentMode::Math);
    let command = doc
        .create_command("probe", ["a".into(), "b".into()])
        .unwrap();
    doc.append_child(doc.root().id(), command).unwrap();
    let latex = doc.to_latex().unwrap();
    assert!(
        latex.contains("}["),
        "tight optional slot was separated: {latex}"
    );
    let (parsed, diagnostics) = kb
        .parse(&latex, &Default::default())
        .try_into_document()
        .unwrap();
    assert!(diagnostics.is_empty());
    assert!(parsed.root().children().next().unwrap().arg(1).is_some());
}

#[test]
fn constructed_until_arguments_reparse_with_their_terminators_protected() {
    use texform_core::parse::{AllowedMode, CommandItem, CommandKind};
    let kb = KnowledgeBase::builder()
        .packages(&["base"])
        .item(CommandItem::new(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            r"u{\of}:N",
        ))
        .build()
        .unwrap();
    let mut doc = Document::with_knowledge_base(&kb, ContentMode::Math);
    // Slot sources are written inside the boundaries, so this `\of` is content.
    let x = doc.create_char('x').unwrap();
    let root = doc
        .create_command("root", [r"a\of b".into(), x.into()])
        .unwrap();
    let probe = doc.create_command("probe", ["eq:1".into()]).unwrap();
    doc.append_child(doc.root().id(), root).unwrap();
    doc.append_child(doc.root().id(), probe).unwrap();
    let latex = doc.to_latex().unwrap();
    let (parsed, diagnostics) = kb
        .parse(&latex, &Default::default())
        .try_into_document()
        .unwrap();
    assert!(diagnostics.is_empty());
    assert_eq!(parsed.to_syntax(), doc.to_syntax(), "{latex}");
}

#[test]
fn body_and_argument_reuse_is_rejected_before_adoption() {
    use texform_core::parse::{AllowedMode, EnvironmentItem};
    let kb = KnowledgeBase::builder()
        .packages(&["base"])
        .item(EnvironmentItem::new(
            "probe",
            AllowedMode::Math,
            ContentMode::Math,
            "m",
        ))
        .build()
        .unwrap();
    let mut doc = Document::with_knowledge_base(&kb, ContentMode::Math);
    let x = doc.create_char('x').unwrap();
    assert_eq!(
        doc.create_environment_with_children("probe", [x.into()], vec![x.into()]),
        Err(EditError::DuplicateChild)
    );
    assert!(doc.node(x).unwrap().parent().is_none());
    doc.append_child(doc.root().id(), x).unwrap();
    assert_eq!(
        rule(
            doc.create_environment("unknownenv", ["x".into()], "y")
                .unwrap_err()
        ),
        ConformanceRule::UnknownWithArguments
    );
}

#[test]
fn wrong_command_constructors_suggest_the_matching_constructor_without_adopting_inputs() {
    for name in ["bf", "frac", "over"] {
        for constructor in ["create_command", "create_declarative", "create_infix"] {
            let expected = match name {
                "bf" => "create_declarative",
                "frac" => "create_command",
                "over" => "create_infix",
                _ => unreachable!(),
            };
            if constructor == expected {
                continue;
            }
            let mut doc = Document::new();
            let x = doc.create_char('x').unwrap();
            let before = doc.to_syntax();
            // One argument is the wrong count for all three records. Kind errors
            // must still identify the constructor before processing any inputs.
            let error = match constructor {
                "create_command" => doc.create_command(name, [x.into()]),
                "create_declarative" => doc.create_declarative(name, [x.into()]),
                "create_infix" => doc.create_infix(name, x, "{", [x.into()]),
                _ => unreachable!(),
            }
            .unwrap_err();
            let EditError::Conformance(error) = error else {
                panic!("expected conformance failure, got {error:?}");
            };
            assert_eq!(error.rule, ConformanceRule::CommandKindMismatch);
            let kind = expected.trim_start_matches("create_");
            assert!(
                error
                    .message
                    .ends_with(&format!("use the {kind} constructor")),
                "{error}"
            );
            assert_eq!(doc.to_syntax(), before);
            assert!(doc.node(x).unwrap().parent().is_none());

            let node = match name {
                "bf" => {
                    let declaration = doc.create_declarative(name, []).unwrap();
                    doc.append_child(doc.root().id(), x).unwrap();
                    declaration
                }
                "frac" => doc.create_command(name, [x.into(), "y".into()]).unwrap(),
                "over" => doc.create_infix(name, x, "y", []).unwrap(),
                _ => unreachable!(),
            };
            doc.append_child(doc.root().id(), node).unwrap();
            assert!(doc.to_latex().unwrap().contains(name));
            doc.__validate_conformance().unwrap();
        }
    }
}
