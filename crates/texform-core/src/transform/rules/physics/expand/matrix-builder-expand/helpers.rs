use crate::ast::{Argument, ArgumentKind, ArgumentValue, Delimiter, Node, NodeId};
use crate::transform::helpers::star;
use texform_specs::builtin::physics;

pub(super) fn matrix_quantity_command(body: NodeId, open: char, close: char) -> Node {
    Node::Command {
        name: physics::cmd::MQTY.name.to_string(),
        args: vec![
            star(false),
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
