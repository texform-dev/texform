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
fn test_root_preserves_group_helper_semantics() {
    let root = SyntaxNode::root(ContentMode::Text, vec![SyntaxNode::Text("x".to_string())]);

    assert!(root.is_group());
    assert_eq!(root.group_mode(), Some(ContentMode::Text));
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
    let mandatory = Argument::mandatory(ContentMode::Math, node.clone());
    assert_eq!(mandatory.kind, ArgumentKind::Mandatory);
    assert_eq!(
        mandatory.value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );

    let optional = Argument::optional(ContentMode::Math, node);
    assert_eq!(optional.kind, ArgumentKind::Optional);
    assert_eq!(
        optional.value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
}

#[test]
fn test_text_argument_uses_text_content_variant_for_single_char_item() {
    let arg = Argument::mandatory(ContentMode::Text, SyntaxNode::Char('x'));

    assert_eq!(arg.kind, ArgumentKind::Mandatory);
    assert_eq!(arg.value, ArgumentValue::TextContent(SyntaxNode::Char('x')));
}

#[test]
fn test_content_mode_helpers() {
    assert!(ContentMode::Math.is_math());
    assert!(!ContentMode::Math.is_text());
    assert!(ContentMode::Text.is_text());
    assert!(!ContentMode::Text.is_math());
    assert_eq!(ContentMode::Math.to_string(), "math");
    assert_eq!(ContentMode::Text.to_string(), "text");
}

#[test]
fn test_command_node() {
    let cmd = SyntaxNode::Command {
        name: "frac".to_string(),
        args: vec![
            Some(Argument::mandatory(
                ContentMode::Math,
                SyntaxNode::Char('a'),
            )),
            Some(Argument::mandatory(
                ContentMode::Math,
                SyntaxNode::Char('b'),
            )),
        ],
        known: true,
    };

    match cmd {
        SyntaxNode::Command { name, args, known } => {
            assert_eq!(name, "frac");
            assert_eq!(args.len(), 2);
            assert!(known);
        }
        _ => panic!("Expected Command"),
    }
}

#[test]
fn test_infix_node() {
    let infix = SyntaxNode::Infix {
        name: "over".to_string(),
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
    assert!(display.contains("Chars(\"xy\")"));
    assert!(!display.contains("Char('x')"));
    assert!(!display.contains("Char('y')"));
}

#[test]
fn test_display_char_merging_boundaries() {
    let group = SyntaxNode::implicit_group(
        ContentMode::Math,
        vec![
            SyntaxNode::Char('a'),
            SyntaxNode::Char('b'),
            SyntaxNode::ActiveSpace,
            SyntaxNode::Char('c'),
        ],
    );
    let display = format!("{}", group);

    assert!(display.contains("Chars(\"ab\")"));
    assert!(display.contains("ActiveSpace"));
    assert!(display.contains("Char('c')"));
}

#[test]
fn test_default_display_marks_root_node() {
    let root = SyntaxNode::root(ContentMode::Math, vec![SyntaxNode::Char('x')]);
    let display = format!("{}", root);

    assert!(display.contains("Root(Math)"));
    assert!(!display.contains("Group(Math, root)"));
}

#[test]
fn test_display_standalone_implicit_group() {
    // An implicit group that is not the top-level root must still display as
    // a regular group, not as the root marker.
    let group = SyntaxNode::implicit_group(ContentMode::Math, vec![SyntaxNode::Char('x')]);
    let display = format!("{}", group);

    assert!(display.contains("Group(Math, Implicit)"));
    assert!(!display.contains("Root("));
}

#[test]
fn test_unknown_command() {
    let unknown = SyntaxNode::Command {
        name: "foo".to_string(),
        args: vec![],
        known: false,
    };

    assert!(unknown.is_leaf());

    match unknown {
        SyntaxNode::Command { name, args, known } => {
            assert_eq!(name, "foo");
            assert!(args.is_empty());
            assert!(!known);
        }
        _ => panic!("Expected unknown Command"),
    }
}

#[test]
fn test_error_node_display_and_leaf_status() {
    let error = SyntaxNode::Error {
        message: "invalid \\left delimiter".to_string(),
        snippet: "\\left\\foo x \\right)".to_string(),
    };

    assert!(error.is_leaf());

    let display = error.to_string();
    assert!(display.contains("invalid \\left delimiter"));
    assert!(display.contains("\\left\\foo x \\right)"));
}

#[test]
fn test_environment_structure() {
    let env = SyntaxNode::Environment {
        name: "matrix".to_string(),
        args: vec![],
        known: true,
        body: Box::new(SyntaxNode::empty_group(ContentMode::Math)),
    };

    match env {
        SyntaxNode::Environment {
            name,
            args,
            known,
            body,
        } => {
            assert_eq!(name, "matrix");
            assert!(args.is_empty());
            assert!(known);
            assert!(body.is_group());
        }
        _ => panic!("Expected Environment"),
    }
}

#[test]
fn test_declarative_structure() {
    let decl = SyntaxNode::Declarative {
        name: "color".to_string(),
        args: vec![Some(Argument::mandatory(
            ContentMode::Text,
            SyntaxNode::Text("red".to_string()),
        ))],
        scope: Box::new(SyntaxNode::Text("text".to_string())),
    };

    match decl {
        SyntaxNode::Declarative { name, args, scope } => {
            assert_eq!(name, "color");
            assert_eq!(args.len(), 1);
            assert!(matches!(*scope, SyntaxNode::Text(_)));
        }
        _ => panic!("Expected Declarative"),
    }
}
