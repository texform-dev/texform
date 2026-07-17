use crate::ast::{Delimiter, Node, NodeId};
use crate::rewrite::helpers::{bare_command_node, remove_node_preserving_slot};
use crate::rewrite::rule_context::RuleContext;

pub(super) fn drop_fixed_delimiter_size(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    delimiter: Delimiter,
) {
    let Some(replacement) = delimiter_node(delimiter) else {
        remove_node_preserving_slot(cx.ast, node_id);
        return;
    };

    let replacement = cx.ast.new_node(replacement);
    cx.ast.replace_with_math_sequence_preserving_scripts(
        node_id,
        Vec::new(),
        replacement,
        Vec::new(),
    );
}

fn delimiter_node(delimiter: Delimiter) -> Option<Node> {
    match delimiter {
        Delimiter::None => None,
        Delimiter::Char('<') => Some(bare_command_node("langle")),
        Delimiter::Char('>') => Some(bare_command_node("rangle")),
        Delimiter::Char(ch) => Some(Node::Char(ch)),
        Delimiter::Control(name) if name == "lt" => Some(bare_command_node("langle")),
        Delimiter::Control(name) if name == "gt" => Some(bare_command_node("rangle")),
        Delimiter::Control(name) => Some(bare_command_node(&name)),
    }
}
