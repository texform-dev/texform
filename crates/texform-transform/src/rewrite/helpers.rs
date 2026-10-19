//! Convenience constructors for building AST argument slots and nodes.
//!
//! Transform rules frequently need to assemble replacement [`Node`] trees.
//! The helpers here eliminate the boilerplate of constructing [`Argument`]
//! wrappers by hand, keeping rule implementations focused on semantics.

use texform_knowledge::builtin::base;
use texform_knowledge::specs::BuiltinCommandRecord;

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind,
    Node, NodeId, Slot,
};

/// Creates a mandatory content argument slot wrapping the subtree rooted at `node_id`.
pub fn mandatory_content_slot(node_id: NodeId, mode: ContentMode) -> ArgumentSlot {
    let value = match mode {
        ContentMode::Math => ArgumentValue::MathContent(node_id),
        ContentMode::Text => ArgumentValue::TextContent(node_id),
    };
    Some(Argument::from_value(ArgumentKind::Mandatory, value))
}

/// Creates a mandatory operator-name content argument slot.
pub fn mandatory_operator_name_slot(node_id: NodeId) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::OperatorNameContent(node_id),
    ))
}

/// Creates a mandatory delimiter argument slot.
pub fn delimiter_slot(delimiter: Delimiter) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::Delimiter(delimiter),
    ))
}

/// Creates a mandatory dimension argument slot.
pub fn dimension_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::Dimension(value.into()),
    ))
}

/// Creates a mandatory integer argument slot.
pub fn integer_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::Integer(value.into()),
    ))
}

/// Creates the two mandatory content arguments used when converting an infix node to a prefix command.
pub fn infix_prefix_args(left: NodeId, right: NodeId, mode: ContentMode) -> Vec<ArgumentSlot> {
    vec![
        mandatory_content_slot(left, mode),
        mandatory_content_slot(right, mode),
    ]
}

/// Creates a star (boolean) argument slot, representing a `*` modifier on a command.
pub fn star_slot(value: bool) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Star,
        ArgumentValue::Boolean(value),
    ))
}

/// Creates a known command node with no arguments.
pub fn bare_command_node(name: &str) -> Node {
    Node::Command {
        name: name.to_string(),
        args: Vec::new(),
        known: true,
    }
}

/// Creates a prefix [`Node::Command`] from a builtin command record and a list of argument slots.
pub fn prefix_command_node(
    record: &'static BuiltinCommandRecord,
    mut args: Vec<ArgumentSlot>,
) -> Node {
    for (argument, spec) in args.iter_mut().zip(record.argspec.args) {
        if let Some(argument) = argument {
            argument.no_leading_space = spec.no_leading_space;
        }
    }
    Node::Command {
        name: record.name.to_string(),
        args,
        known: true,
    }
}

/// Creates the parser-shaped linebreak command.
pub fn linebreak_command_node() -> Node {
    prefix_command_node(&base::cmd::_BACKSLASH, vec![star_slot(false), None])
}

/// Removes a node from a sibling sequence or replaces it with an empty group matching a single-child slot's content mode.
pub fn remove_node_preserving_slot(ast: &mut Ast, node_id: NodeId) {
    if matches!(ast.slot(node_id), Some(Slot::GroupChild(_))) {
        ast.remove_node(node_id);
        return;
    }

    let mode = match ast.parent(node_id) {
        Some(crate::ast::ParentLink {
            parent,
            slot: Slot::Argument(index),
        }) => match ast.arg_slots(parent)[index].as_ref().map(|arg| &arg.value) {
            Some(ArgumentValue::TextContent(_)) => ContentMode::Text,
            _ => ContentMode::Math,
        },
        _ => ContentMode::Math,
    };
    ast.replace_node_drop_detached_children(
        node_id,
        Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode,
        },
    );
}

/// Returns the base of a scripted node, or the node itself when it carries no scripts.
pub fn script_base(ast: &Ast, node_id: NodeId) -> NodeId {
    match ast.node(node_id) {
        Node::Scripted { base, .. } => *base,
        _ => node_id,
    }
}

/// Reports whether the sibling after `node_id` is `(`, `[`, or an explicit math brace group.
///
/// The sibling counts by its script base. A node that is not a sequence child,
/// such as a scripted base or a single-child slot, has no following sibling
/// and reports `false`.
pub fn followed_by_paren_bracket_or_group(ast: &Ast, node_id: NodeId) -> bool {
    if !matches!(ast.slot(node_id), Some(Slot::GroupChild(_))) {
        return false;
    }
    let Some(next) = ast.next_sibling(node_id) else {
        return false;
    };
    matches!(
        ast.node(script_base(ast, next)),
        Node::Char('(' | '[')
            | Node::Group {
                kind: GroupKind::Explicit,
                mode: ContentMode::Math,
                ..
            }
    )
}

#[derive(Clone, Copy)]
pub enum FenceToken {
    Char(char),
    Control(&'static str),
}

impl FenceToken {
    pub fn node(self) -> Node {
        match self {
            Self::Char(ch) => Node::Char(ch),
            Self::Control(name) => bare_command_node(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use texform_core::parse::ParseConfig;

    fn parse_base(input: &str) -> Ast {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT)
    }

    #[test]
    fn detects_paren_bracket_or_group_after_sequence_child() {
        let ast = parse_base(r"\alpha(x)\beta[x]\gamma{x}\delta^2(x)\epsilon x");
        let children = ast.children(ast.root()).to_vec();
        let command = |name: &str| {
            children
                .iter()
                .copied()
                .find(|child| {
                    matches!(ast.node(script_base(&ast, *child)), Node::Command { name: n, .. } if n == name)
                })
                .unwrap()
        };

        assert!(followed_by_paren_bracket_or_group(&ast, command("alpha")));
        assert!(followed_by_paren_bracket_or_group(&ast, command("beta")));
        assert!(followed_by_paren_bracket_or_group(&ast, command("gamma")));
        assert!(!followed_by_paren_bracket_or_group(
            &ast,
            command("epsilon")
        ));
        // The scripted base is not a sequence child, so it has no following sibling.
        let delta = script_base(&ast, command("delta"));
        assert!(!followed_by_paren_bracket_or_group(&ast, delta));
    }

    #[test]
    fn removes_group_child_without_leaving_an_empty_group() {
        let mut ast = Ast::new();
        let node_id = ast.new_node(bare_command_node("drop"));
        ast.append_child(ast.root(), node_id);

        remove_node_preserving_slot(&mut ast, node_id);

        assert!(!ast.contains(node_id));
        assert!(ast.children(ast.root()).is_empty());
        ast.assert_invariants();
    }

    #[test]
    fn preserves_script_base_slot_with_an_empty_math_group() {
        let mut ast = Ast::new();
        let base = ast.new_node(bare_command_node("drop"));
        let subscript = ast.new_node(Node::Char('i'));
        let scripted = ast.new_node(Node::Scripted {
            base,
            subscript: Some(subscript),
            superscript: None,
        });
        ast.append_child(ast.root(), scripted);

        remove_node_preserving_slot(&mut ast, base);

        assert_eq!(
            ast.node(base),
            &Node::Group {
                children: Vec::new(),
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            }
        );
        assert_eq!(ast.parent_id(base), Some(scripted));
        ast.assert_invariants();
    }
}
