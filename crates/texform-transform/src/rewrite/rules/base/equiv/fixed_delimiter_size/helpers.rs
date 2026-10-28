use crate::ast::{Delimiter, Node, NodeId};
use crate::rewrite::helpers::{bare_command_node, remove_node_preserving_slot};
use crate::rewrite::rule_context::RuleContext;

/// Replaces a sized delimiter with its plain form.
///
/// The size command stopped a preceding command from reading the delimiter as
/// an argument, as in `\\\big[x\big]` or the `physics` `\sin\big[x\big]`.
/// Boundary separation keeps them apart after the transform. Where that
/// command would also read the separating empty group, as the `physics` `\exp`
/// and `\det` do, separation wraps the command instead, so
/// `\ell_0\exp\Big(x\Big)` becomes `\ell _ { 0 } { \exp } ( x )`. The
/// wrapped command is an ordinary atom rather than an operator: after an
/// ordinary or closing atom the thin space before it disappears, and against
/// any other neighbor, including the following opening delimiter, the spacing
/// is the same. A command carrying scripts is never wrapped, so its limits
/// keep their place. This stays within the `Reading` fidelity of these rules.
pub(super) fn drop_fixed_delimiter_size(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    delimiter: Delimiter,
) {
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
