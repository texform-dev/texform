use texform::{
    ArgRef, ConformanceRule, ContentMode, Document, EditError, GroupKindRef, KnowledgeBase, Parser,
    SyntaxNode,
};

fn parse(source: &str) -> Document {
    Parser::builder()
        .build()
        .parse(source)
        .try_into_document()
        .unwrap()
        .0
}

fn assert_rule(result: Result<texform::NodeId, EditError>, rule: ConformanceRule) {
    match result {
        Err(EditError::Conformance(error)) => assert_eq!(error.rule, rule),
        result => panic!("expected {rule}, got {result:?}"),
    }
}

#[test]
fn clone_is_detached_and_deeply_independent() {
    let mut doc = parse("{x}");
    let before = doc.to_syntax();
    let original = doc.root().children().next().unwrap().id();
    let copy = doc.clone_node(original).unwrap();
    assert!(doc.node(copy).unwrap().parent().is_none());
    let copied_leaf = doc.node(copy).unwrap().children().next().unwrap().id();
    doc.set_char(copied_leaf, 'y').unwrap();
    assert_eq!(doc.to_syntax(), before);
    doc.append_child(doc.root().id(), copy).unwrap();
    assert_eq!(doc.node(copied_leaf).unwrap().char(), Some('y'));
    assert_eq!(doc.root().children().count(), 2);
}

#[test]
fn root_copies_are_implicit_groups_that_can_be_attached() {
    let mut source = parse("x_i");
    let cloned = source.clone_node(source.root().id()).unwrap();
    assert!(matches!(
        source.node(cloned).unwrap().group_kind(),
        Some(GroupKindRef::Implicit)
    ));
    let mut target = Document::new();
    let imported = target.import_node(&source, source.root().id()).unwrap();
    assert!(matches!(
        target.node(imported).unwrap().group_kind(),
        Some(GroupKindRef::Implicit)
    ));
    target.append_child(target.root().id(), imported).unwrap();
    let SyntaxNode::Root {
        children: source_children,
        ..
    } = source.to_syntax()
    else {
        panic!("expected source root");
    };
    let SyntaxNode::Root {
        children: target_children,
        ..
    } = target.to_syntax()
    else {
        panic!("expected target root");
    };
    let [SyntaxNode::Group { children, .. }] = target_children.as_slice() else {
        panic!("expected one imported group");
    };
    assert_eq!(*children, source_children);
}

#[test]
fn copies_preserve_text_argument_context() {
    let mut source = parse(r"\text{hello}");
    let command = source.root().children().next().unwrap();
    let Some(ArgRef::Text(content)) = command.arg(0) else {
        panic!("expected text argument");
    };
    let content = content.id();
    let cloned = source.clone_node(content).unwrap();
    assert_rule(
        source
            .append_child(source.root().id(), cloned)
            .map(|()| cloned),
        ConformanceRule::ModeMismatch,
    );
    let mut target = Document::with_mode(ContentMode::Text);
    let imported = target.import_node(&source, content).unwrap();
    target.append_child(target.root().id(), imported).unwrap();
    assert_eq!(target.to_latex().unwrap(), "hello");
}

#[test]
fn error_source_requires_subtree_validation_and_failed_imports_are_atomic() {
    let source = Document::from_syntax(&SyntaxNode::Root {
        mode: ContentMode::Math,
        children: vec![
            SyntaxNode::Command {
                name: "frac".into(),
                args: vec![],
                known: true,
            },
            SyntaxNode::Error {
                message: "incomplete".into(),
                snippet: "?".into(),
            },
            SyntaxNode::Char('x'),
        ],
    })
    .unwrap();
    let mut target = parse("a");
    let before = target.to_syntax();
    let children: Vec<_> = source.root().children().map(|node| node.id()).collect();
    assert_rule(
        target.import_node(&source, source.root().id()),
        ConformanceRule::ErrorNode,
    );
    assert_rule(
        target.import_node(&source, children[1]),
        ConformanceRule::ErrorNode,
    );
    assert_rule(
        target.import_node(&source, children[0]),
        ConformanceRule::ArgumentCount,
    );
    assert_eq!(target.to_syntax(), before);
    let clean = target.import_node(&source, children[2]).unwrap();
    target.append_child(target.root().id(), clean).unwrap();
    assert!(!target.has_errors());
}

#[test]
fn importing_physics_arguments_into_unknown_command_is_rejected() {
    let kb = KnowledgeBase::builder()
        .packages(&["base", "physics"])
        .build()
        .unwrap();
    let parser = Parser::builder().knowledge_base(kb).build();
    let source = parser.parse(r"\qty(x)").try_into_document().unwrap().0;
    let mut target = Document::new();
    let command = source.root().children().next().unwrap().id();
    assert_rule(
        target.import_node(&source, command),
        ConformanceRule::UnknownWithArguments,
    );
    assert_eq!(target.to_latex().unwrap(), "");
    let x = target.create_char('x').unwrap();
    target.append_child(target.root().id(), x).unwrap();
}

#[test]
fn cross_knowledge_import_accepts_conforming_subtrees_and_rejects_foreign_handles() {
    let source = parse(r"\frac{x}{y}");
    let kb = KnowledgeBase::builder()
        .packages(&["base"])
        .build()
        .unwrap();
    let mut target = Document::with_knowledge_base(&kb, ContentMode::Math);
    let command = source.root().children().next().unwrap().id();
    let imported = target.import_node(&source, command).unwrap();
    target.append_child(target.root().id(), imported).unwrap();
    assert_eq!(target.to_latex().unwrap(), source.to_latex().unwrap());
    assert_eq!(
        target.import_node(&source, target.root().id()),
        Err(EditError::ForeignNode)
    );
    assert_eq!(
        target.clone_node(source.root().id()),
        Err(EditError::ForeignNode)
    );
}
