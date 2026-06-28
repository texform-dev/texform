use texform_core::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId,
    NodeKind, ParentLink, Slot,
};

fn explicit_group(ast: &mut Ast, mode: ContentMode) -> NodeId {
    let id = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Explicit,
        mode,
    });
    ast.assert_invariants();
    id
}

fn content_arg(kind: ArgumentKind, child: NodeId) -> ArgumentSlot {
    Some(Argument::from_value(
        kind,
        ArgumentValue::MathContent(child),
    ))
}

#[test]
fn test_new_ast_starts_with_math_root() {
    let ast = Ast::new();
    let root = ast.root();

    assert_eq!(ast.kind(root), NodeKind::Root);
    assert!(ast.parent(root).is_none());

    match ast.node(root) {
        Node::Root { children, mode } => {
            assert!(children.is_empty());
            assert_eq!(*mode, ContentMode::Math);
        }
        other => panic!("Expected root node, got {:?}", other),
    }

    ast.assert_invariants();
}

#[test]
#[should_panic(expected = "Cannot create detached root node")]
fn test_new_node_rejects_root_variant() {
    let mut ast = Ast::new();
    let _ = ast.new_node(Node::Root {
        children: Vec::new(),
        mode: ContentMode::Math,
    });
}

#[test]
#[should_panic(expected = "Cannot replace node with root variant")]
fn test_replace_node_rejects_root_variant() {
    let mut ast = Ast::new();
    let root = ast.root();

    let child = ast.new_node(Node::Char('x'));
    ast.assert_invariants();
    ast.append_child(root, child);
    ast.assert_invariants();

    let _ = ast.replace_node(
        child,
        Node::Root {
            children: Vec::new(),
            mode: ContentMode::Math,
        },
    );
}

#[test]
fn test_group_child_editing_updates_navigation_and_detached_roots() {
    let mut ast = Ast::new();
    let root = ast.root();

    let a = ast.new_node(Node::Char('a'));
    ast.assert_invariants();
    let b = ast.new_node(Node::Char('b'));
    ast.assert_invariants();
    let c = ast.new_node(Node::Char('c'));
    ast.assert_invariants();

    ast.append_child(root, a);
    ast.assert_invariants();
    ast.append_child(root, c);
    ast.assert_invariants();
    ast.insert_child(root, 1, b);
    ast.assert_invariants();

    assert_eq!(ast.children(root), [a, b, c].as_slice());
    assert_eq!(
        ast.parent(a),
        Some(ParentLink {
            parent: root,
            slot: Slot::GroupChild(0),
        })
    );
    assert_eq!(
        ast.parent(b),
        Some(ParentLink {
            parent: root,
            slot: Slot::GroupChild(1),
        })
    );
    assert_eq!(
        ast.parent(c),
        Some(ParentLink {
            parent: root,
            slot: Slot::GroupChild(2),
        })
    );
    assert_eq!(ast.next_sibling(a), Some(b));
    assert_eq!(ast.next_sibling(b), Some(c));
    assert_eq!(ast.prev_sibling(c), Some(b));
    assert_eq!(ast.prev_sibling(a), None);

    let detached = ast.detach(b);
    ast.assert_invariants();

    assert_eq!(detached, b);
    assert_eq!(ast.children(root), [a, c].as_slice());
    assert_eq!(ast.parent(b), None);
    assert_eq!(ast.next_sibling(a), Some(c));
    assert_eq!(ast.prev_sibling(c), Some(a));

    let removed = ast.remove_detached(detached);
    ast.assert_invariants();

    assert_eq!(removed, Node::Char('b'));
    assert!(!ast.contains(b));
}

#[test]
fn test_command_arg_slots_keep_non_content_values_out_of_tree_edges() {
    let mut ast = Ast::new();
    let root = ast.root();

    let group = explicit_group(&mut ast, ContentMode::Math);
    let x = ast.new_node(Node::Char('x'));
    ast.assert_invariants();
    ast.append_child(group, x);
    ast.assert_invariants();

    let command = ast.new_node(Node::Command {
        name: "probe".to_string(),
        args: vec![
            Some(Argument {
                kind: ArgumentKind::Star,
                no_leading_space: false,
                value: ArgumentValue::Boolean(true),
            }),
            None,
            content_arg(ArgumentKind::Mandatory, group),
            Some(Argument {
                kind: ArgumentKind::Optional,
                no_leading_space: false,
                value: ArgumentValue::Dimension("1em".to_string()),
            }),
        ],
        known: true,
    });
    ast.assert_invariants();

    ast.append_child(root, command);
    ast.assert_invariants();

    let slots = ast.arg_slots(command);
    assert_eq!(slots.len(), 4);

    let star = slots[0]
        .as_ref()
        .unwrap_or_else(|| panic!("Expected star slot to be present"));
    assert_eq!(star.kind, ArgumentKind::Star);
    assert_eq!(star.value, ArgumentValue::Boolean(true));

    assert!(slots[1].is_none());

    let dimension = slots[3]
        .as_ref()
        .unwrap_or_else(|| panic!("Expected dimension slot to be present"));
    assert_eq!(dimension.value, ArgumentValue::Dimension("1em".to_string()));

    assert_eq!(ast.edges(command), vec![(group, Slot::Argument(2))]);
    assert_eq!(
        ast.parent(group),
        Some(ParentLink {
            parent: command,
            slot: Slot::Argument(2),
        })
    );
    assert_eq!(
        ast.find(root, |node| matches!(node, Node::Char('x'))),
        Some(x)
    );
    assert_eq!(
        ast.find_all(root, |node| matches!(node, Node::Char(_))),
        vec![x]
    );
}

#[test]
fn test_command_arg_slots_keep_scalar_variants_opaque() {
    let mut ast = Ast::new();
    let root = ast.root();

    let content = explicit_group(&mut ast, ContentMode::Math);
    let x = ast.new_node(Node::Char('x'));
    ast.append_child(content, x);

    let command = ast.new_node(Node::Command {
        name: "probe".to_string(),
        args: vec![
            Some(Argument {
                kind: ArgumentKind::Optional,
                no_leading_space: false,
                value: ArgumentValue::Integer("12".to_string()),
            }),
            Some(Argument {
                kind: ArgumentKind::Mandatory,
                no_leading_space: false,
                value: ArgumentValue::MathContent(content),
            }),
        ],
        known: true,
    });
    ast.append_child(root, command);
    ast.assert_invariants();

    assert_eq!(ast.edges(command), vec![(content, Slot::Argument(1))]);
}

#[test]
fn test_replace_node_reuses_children_and_detaches_removed_subtrees() {
    let mut ast = Ast::new();
    let root = ast.root();

    let lhs = explicit_group(&mut ast, ContentMode::Math);
    let a = ast.new_node(Node::Char('a'));
    ast.assert_invariants();
    ast.append_child(lhs, a);
    ast.assert_invariants();

    let rhs = explicit_group(&mut ast, ContentMode::Math);
    let b = ast.new_node(Node::Char('b'));
    ast.assert_invariants();
    ast.append_child(rhs, b);
    ast.assert_invariants();

    let command = ast.new_node(Node::Command {
        name: "pair".to_string(),
        args: vec![
            content_arg(ArgumentKind::Mandatory, lhs),
            content_arg(ArgumentKind::Mandatory, rhs),
        ],
        known: true,
    });
    ast.assert_invariants();
    ast.append_child(root, command);
    ast.assert_invariants();

    let sub_id = ast.new_node(Node::Char('i'));
    ast.assert_invariants();

    let old_node = ast.replace_node(
        command,
        Node::Scripted {
            base: lhs,
            subscript: Some(sub_id),
            superscript: None,
        },
    );
    ast.assert_invariants();

    match old_node {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "pair");
            assert_eq!(args.len(), 2);
        }
        other => panic!("Expected old command node, got {:?}", other),
    }

    match ast.node(command) {
        Node::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(*base, lhs);
            assert_eq!(*subscript, Some(sub_id));
            assert_eq!(*superscript, None);
        }
        other => panic!("Expected scripted node, got {:?}", other),
    }

    assert_eq!(
        ast.parent(lhs),
        Some(ParentLink {
            parent: command,
            slot: Slot::ScriptBase,
        })
    );
    assert_eq!(
        ast.parent(sub_id),
        Some(ParentLink {
            parent: command,
            slot: Slot::ScriptSub,
        })
    );
    assert_eq!(ast.parent(rhs), None);

    let removed_rhs = ast.remove_detached(rhs);
    ast.assert_invariants();

    match removed_rhs {
        Node::Group { kind, mode, .. } => {
            assert_eq!(kind, GroupKind::Explicit);
            assert_eq!(mode, ContentMode::Math);
        }
        other => panic!("Expected detached group, got {:?}", other),
    }

    assert!(!ast.contains(rhs));
    assert!(!ast.contains(b));
}

#[test]
fn test_remove_node_deletes_attached_subtree() {
    let mut ast = Ast::new();
    let root = ast.root();

    let numerator = explicit_group(&mut ast, ContentMode::Math);
    let a = ast.new_node(Node::Char('a'));
    ast.assert_invariants();
    ast.append_child(numerator, a);
    ast.assert_invariants();

    let denominator = explicit_group(&mut ast, ContentMode::Math);
    let b = ast.new_node(Node::Char('b'));
    ast.assert_invariants();
    ast.append_child(denominator, b);
    ast.assert_invariants();

    let frac = ast.new_node(Node::Command {
        name: "frac".to_string(),
        args: vec![
            content_arg(ArgumentKind::Mandatory, numerator),
            content_arg(ArgumentKind::Mandatory, denominator),
        ],
        known: true,
    });
    ast.assert_invariants();
    ast.append_child(root, frac);
    ast.assert_invariants();

    ast.remove_node(frac);
    ast.assert_invariants();

    assert!(ast.children(root).is_empty());
    assert!(!ast.contains(frac));
    assert!(!ast.contains(numerator));
    assert!(!ast.contains(denominator));
    assert!(!ast.contains(a));
    assert!(!ast.contains(b));
}

#[test]
#[should_panic(expected = "Cannot attach child that already has a parent")]
fn test_append_child_rejects_attached_child() {
    let mut ast = Ast::new();
    let root = ast.root();

    let group = explicit_group(&mut ast, ContentMode::Math);
    let child = ast.new_node(Node::Char('x'));
    ast.assert_invariants();

    ast.append_child(root, group);
    ast.assert_invariants();
    ast.append_child(group, child);
    ast.assert_invariants();

    ast.append_child(root, child);
}

#[test]
#[should_panic(expected = "Can only detach GroupChild nodes")]
fn test_detach_rejects_non_group_child_nodes() {
    let mut ast = Ast::new();
    let root = ast.root();

    let arg = explicit_group(&mut ast, ContentMode::Math);
    let command = ast.new_node(Node::Command {
        name: "cmd".to_string(),
        args: vec![content_arg(ArgumentKind::Mandatory, arg)],
        known: true,
    });
    ast.assert_invariants();

    ast.append_child(root, command);
    ast.assert_invariants();

    ast.detach(arg);
}

#[test]
#[should_panic(expected = "Can only remove detached roots")]
fn test_remove_detached_rejects_attached_nodes() {
    let mut ast = Ast::new();
    let root = ast.root();

    let child = ast.new_node(Node::Char('x'));
    ast.assert_invariants();

    ast.append_child(root, child);
    ast.assert_invariants();

    ast.remove_detached(child);
}

#[test]
#[should_panic(expected = "Environment body must be a Group node")]
fn test_environment_body_must_be_group() {
    let mut ast = Ast::new();

    let body = ast.new_node(Node::Char('x'));
    ast.assert_invariants();

    let _ = ast.new_node(Node::Environment {
        name: "aligned".to_string(),
        args: vec![],
        known: true,
        body,
    });
}
