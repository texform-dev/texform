use crate::ast::Node;
use crate::rewrite::helpers::linebreak_command_node;

pub(super) fn linebreak_command() -> Node {
    linebreak_command_node()
}
