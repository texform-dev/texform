use texform::ast::{
    Argument,
    ArgumentKind,
    Ast,
    ContentMode,
    GroupKind,
    Node,
    Slot,
};

#[test]
fn test_ast_creation() {
    let ast = Ast::new();
    let root = ast.root();

    // Root should be a Group with Math mode
    match ast.node(root) {
        Node::Group {
            kind,
            mode,
            children,
        } => {
            assert_eq!(*kind, GroupKind::Implicit);
            assert_eq!(*mode, ContentMode::Math);
            assert!(children.is_empty());
        }
        _ => panic!("Root should be a Group"),
    }

    // Root should have no parent
    assert!(ast.parent(root).is_none());
}

#[test]
fn test_simple_tree_construction() {
    let mut ast = Ast::new();
    let root = ast.root();

    // Create some leaf nodes
    let char_a = ast.new_node(Node::Char('a'));
    let char_plus = ast.new_node(Node::Char('+'));
    let char_b = ast.new_node(Node::Char('b'));

    // Append to root
    ast.append_child(root, char_a);
    ast.append_child(root, char_plus);
    ast.append_child(root, char_b);

    // Verify children
    let children = ast.children(root);
    assert_eq!(children.len(), 3);
    assert_eq!(children[0], char_a);
    assert_eq!(children[1], char_plus);
    assert_eq!(children[2], char_b);

    // Verify parent links
    assert_eq!(ast.parent_id(char_a), Some(root));
    assert_eq!(ast.slot(char_a), Some(Slot::GroupChild(0)));

    assert_eq!(ast.parent_id(char_plus), Some(root));
    assert_eq!(ast.slot(char_plus), Some(Slot::GroupChild(1)));

    assert_eq!(ast.parent_id(char_b), Some(root));
    assert_eq!(ast.slot(char_b), Some(Slot::GroupChild(2)));
}

#[test]
fn test_sibling_navigation() {
    let mut ast = Ast::new();
    let root = ast.root();

    let a = ast.new_node(Node::Char('a'));
    let b = ast.new_node(Node::Char('b'));
    let c = ast.new_node(Node::Char('c'));

    ast.append_child(root, a);
    ast.append_child(root, b);
    ast.append_child(root, c);

    // Test next_sibling
    assert_eq!(ast.next_sibling(a), Some(b));
    assert_eq!(ast.next_sibling(b), Some(c));
    assert_eq!(ast.next_sibling(c), None);

    // Test prev_sibling
    assert_eq!(ast.prev_sibling(a), None);
    assert_eq!(ast.prev_sibling(b), Some(a));
    assert_eq!(ast.prev_sibling(c), Some(b));
}

#[test]
fn test_scripted_node() {
    let mut ast = Ast::new();

    let x = ast.new_node(Node::Char('x'));
    let two = ast.new_node(Node::Char('2'));

    let scripted = ast.new_node(Node::Scripted {
        base: x,
        subscript: None,
        superscript: Some(two),
    });

    ast.append_child(ast.root(), scripted);

    // Verify scripted node structure
    assert_eq!(ast.script_base(scripted), x);
    assert_eq!(ast.script_sub(scripted), None);
    assert_eq!(ast.script_sup(scripted), Some(two));

    // Verify parent links
    assert_eq!(ast.parent_id(x), Some(scripted));
    assert_eq!(ast.slot(x), Some(Slot::ScriptBase));

    assert_eq!(ast.parent_id(two), Some(scripted));
    assert_eq!(ast.slot(two), Some(Slot::ScriptSup));
}

#[test]
fn test_command_with_arguments() {
    let mut ast = Ast::new();

    // Create \frac{a}{b}
    let a_group = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let char_a = ast.new_node(Node::Char('a'));
    ast.append_child(a_group, char_a);

    let b_group = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let char_b = ast.new_node(Node::Char('b'));
    ast.append_child(b_group, char_b);

    let frac = ast.new_node(Node::Command {
        name: "frac".to_string(),
        starred: false,
        args: vec![
            Argument {
                kind: ArgumentKind::Mandatory,
                content: a_group,
            },
            Argument {
                kind: ArgumentKind::Mandatory,
                content: b_group,
            },
        ],
    });

    ast.append_child(ast.root(), frac);

    // Verify arguments
    let args = ast.args(frac);
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].content, a_group);
    assert_eq!(args[1].content, b_group);

    // Verify parent links
    assert_eq!(ast.parent_id(a_group), Some(frac));
    assert_eq!(ast.slot(a_group), Some(Slot::Argument(0)));

    assert_eq!(ast.parent_id(b_group), Some(frac));
    assert_eq!(ast.slot(b_group), Some(Slot::Argument(1)));
}

#[test]
fn test_environment_body_parent_links() {
    let mut ast = Ast::new();

    let body = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });

    let env = ast.new_node(Node::Environment {
        name: "aligned".to_string(),
        starred: false,
        args: vec![],
        body,
    });

    ast.append_child(ast.root(), env);

    assert_eq!(ast.parent_id(body), Some(env));
    assert_eq!(ast.slot(body), Some(Slot::EnvBody));
}

#[test]
#[should_panic(expected = "Environment body must be a Group node")]
fn test_environment_body_must_be_group() {
    let mut ast = Ast::new();

    let not_group = ast.new_node(Node::Char('x'));
    let env = ast.new_node(Node::Environment {
        name: "aligned".to_string(),
        starred: false,
        args: vec![],
        body: not_group,
    });

    ast.append_child(ast.root(), env);
}

#[test]
fn test_find_operations() {
    let mut ast = Ast::new();
    let root = ast.root();

    let a = ast.new_node(Node::Char('a'));
    let b = ast.new_node(Node::Char('b'));
    let text = ast.new_node(Node::Text("hello".to_string()));

    ast.append_child(root, a);
    ast.append_child(root, b);
    ast.append_child(root, text);

    // Find first char
    let first_char = ast.find(root, |node| matches!(node, Node::Char(_)));
    assert_eq!(first_char, Some(a));

    // Find all chars
    let all_chars = ast.find_all(root, |node| matches!(node, Node::Char(_)));
    assert_eq!(all_chars.len(), 2);
    assert!(all_chars.contains(&a));
    assert!(all_chars.contains(&b));

    // Find text node
    let text_node = ast.find(root, |node| matches!(node, Node::Text(_)));
    assert_eq!(text_node, Some(text));
}

#[test]
fn test_insert_and_remove_child() {
    let mut ast = Ast::new();
    let root = ast.root();

    let a = ast.new_node(Node::Char('a'));
    let b = ast.new_node(Node::Char('b'));
    let c = ast.new_node(Node::Char('c'));

    ast.append_child(root, a);
    ast.append_child(root, c);

    // Insert b between a and c
    ast.insert_child(root, 1, b);

    // Verify children order
    let children = ast.children(root);
    assert_eq!(children.len(), 3);
    assert_eq!(children[0], a);
    assert_eq!(children[1], b);
    assert_eq!(children[2], c);

    // Verify slots are updated
    assert_eq!(ast.slot(a), Some(Slot::GroupChild(0)));
    assert_eq!(ast.slot(b), Some(Slot::GroupChild(1)));
    assert_eq!(ast.slot(c), Some(Slot::GroupChild(2)));

    // Detach b
    let detached = ast.detach_child(root, 1);
    assert_eq!(detached, b);

    // Verify children after detachment
    let children = ast.children(root);
    assert_eq!(children.len(), 2);
    assert_eq!(children[0], a);
    assert_eq!(children[1], c);

    // Verify slots are updated after detachment
    assert_eq!(ast.slot(a), Some(Slot::GroupChild(0)));
    assert_eq!(ast.slot(c), Some(Slot::GroupChild(1)));
    assert_eq!(ast.parent(b), None);

    // Delete the detached node to reclaim memory
    ast.delete_subtree(b);
}

#[test]
fn test_delete_subtree() {
    let mut ast = Ast::new();

    // Create a tree: frac{a+b}{c}
    let a = ast.new_node(Node::Char('a'));
    let plus = ast.new_node(Node::Char('+'));
    let b = ast.new_node(Node::Char('b'));

    let numerator = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    ast.append_child(numerator, a);
    ast.append_child(numerator, plus);
    ast.append_child(numerator, b);

    let c = ast.new_node(Node::Char('c'));
    let denominator = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    ast.append_child(denominator, c);

    let frac = ast.new_node(Node::Command {
        name: "frac".to_string(),
        starred: false,
        args: vec![
            Argument {
                kind: ArgumentKind::Mandatory,
                content: numerator,
            },
            Argument {
                kind: ArgumentKind::Mandatory,
                content: denominator,
            },
        ],
    });

    ast.append_child(ast.root(), frac);

    // Verify all nodes are in the tree
    assert!(ast.contains(frac));
    assert!(ast.contains(numerator));
    assert!(ast.contains(denominator));
    assert!(ast.contains(a));
    assert!(ast.contains(plus));
    assert!(ast.contains(b));
    assert!(ast.contains(c));

    // Delete the frac node and its entire subtree
    ast.remove_node(frac);

    // Verify all nodes in the subtree are deleted
    assert!(!ast.contains(frac));
    assert!(!ast.contains(numerator));
    assert!(!ast.contains(denominator));
    assert!(!ast.contains(a));
    assert!(!ast.contains(plus));
    assert!(!ast.contains(b));
    assert!(!ast.contains(c));
}

#[test]
#[should_panic(expected = "Cannot append child that already has a parent")]
fn test_append_child_with_existing_parent_panics() {
    let mut ast = Ast::new();

    let group1 = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let group2 = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });

    let child = ast.new_node(Node::Char('x'));

    // First append is fine
    ast.append_child(group1, child);
    // Second append should panic because child already has a parent
    ast.append_child(group2, child);
}

#[test]
#[should_panic(expected = "Cannot insert child that already has a parent")]
fn test_insert_child_with_existing_parent_panics() {
    let mut ast = Ast::new();

    let group1 = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let group2 = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });

    let child = ast.new_node(Node::Char('x'));

    // First append is fine
    ast.append_child(group1, child);
    // Insert should panic because child already has a parent
    ast.insert_child(group2, 0, child);
}

#[test]
fn test_detach_and_reattach() {
    let mut ast = Ast::new();

    let group1 = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let group2 = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });

    let child = ast.new_node(Node::Char('x'));

    // Attach to group1
    ast.append_child(group1, child);
    assert_eq!(ast.parent_id(child), Some(group1));

    // Detach from group1
    let detached = ast.detach_child(group1, 0);
    assert_eq!(detached, child);
    assert_eq!(ast.parent_id(child), None);

    // Now can attach to group2
    ast.append_child(group2, child);
    assert_eq!(ast.parent_id(child), Some(group2));
}

#[test]
#[should_panic(expected = "already has a parent")]
fn test_replace_node_with_attached_children_panics() {
    let mut ast = Ast::new();

    let group = ast.new_node(Node::Group {
        children: vec![],
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });

    let child = ast.new_node(Node::Char('x'));
    ast.append_child(group, child);

    let old_node = ast.new_node(Node::Char('a'));
    ast.append_child(ast.root(), old_node);

    // Try to replace old_node with a Command that uses the attached child
    let new_node = Node::Command {
        name: "test".to_string(),
        starred: false,
        args: vec![Argument {
            kind: ArgumentKind::Mandatory,
            content: child,
        }],
    };

    ast.replace_node(old_node, new_node);
}

#[test]
fn test_replace_node_with_detached_children() {
    let mut ast = Ast::new();

    let child = ast.new_node(Node::Char('x'));
    let old_node = ast.new_node(Node::Char('a'));
    ast.append_child(ast.root(), old_node);

    let new_node = Node::Command {
        name: "test".to_string(),
        starred: false,
        args: vec![Argument {
            kind: ArgumentKind::Mandatory,
            content: child,
        }],
    };

    let replaced = ast.replace_node(old_node, new_node);
    assert_eq!(replaced, Node::Char('a'));

    // Verify the new structure
    match ast.node(old_node) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "test");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].content, child);
        }
        _ => panic!("Should be a Command"),
    }

    // Verify child has correct parent
    assert_eq!(ast.parent_id(child), Some(old_node));
    assert_eq!(ast.slot(child), Some(Slot::Argument(0)));
}
