use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};

#[test]
fn test_syntax_node_creation() {
    // Test creating various node types
    let char_node = SyntaxNode::Char('a');
    assert!(char_node.is_leaf());

    let text_node = SyntaxNode::Text("hello".to_string());
    assert!(text_node.is_leaf());

    let group = SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: GroupKind::Explicit,
        children: vec![SyntaxNode::Char('x')],
    };
    assert!(group.is_group());
    assert_eq!(group.group_mode(), Some(ContentMode::Math));
}

#[test]
fn test_implicit_group_helpers() {
    let children = vec![SyntaxNode::Char('a'), SyntaxNode::Char('b')];
    let group = SyntaxNode::implicit_group(ContentMode::Math, children.clone());

    match group {
        SyntaxNode::Group {
            mode,
            kind,
            children: c,
        } => {
            assert_eq!(mode, ContentMode::Math);
            assert_eq!(kind, GroupKind::Implicit);
            assert_eq!(c.len(), 2);
        }
        _ => panic!("Expected Group"),
    }

    let empty = SyntaxNode::empty_group(ContentMode::Text);
    match empty {
        SyntaxNode::Group {
            mode,
            kind,
            children,
        } => {
            assert_eq!(mode, ContentMode::Text);
            assert_eq!(kind, GroupKind::Implicit);
            assert!(children.is_empty());
        }
        _ => panic!("Expected Group"),
    }
}

#[test]
fn test_argument_creation() {
    let node = SyntaxNode::Char('x');
    let mandatory = Argument::mandatory(node.clone());
    assert_eq!(mandatory.kind, ArgumentKind::Mandatory);
    assert_eq!(
        mandatory.value,
        ArgumentValue::Content(SyntaxNode::Char('x'))
    );

    let optional = Argument::optional(node);
    assert_eq!(optional.kind, ArgumentKind::Optional);
    assert_eq!(
        optional.value,
        ArgumentValue::Content(SyntaxNode::Char('x'))
    );
}

#[test]
fn test_content_mode_helpers() {
    assert!(ContentMode::Math.is_math());
    assert!(!ContentMode::Math.is_text());
    assert!(ContentMode::Text.is_text());
    assert!(!ContentMode::Text.is_math());
}

#[test]
fn test_command_node() {
    let cmd = SyntaxNode::Command {
        name: "frac".to_string(),
        starred: false,
        args: vec![
            Argument::mandatory(SyntaxNode::Char('a')),
            Argument::mandatory(SyntaxNode::Char('b')),
        ],
    };

    match cmd {
        SyntaxNode::Command {
            name,
            starred,
            args,
        } => {
            assert_eq!(name, "frac");
            assert!(!starred);
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected Command"),
    }
}

#[test]
fn test_infix_node() {
    let infix = SyntaxNode::Infix {
        name: "over".to_string(),
        starred: false,
        args: vec![],
        left: Box::new(SyntaxNode::Char('a')),
        right: Box::new(SyntaxNode::Char('b')),
    };

    match infix {
        SyntaxNode::Infix {
            name, left, right, ..
        } => {
            assert_eq!(name, "over");
            assert!(matches!(*left, SyntaxNode::Char('a')));
            assert!(matches!(*right, SyntaxNode::Char('b')));
        }
        _ => panic!("Expected Infix"),
    }
}

#[test]
fn test_scripted_normalization_structure() {
    // Test that we can create a Scripted node with both sub and sup
    let scripted = SyntaxNode::Scripted {
        base: Box::new(SyntaxNode::Char('x')),
        subscript: Some(Box::new(SyntaxNode::Char('i'))),
        superscript: Some(Box::new(SyntaxNode::Char('2'))),
    };

    match scripted {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert!(matches!(*base, SyntaxNode::Char('x')));
            assert!(subscript.is_some());
            assert!(superscript.is_some());
        }
        _ => panic!("Expected Scripted"),
    }
}

#[test]
fn test_group_kind_variants() {
    let explicit = GroupKind::Explicit;
    let implicit = GroupKind::Implicit;
    let delimited = GroupKind::Delimited {
        left: Delimiter::Char('('),
        right: Delimiter::Char(')'),
    };
    let inline_math = GroupKind::InlineMath;

    assert_ne!(explicit, implicit);
    assert_ne!(explicit, delimited);
    assert_ne!(explicit, inline_math);

    match delimited {
        GroupKind::Delimited { left, right } => {
            assert_eq!(left, Delimiter::Char('('));
            assert_eq!(right, Delimiter::Char(')'));
        }
        _ => panic!("Expected Delimited"),
    }
}

#[test]
fn test_delimiter_variants() {
    let none = Delimiter::None;
    let char_delim = Delimiter::Char('(');
    let control_delim = Delimiter::Control("langle");

    assert_ne!(none, char_delim);
    assert_ne!(char_delim, control_delim);

    match char_delim {
        Delimiter::Char(c) => assert_eq!(c, '('),
        _ => panic!("Expected Char delimiter"),
    }

    match control_delim {
        Delimiter::Control(s) => assert_eq!(s, "langle"),
        _ => panic!("Expected Control delimiter"),
    }
}

#[test]
fn test_display_simple() {
    // Test that Display implementation works without panicking
    let node = SyntaxNode::Char('a');
    let display = format!("{}", node);
    assert!(display.contains("Char"));
    assert!(display.contains('a'));

    let group = SyntaxNode::implicit_group(
        ContentMode::Math,
        vec![SyntaxNode::Char('x'), SyntaxNode::Char('y')],
    );
    let display = format!("{}", group);
    assert!(display.contains("Group"));
    assert!(display.contains("Char('x')"));
    assert!(display.contains("Char('y')"));
}

#[test]
fn test_unknown_command() {
    let unknown = SyntaxNode::UnknownCommand {
        name: "foo".to_string(),
        starred: false,
    };

    assert!(unknown.is_leaf());

    match unknown {
        SyntaxNode::UnknownCommand { name, starred } => {
            assert_eq!(name, "foo");
            assert!(!starred);
        }
        _ => panic!("Expected UnknownCommand"),
    }
}

#[test]
fn test_environment_structure() {
    let env = SyntaxNode::Environment {
        name: "matrix".to_string(),
        starred: false,
        args: vec![],
        body: Box::new(SyntaxNode::empty_group(ContentMode::Math)),
    };

    match env {
        SyntaxNode::Environment {
            name,
            starred,
            args,
            body,
        } => {
            assert_eq!(name, "matrix");
            assert!(!starred);
            assert!(args.is_empty());
            assert!(body.is_group());
        }
        _ => panic!("Expected Environment"),
    }
}

#[test]
fn test_declarative_structure() {
    let decl = SyntaxNode::Declarative {
        name: "color".to_string(),
        starred: false,
        args: vec![Argument::mandatory(SyntaxNode::Text("red".to_string()))],
        scope: Box::new(SyntaxNode::Text("text".to_string())),
    };

    match decl {
        SyntaxNode::Declarative {
            name,
            starred,
            args,
            scope,
        } => {
            assert_eq!(name, "color");
            assert!(!starred);
            assert_eq!(args.len(), 1);
            assert!(matches!(*scope, SyntaxNode::Text(_)));
        }
        _ => panic!("Expected Declarative"),
    }
}
