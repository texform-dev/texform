use crate::ast::{ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, Slot};

pub(super) fn drop_spacer(ast: &mut Ast, node_id: NodeId, spacer_names: &[&str]) -> bool {
    let Some(mode) = content_mode(ast, node_id) else {
        return false;
    };

    let Some(link) = ast.parent(node_id) else {
        return false;
    };
    if matches!(link.slot, Slot::GroupChild(_)) {
        match mode {
            ContentMode::Math => drop_math_spacers(ast, link.parent, spacer_names),
            ContentMode::Text => collapse_text_spacer_runs(ast, link.parent, spacer_names),
        }
    } else {
        if mode == ContentMode::Text && command_name_ends_in_letter(ast, node_id) {
            consume_direct_text_argument_separator(ast, node_id);
        }
        let replacement = match mode {
            ContentMode::Math => Node::Group {
                children: Vec::new(),
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            },
            ContentMode::Text => Node::Text(" ".to_string()),
        };
        ast.replace_node_drop_detached_children(node_id, replacement);
    }

    true
}

fn content_mode(ast: &Ast, node_id: NodeId) -> Option<ContentMode> {
    let link = ast.parent(node_id)?;
    match link.slot {
        Slot::GroupChild(_) => match ast.node(link.parent) {
            Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
            _ => None,
        },
        Slot::Argument(index) => {
            let argument = ast.arg_slots(link.parent).get(index)?.as_ref()?;
            match &argument.value {
                ArgumentValue::MathContent(_) | ArgumentValue::OperatorNameContent(_) => {
                    Some(ContentMode::Math)
                }
                ArgumentValue::TextContent(_) => Some(ContentMode::Text),
                _ => None,
            }
        }
        Slot::ScriptBase
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => Some(ContentMode::Math),
        Slot::EnvBody => None,
    }
}

fn drop_math_spacers(ast: &mut Ast, parent: NodeId, spacer_names: &[&str]) {
    let children = ast.children(parent).to_vec();
    let kept = children
        .iter()
        .copied()
        .filter(|child| !is_spacer(ast, *child, spacer_names))
        .collect();
    remove_replaced_children(ast, parent, kept);
}

fn collapse_text_spacer_runs(ast: &mut Ast, parent: NodeId, spacer_names: &[&str]) {
    let children = ast.children(parent).to_vec();
    let mut kept = Vec::with_capacity(children.len());
    let mut run_heads = Vec::new();
    let mut run_tail = None;

    for child in children {
        if is_spacer(ast, child, spacer_names) {
            if run_tail.is_none() {
                kept.push(child);
                run_heads.push(child);
            }
            run_tail = Some(child);
        } else {
            if run_tail.is_some_and(|tail| command_name_ends_in_letter(ast, tail))
                && let Node::Text(text) = ast.node(child)
                && text.starts_with(' ')
                && let Node::Text(text) = ast.node_opt_mut(child).expect("text node should exist")
            {
                text.remove(0);
            }
            kept.push(child);
            run_tail = None;
        }
    }

    for head in run_heads {
        ast.replace_node_drop_detached_children(head, Node::Text(" ".to_string()));
    }
    remove_replaced_children(ast, parent, kept);
}

fn remove_replaced_children(ast: &mut Ast, parent: NodeId, children: Vec<NodeId>) {
    for removed in ast.replace_children(parent, children) {
        ast.remove_detached(removed);
    }
}

fn consume_direct_text_argument_separator(ast: &mut Ast, node_id: NodeId) {
    let mut argument_owner = node_id;
    loop {
        let Some(link) = ast.parent(argument_owner) else {
            return;
        };
        let Slot::Argument(index) = link.slot else {
            return;
        };
        let Some(argument) = ast.arg_slots(link.parent).get(index).and_then(Option::as_ref) else {
            return;
        };
        if !matches!(argument.value, ArgumentValue::TextContent(child) if child == argument_owner) {
            return;
        }

        argument_owner = link.parent;
        match ast.slot(argument_owner) {
            Some(Slot::Argument(_)) => {}
            Some(Slot::GroupChild(_)) => {
                let Some(next) = ast.next_sibling(argument_owner) else {
                    return;
                };
                if let Some(Node::Text(text)) = ast.node_opt_mut(next)
                    && text.starts_with(' ')
                {
                    text.remove(0);
                }
                return;
            }
            _ => return,
        }
    }
}

fn command_name_ends_in_letter(ast: &Ast, node_id: NodeId) -> bool {
    let Node::Command { name, .. } = ast.node(node_id) else {
        return false;
    };
    name.as_bytes().last().is_some_and(u8::is_ascii_alphabetic)
}

fn is_spacer(ast: &Ast, node_id: NodeId, spacer_names: &[&str]) -> bool {
    matches!(
        ast.node(node_id),
        Node::Command {
            name,
            args,
            known: true,
        } if args.is_empty() && spacer_names.contains(&name.as_str())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spacer(ast: &mut Ast, name: &str) -> NodeId {
        ast.new_node(Node::Command {
            name: name.to_string(),
            args: Vec::new(),
            known: true,
        })
    }

    #[test]
    fn collapses_each_text_run_to_one_ordinary_space() {
        let mut ast = Ast::new();
        let text_group = ast.new_node(Node::Group {
            children: Vec::new(),
            kind: GroupKind::Explicit,
            mode: ContentMode::Text,
        });
        let a = ast.new_node(Node::Text("A".to_string()));
        let first = spacer(&mut ast, "quad");
        let second = spacer(&mut ast, "quad");
        let x = ast.new_node(Node::Text("X".to_string()));
        let third = spacer(&mut ast, "quad");
        let b = ast.new_node(Node::Text("B".to_string()));
        for child in [a, first, second, x, third, b] {
            ast.append_child(text_group, child);
        }
        ast.append_child(ast.root(), text_group);

        assert!(drop_spacer(&mut ast, second, &["quad"]));

        let children = ast.children(text_group);
        assert_eq!(children.len(), 5);
        assert_eq!(ast.node(children[0]), &Node::Text("A".to_string()));
        assert_eq!(ast.node(children[1]), &Node::Text(" ".to_string()));
        assert_eq!(ast.node(children[2]), &Node::Text("X".to_string()));
        assert_eq!(ast.node(children[3]), &Node::Text(" ".to_string()));
        assert_eq!(ast.node(children[4]), &Node::Text("B".to_string()));
        assert!(!ast.contains(second));
        ast.assert_invariants();
    }

    #[test]
    fn removes_all_matching_math_spacers_from_the_container() {
        let mut ast = Ast::new();
        let first = spacer(&mut ast, "quad");
        let x = ast.new_node(Node::Char('x'));
        let second = spacer(&mut ast, "quad");
        for child in [first, x, second] {
            ast.append_child(ast.root(), child);
        }

        assert!(drop_spacer(&mut ast, second, &["quad"]));

        assert_eq!(ast.children(ast.root()), &[x]);
        assert!(!ast.contains(first));
        assert!(!ast.contains(second));
        ast.assert_invariants();
    }
}
