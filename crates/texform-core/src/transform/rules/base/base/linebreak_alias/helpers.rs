use crate::ast::{ArgumentSlot, Node};
use crate::transform::helpers::star;

pub(super) fn linebreak_args() -> Vec<ArgumentSlot> {
    vec![star(false), None]
}

pub(super) fn linebreak_command() -> Node {
    Node::Command {
        name: "\\".to_string(),
        args: linebreak_args(),
        known: true,
    }
}
