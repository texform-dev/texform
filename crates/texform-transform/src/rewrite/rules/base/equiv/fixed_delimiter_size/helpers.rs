use crate::ast::{Delimiter, Node, NodeId, Slot};
use crate::rewrite::helpers::{bare_command_node, remove_node_preserving_slot};
use crate::rewrite::rule_context::RuleContext;

/// Replaces a sized delimiter with its plain form.
///
/// The size command stopped a preceding command from reading the delimiter as
/// an argument, as in `\\\big[x\big]` or the `physics` `\sin\big[x\big]`.
/// When the plain delimiter would be read that way, a group keeps them apart.
pub(super) fn drop_fixed_delimiter_size(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    delimiter: Delimiter,
) {
    // The sequence position stays occupied by this node or its scripted
    // parent, or by the next sibling when a null delimiter is removed.
    let anchor = match cx.ast.slot(node_id) {
        Some(Slot::ScriptBase) => cx.ast.parent_id(node_id),
        _ => Some(node_id),
    }
    .and_then(|anchor| Some((cx.ast.parent_id(anchor)?, cx.ast.slot(anchor)?)));

    match delimiter_node(delimiter) {
        Some(replacement) => {
            let replacement = cx.ast.new_node(replacement);
            cx.ast.replace_with_math_sequence_preserving_scripts(
                node_id,
                Vec::new(),
                replacement,
                Vec::new(),
            );
        }
        None => remove_node_preserving_slot(cx.ast, node_id),
    }

    let Some((parent, Slot::GroupChild(index))) = anchor else {
        return;
    };
    let Some(&next) = cx.ast.children(parent).get(index) else {
        return;
    };
    cx.separate_from_preceding_command(next);
}

fn delimiter_node(delimiter: Delimiter) -> Option<Node> {
    match delimiter {
        Delimiter::None => None,
        Delimiter::Char('<') => Some(bare_command_node("langle")),
        Delimiter::Char('>') => Some(bare_command_node("rangle")),
        Delimiter::Char(ch) => Some(Node::Char(ch)),
        // The parser reads `\{` and `\}` as escaped characters, not commands.
        Delimiter::Control(name) if name == "{" => Some(Node::Char('{')),
        Delimiter::Control(name) if name == "}" => Some(Node::Char('}')),
        Delimiter::Control(name) if name == "lt" => Some(bare_command_node("langle")),
        Delimiter::Control(name) if name == "gt" => Some(bare_command_node("rangle")),
        Delimiter::Control(name) => Some(bare_command_node(&name)),
    }
}
