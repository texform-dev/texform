use texform_knowledge::specs::BuiltinCharacterRecord;

use crate::ast::{ContentMode, Node, NodeId};
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule_context::RuleContext;

pub(super) fn following_character_atom(
    cx: &RuleContext<'_>,
    node_id: NodeId,
    literal: char,
    command_records: &[&'static BuiltinCharacterRecord],
) -> Option<NodeId> {
    let next = cx.ast.next_sibling(node_id)?;
    match cx.ast.node(next) {
        Node::Char(ch) if *ch == literal => Some(next),
        Node::Command {
            name,
            args,
            known: true,
        } if args.is_empty()
            && command_records.iter().any(|record| record.name == name)
            && cx.lookup_character(name, ContentMode::Math).is_some() =>
        {
            Some(next)
        }
        _ => None,
    }
}

pub(super) fn replace_not_pair(
    cx: &mut RuleContext<'_>,
    not_id: NodeId,
    atom_id: NodeId,
    target: &'static BuiltinCharacterRecord,
) {
    cx.ast
        .replace_node(not_id, bare_command_node(target.name));
    cx.ast.remove_node(atom_id);
}
