//! Convenience constructors for building AST argument slots and nodes.
//!
//! Transform rules frequently need to assemble replacement [`Node`] trees.
//! The helpers here eliminate the boilerplate of constructing [`Argument`]
//! wrappers by hand, keeping rule implementations focused on semantics.

use texform_knowledge::builtin::base;
use texform_knowledge::specs::BuiltinCommandRecord;

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, Node, NodeId,
};

/// Creates a mandatory content argument slot wrapping the subtree rooted at `node_id`.
pub fn mandatory_content_slot(node_id: NodeId, mode: ContentMode) -> ArgumentSlot {
    let value = match mode {
        ContentMode::Math => ArgumentValue::MathContent(node_id),
        ContentMode::Text => ArgumentValue::TextContent(node_id),
    };
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value,
    })
}

/// Creates a mandatory delimiter argument slot.
pub fn delimiter_slot(delimiter: Delimiter) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Delimiter(delimiter),
    })
}

/// Creates a mandatory dimension argument slot.
pub fn dimension_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Dimension(value.into()),
    })
}

/// Creates a mandatory integer argument slot.
pub fn integer_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Integer(value.into()),
    })
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
    Some(Argument {
        kind: ArgumentKind::Star,
        value: ArgumentValue::Boolean(value),
    })
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
pub fn prefix_command_node(record: &'static BuiltinCommandRecord, args: Vec<ArgumentSlot>) -> Node {
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

    #[test]
    fn constructs_common_argument_slots() {
        let mut ast = crate::ast::Ast::new();
        let node_id = ast.new_node(Node::Char('x'));

        let mandatory = mandatory_content_slot(node_id, ContentMode::Math);
        assert!(matches!(
            mandatory,
            Some(Argument {
                kind: ArgumentKind::Mandatory,
                value: ArgumentValue::MathContent(id),
            }) if id == node_id
        ));

        let star = star_slot(true);
        assert!(matches!(
            star,
            Some(Argument {
                kind: ArgumentKind::Star,
                value: ArgumentValue::Boolean(true),
            })
        ));

        let linebreak = linebreak_command_node();
        assert!(matches!(
            linebreak,
            Node::Command { name, args, known }
                if name == base::cmd::_BACKSLASH.name
                    && known
                    && matches!(
                        args.as_slice(),
                        [
                            Some(Argument {
                                kind: ArgumentKind::Star,
                                value: ArgumentValue::Boolean(false),
                            }),
                            None,
                        ]
                    )
        ));
    }
}
