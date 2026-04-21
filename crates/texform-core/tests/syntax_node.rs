use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, GroupKind, SyntaxNode,
};

fn content_argument(node: SyntaxNode) -> Argument {
    Argument::from_value(ArgumentKind::Mandatory, ArgumentValue::TextContent(node))
}

#[test]
fn leaf_and_group_helpers_report_meaningful_structure() {
    let char_node = SyntaxNode::Char('a');
    let group = SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: GroupKind::Explicit,
        children: vec![SyntaxNode::Char('x')],
    };

    assert!(char_node.is_leaf());
    assert!(group.is_group());
    assert_eq!(group.group_mode(), Some(ContentMode::Math));
}

#[test]
fn root_preserves_group_helper_semantics() {
    let root = SyntaxNode::root(ContentMode::Text, vec![SyntaxNode::Text("x".to_string())]);

    assert!(root.is_group());
    assert_eq!(root.group_mode(), Some(ContentMode::Text));
}

#[test]
fn implicit_group_helpers_preserve_mode_and_kind() {
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
fn argument_from_value_preserves_kind_and_variant() {
    let arg = Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::TextContent(SyntaxNode::Char('x')),
    );

    assert_eq!(arg.kind, ArgumentKind::Mandatory);
    assert_eq!(arg.value, ArgumentValue::TextContent(SyntaxNode::Char('x')));
}

#[test]
fn content_mode_display_helpers_match_public_strings() {
    assert_eq!(ContentMode::Math.as_str(), "math");
    assert_eq!(ContentMode::Text.as_str(), "text");
    assert_eq!(ContentMode::Math.to_string(), "math");
    assert_eq!(ContentMode::Text.to_string(), "text");
}

#[test]
fn command_is_leaf_only_without_content_arguments() {
    let leaf = SyntaxNode::Command {
        name: "alpha".to_string(),
        args: vec![],
        known: true,
    };
    let non_leaf = SyntaxNode::Command {
        name: "text".to_string(),
        args: vec![Some(content_argument(SyntaxNode::Char('x')))],
        known: true,
    };

    assert!(leaf.is_leaf());
    assert!(!non_leaf.is_leaf());
}

#[test]
fn declarative_is_leaf_only_without_content_arguments() {
    let leaf = SyntaxNode::Declarative {
        name: "bfseries".to_string(),
        args: vec![],
    };
    let non_leaf = SyntaxNode::Declarative {
        name: "color".to_string(),
        args: vec![Some(content_argument(SyntaxNode::Text("red".to_string())))],
    };

    assert!(leaf.is_leaf());
    assert!(!non_leaf.is_leaf());
}

#[test]
fn display_merges_adjacent_chars_without_losing_node_boundaries() {
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
fn display_keeps_non_char_boundaries_visible() {
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
fn default_display_marks_root_node() {
    let root = SyntaxNode::root(ContentMode::Math, vec![SyntaxNode::Char('x')]);
    let display = format!("{}", root);

    assert!(display.contains("Root(Math)"));
    assert!(!display.contains("Group(Math, root)"));
}

#[test]
fn display_standalone_implicit_group_is_not_marked_as_root() {
    let group = SyntaxNode::implicit_group(ContentMode::Math, vec![SyntaxNode::Char('x')]);
    let display = format!("{}", group);

    assert!(display.contains("Group(Math, Implicit)"));
    assert!(!display.contains("Root("));
}

#[test]
fn error_node_display_and_leaf_status() {
    let error = SyntaxNode::Error {
        message: "invalid \\left delimiter".to_string(),
        snippet: "\\left\\foo x \\right)".to_string(),
    };

    assert!(error.is_leaf());

    let display = error.to_string();
    assert!(display.contains("invalid \\left delimiter"));
    assert!(display.contains("\\left\\foo x \\right)"));
}
