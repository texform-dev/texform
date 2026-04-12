//! Convenience constructors for building AST argument slots and nodes.
//!
//! Transform rules frequently need to assemble replacement [`Node`] trees.
//! The helpers here eliminate the boilerplate of constructing [`Argument`]
//! wrappers by hand, keeping rule implementations focused on semantics.

use texform_specs::specs::BuiltinCommandRecord;

use crate::ast::{Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Node, NodeId};

/// Creates a mandatory content argument slot wrapping the subtree rooted at `node_id`.
pub fn mandatory_content(node_id: NodeId, mode: ContentMode) -> ArgumentSlot {
    let value = match mode {
        ContentMode::Math => ArgumentValue::MathContent(node_id),
        ContentMode::Text => ArgumentValue::TextContent(node_id),
    };
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value,
    })
}

/// Creates an optional content argument slot wrapping the subtree rooted at `node_id`.
pub fn optional_content(node_id: NodeId, mode: ContentMode) -> ArgumentSlot {
    let value = match mode {
        ContentMode::Math => ArgumentValue::MathContent(node_id),
        ContentMode::Text => ArgumentValue::TextContent(node_id),
    };
    Some(Argument {
        kind: ArgumentKind::Optional,
        value,
    })
}

/// Creates a star (boolean) argument slot, representing a `*` modifier on a command.
pub fn star(value: bool) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Star,
        value: ArgumentValue::Boolean(value),
    })
}

/// Creates a prefix [`Node::Command`] from a builtin command record and a list of argument slots.
pub fn prefix_command(record: &'static BuiltinCommandRecord, args: Vec<ArgumentSlot>) -> Node {
    Node::Command {
        name: record.name.to_string(),
        args,
    }
}
