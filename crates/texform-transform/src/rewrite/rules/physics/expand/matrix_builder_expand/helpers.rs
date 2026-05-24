use crate::ast::{Argument, ArgumentKind, ArgumentValue, Delimiter, Node, NodeId};
use crate::rewrite::helpers::star_slot;
use texform_knowledge::builtin::physics;

pub(super) fn matrix_quantity_command(body: NodeId, open: char, close: char) -> Node {
    Node::Command {
        name: physics::cmd::MQTY.name.to_string(),
        args: vec![
            star_slot(false),
            Some(Argument {
                kind: ArgumentKind::Paired {
                    open: Delimiter::Char(open),
                    close: Delimiter::Char(close),
                },
                value: ArgumentValue::MathContent(body),
            }),
        ],
        known: true,
    }
}
