//! Convenience constructors for building AST argument slots and nodes.
//!
//! Transform rules frequently need to assemble replacement [`Node`] trees.
//! The helpers here eliminate the boilerplate of constructing [`Argument`]
//! wrappers by hand, keeping rule implementations focused on semantics.

use texform_knowledge::builtin::base;
use texform_knowledge::specs::{ActiveCommandRecord, BuiltinCharacterRecord, BuiltinCommandRecord};

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind,
    Node, NodeId, Slot,
};
use crate::rewrite::rule::RuleEffect;
use crate::rewrite::rule_context::RuleContext;

/// Creates a mandatory content argument slot wrapping the subtree rooted at `node_id`.
pub fn mandatory_content_slot(node_id: NodeId, mode: ContentMode) -> ArgumentSlot {
    let value = match mode {
        ContentMode::Math => ArgumentValue::MathContent(node_id),
        ContentMode::Text => ArgumentValue::TextContent(node_id),
    };
    Some(Argument::from_value(ArgumentKind::Mandatory, value))
}

/// Creates a mandatory operator-name content argument slot.
pub fn mandatory_operator_name_slot(node_id: NodeId) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::OperatorNameContent(node_id),
    ))
}

/// Creates a mandatory delimiter argument slot.
pub fn delimiter_slot(delimiter: Delimiter) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::Delimiter(delimiter),
    ))
}

/// Creates a mandatory dimension argument slot.
pub fn dimension_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::Dimension(value.into()),
    ))
}

/// Creates a mandatory integer argument slot.
pub fn integer_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::Integer(value.into()),
    ))
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
    Some(Argument::from_value(
        ArgumentKind::Star,
        ArgumentValue::Boolean(value),
    ))
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
pub fn prefix_command_node(
    record: &'static BuiltinCommandRecord,
    mut args: Vec<ArgumentSlot>,
) -> Node {
    for (argument, spec) in args.iter_mut().zip(record.argspec.args) {
        if let Some(argument) = argument {
            argument.no_leading_space = spec.no_leading_space;
        }
    }
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

/// Replaces an alias with a linebreak, leaving scripts on an empty base.
pub fn replace_with_linebreak(ast: &mut Ast, node_id: NodeId) {
    if ast.slot(node_id) == Some(Slot::ScriptBase) {
        let linebreak = ast.new_node(linebreak_command_node());
        let empty = ast.new_node(Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        ast.replace_with_math_sequence_preserving_scripts(node_id, vec![linebreak], empty, vec![]);
    } else {
        ast.replace_node(node_id, linebreak_command_node());
    }
}

/// Removes a node from a sibling sequence or replaces it with an empty group matching a single-child slot's content mode.
pub fn remove_node_preserving_slot(ast: &mut Ast, node_id: NodeId) {
    if matches!(ast.slot(node_id), Some(Slot::GroupChild(_))) {
        ast.remove_node(node_id);
        return;
    }

    let mode = match ast.parent(node_id) {
        Some(crate::ast::ParentLink {
            parent,
            slot: Slot::Argument(index),
        }) => match ast.arg_slots(parent)[index].as_ref().map(|arg| &arg.value) {
            Some(ArgumentValue::TextContent(_)) => ContentMode::Text,
            _ => ContentMode::Math,
        },
        _ => ContentMode::Math,
    };
    ast.replace_node_drop_detached_children(
        node_id,
        Node::Group {
            children: Vec::new(),
            kind: GroupKind::Implicit,
            mode,
        },
    );
}

/// Returns the content mode `node_id` is read in, as determined by the slot it occupies.
///
/// Sequence children take the mode of their root or group, content arguments
/// take the mode of their argument value (operator-name content is math), and
/// script and infix operands are math. Environment bodies, detached nodes, and
/// the root itself report `None`.
pub fn content_mode_at(ast: &Ast, node_id: NodeId) -> Option<ContentMode> {
    let parent = ast.parent(node_id)?;
    match parent.slot {
        Slot::GroupChild(_) => ast.node(parent.parent).content_mode(),
        Slot::Argument(index) => ast
            .arg_slots(parent.parent)
            .get(index)?
            .as_ref()?
            .value
            .content()
            .map(|(_, mode)| mode),
        Slot::ScriptBase
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => Some(ContentMode::Math),
        Slot::EnvBody => None,
    }
}

/// Returns the active record of a prefix command node, or `None` for any other node kind.
pub fn prefix_command_record<'a>(
    cx: &'a RuleContext<'_>,
    node_id: NodeId,
) -> Option<&'a ActiveCommandRecord> {
    match cx.ast.node(node_id) {
        Node::Command { .. } => cx.active_command(node_id),
        _ => None,
    }
}

/// Reports whether `active` is the builtin command `record` contributed by `package`.
///
/// Kind, allowed mode, and argspec must equal the builtin record. With
/// `exclusive`, `package` must be the only contributing package, so a
/// same-signature redefinition by another package fails closed; otherwise it
/// only has to be among the contributors.
pub fn active_command_is_builtin(
    active: &ActiveCommandRecord,
    record: &'static BuiltinCommandRecord,
    package: &str,
    exclusive: bool,
) -> bool {
    active.name == record.name
        && active.kind == record.kind
        && active.allowed_mode == record.allowed_mode
        && active.argspec.source == record.argspec.source
        && if exclusive {
            active.from_packages == [package]
        } else {
            active.from_packages.contains(&package)
        }
}

/// Reports whether `active` is the character-backed command for the builtin
/// character `record`, contributed by `package` alone and owned by `package`
/// in the active math character table.
pub fn active_command_is_builtin_character(
    cx: &RuleContext<'_>,
    active: &ActiveCommandRecord,
    record: &'static BuiltinCharacterRecord,
    package: &str,
) -> bool {
    active.name == record.name
        && active.from_packages == [package]
        && cx
            .lookup_character(record.name, ContentMode::Math)
            .is_some_and(|character| character.package == package)
}

/// Drops a postfix modifier command that acts on the sequence node directly before it.
///
/// `node_id` must be the argument-less builtin command `modifier` contributed
/// by `package` alone. It applies when the modifier is a sequence child, or
/// the base of a scripted sequence child, whose previous sibling `is_eligible`
/// accepts. Scripts attached to the modifier move onto that predecessor. Any
/// other shape, including a modifier in a single-child slot, returns
/// [`RuleEffect::Skipped`] without changing the tree.
pub fn drop_postfix_modifier(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    modifier: &'static BuiltinCommandRecord,
    package: &str,
    is_eligible: impl FnOnce(&RuleContext<'_>, NodeId) -> bool,
) -> RuleEffect {
    if !prefix_command_record(cx, node_id)
        .is_some_and(|active| active_command_is_builtin(active, modifier, package, true))
        || !cx.ast.arg_slots(node_id).is_empty()
    {
        return RuleEffect::Skipped;
    }

    let (modifier_root, scripts) = match cx.ast.slot(node_id) {
        Some(Slot::GroupChild(_)) => (node_id, None),
        Some(Slot::ScriptBase) => {
            let Some(scripted) = cx.ast.parent_id(node_id) else {
                return RuleEffect::Skipped;
            };
            let Node::Scripted {
                subscript,
                superscript,
                ..
            } = *cx.ast.node(scripted)
            else {
                return RuleEffect::Skipped;
            };
            if !matches!(cx.ast.slot(scripted), Some(Slot::GroupChild(_))) {
                return RuleEffect::Skipped;
            }
            (scripted, Some((subscript, superscript)))
        }
        _ => return RuleEffect::Skipped,
    };
    let Some(predecessor) = cx.ast.prev_sibling(modifier_root) else {
        return RuleEffect::Skipped;
    };
    if !is_eligible(cx, predecessor) {
        return RuleEffect::Skipped;
    }

    if let Some((subscript, superscript)) = scripts {
        let base = cx.ast.clone_subtree(predecessor);
        let subscript = subscript.map(|id| cx.ast.clone_subtree(id));
        let superscript = superscript.map(|id| cx.ast.clone_subtree(id));
        cx.ast.replace_node_drop_detached_children(
            predecessor,
            Node::Scripted {
                base,
                subscript,
                superscript,
            },
        );
    }
    cx.ast.remove_node(modifier_root);
    RuleEffect::Applied
}

/// Returns the base of a scripted node, or the node itself when it carries no scripts.
pub fn script_base(ast: &Ast, node_id: NodeId) -> NodeId {
    match ast.node(node_id) {
        Node::Scripted { base, .. } => *base,
        _ => node_id,
    }
}

/// Reports whether the sibling after `node_id` is `(`, `[`, or an explicit math brace group.
///
/// The sibling counts by its script base. A node that is not a sequence child,
/// such as a scripted base or a single-child slot, has no following sibling
/// and reports `false`.
pub fn followed_by_paren_bracket_or_group(ast: &Ast, node_id: NodeId) -> bool {
    if !matches!(ast.slot(node_id), Some(Slot::GroupChild(_))) {
        return false;
    }
    let Some(next) = ast.next_sibling(node_id) else {
        return false;
    };
    matches!(
        ast.node(script_base(ast, next)),
        Node::Char('(' | '[')
            | Node::Group {
                kind: GroupKind::Explicit,
                mode: ContentMode::Math,
                ..
            }
    )
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
    use crate::parse::ParseContext;
    use texform_core::parse::ParseConfig;

    fn parse_base(input: &str) -> Ast {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT)
    }

    #[test]
    fn detects_paren_bracket_or_group_after_sequence_child() {
        let ast = parse_base(r"\alpha(x)\beta[x]\gamma{x}\delta^2(x)\epsilon x");
        let children = ast.children(ast.root()).to_vec();
        let command = |name: &str| {
            children
                .iter()
                .copied()
                .find(|child| {
                    matches!(ast.node(script_base(&ast, *child)), Node::Command { name: n, .. } if n == name)
                })
                .unwrap()
        };

        assert!(followed_by_paren_bracket_or_group(&ast, command("alpha")));
        assert!(followed_by_paren_bracket_or_group(&ast, command("beta")));
        assert!(followed_by_paren_bracket_or_group(&ast, command("gamma")));
        assert!(!followed_by_paren_bracket_or_group(
            &ast,
            command("epsilon")
        ));
        // The scripted base is not a sequence child, so it has no following sibling.
        let delta = script_base(&ast, command("delta"));
        assert!(!followed_by_paren_bracket_or_group(&ast, delta));
    }

    #[test]
    fn reports_content_mode_from_the_occupied_slot() {
        let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
        let ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"a^b \text{c$d$\ensuremath{e}} \operatorname{f} \frac{g}h {i \over j} \begin{matrix}k\end{matrix}",
            &ParseConfig::STRICT,
        );
        let find = |predicate: &dyn Fn(&Node) -> bool| {
            ast.find(ast.root(), predicate)
                .expect("node should exist in the parsed formula")
        };
        let char_node = |ch: char| find(&|node: &Node| node == &Node::Char(ch));

        // Sequence children follow their container, including inline math in text.
        assert_eq!(
            content_mode_at(&ast, char_node('d')),
            Some(ContentMode::Math)
        );
        let text_chunk = find(&|node: &Node| matches!(node, Node::Text(text) if text == "c"));
        assert_eq!(content_mode_at(&ast, text_chunk), Some(ContentMode::Text));
        let scripted = find(&|node: &Node| matches!(node, Node::Scripted { .. }));
        assert_eq!(content_mode_at(&ast, scripted), Some(ContentMode::Math));

        // Script and infix operands are math.
        assert_eq!(
            content_mode_at(&ast, char_node('a')),
            Some(ContentMode::Math)
        );
        assert_eq!(
            content_mode_at(&ast, char_node('b')),
            Some(ContentMode::Math)
        );
        let infix = find(&|node: &Node| matches!(node, Node::Infix { .. }));
        let Node::Infix { left, right, .. } = ast.node(infix) else {
            unreachable!()
        };
        assert_eq!(content_mode_at(&ast, *left), Some(ContentMode::Math));
        assert_eq!(content_mode_at(&ast, *right), Some(ContentMode::Math));

        // Content arguments follow the argument value, operator names read as math.
        assert_eq!(
            content_mode_at(&ast, char_node('e')),
            Some(ContentMode::Math)
        );
        assert_eq!(
            content_mode_at(&ast, char_node('f')),
            Some(ContentMode::Math)
        );
        assert_eq!(
            content_mode_at(&ast, char_node('h')),
            Some(ContentMode::Math)
        );
        let text_command =
            find(&|node: &Node| matches!(node, Node::Command { name, .. } if name == "text"));
        let ArgumentValue::TextContent(text_body) =
            &ast.arg_slots(text_command)[0].as_ref().unwrap().value
        else {
            panic!("text argument should carry text content")
        };
        assert_eq!(content_mode_at(&ast, *text_body), Some(ContentMode::Text));

        // Environment bodies, the root, and detached nodes have no reading mode.
        let environment = find(&|node: &Node| matches!(node, Node::Environment { .. }));
        let Node::Environment { body, .. } = ast.node(environment) else {
            unreachable!()
        };
        assert_eq!(content_mode_at(&ast, *body), None);
        assert_eq!(content_mode_at(&ast, ast.root()), None);
        let mut ast = ast;
        let detached = ast.new_node(Node::Char('z'));
        assert_eq!(content_mode_at(&ast, detached), None);
    }

    fn find_command(ast: &Ast, name: &str) -> NodeId {
        ast.find(
            ast.root(),
            |node| matches!(node, Node::Command { name: n, .. } if n == name),
        )
        .expect("command should exist in the parsed formula")
    }

    #[test]
    fn matches_builtin_command_and_character_records_by_owner() {
        let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"\lim \det \sum \iiiint \injlim x",
            &ParseConfig::STRICT,
        );
        let ids: Vec<_> = ["lim", "det", "sum", "iiiint", "injlim"]
            .map(|name| find_command(&ast, name))
            .to_vec();
        let x = ast
            .find(ast.root(), |node| node == &Node::Char('x'))
            .unwrap();
        let cx = RuleContext::new(&mut ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
        let record = |id| prefix_command_record(&cx, id).expect("known command");

        assert!(prefix_command_record(&cx, x).is_none());
        assert!(active_command_is_builtin(
            record(ids[0]),
            &base::cmd::LIM,
            "base",
            false
        ));
        assert!(!active_command_is_builtin(
            record(ids[0]),
            &base::cmd::LIMSUP,
            "base",
            false
        ));
        // Physics redefines `\det`, so only a shared-owner check accepts it.
        assert!(!active_command_is_builtin(
            record(ids[1]),
            &base::cmd::DET,
            "base",
            true
        ));
        assert!(active_command_is_builtin(
            record(ids[4]),
            &texform_knowledge::builtin::ams::cmd::INJLIM,
            "ams",
            false
        ));
        assert!(!active_command_is_builtin(
            record(ids[4]),
            &texform_knowledge::builtin::ams::cmd::INJLIM,
            "base",
            false
        ));

        assert!(active_command_is_builtin_character(
            &cx,
            record(ids[2]),
            &base::chars::SUM,
            "base"
        ));
        assert!(!active_command_is_builtin_character(
            &cx,
            record(ids[2]),
            &base::chars::PROD,
            "base"
        ));
        assert!(active_command_is_builtin_character(
            &cx,
            record(ids[3]),
            &texform_knowledge::builtin::ams::chars::IIIINT,
            "ams"
        ));
        assert!(!active_command_is_builtin_character(
            &cx,
            record(ids[3]),
            &texform_knowledge::builtin::ams::chars::IIIINT,
            "base"
        ));
    }

    fn drop_limits(input: &str, accept: bool) -> (RuleEffect, String) {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = parse_base(input);
        let modifier = find_command(&ast, "limits");
        let mut cx = RuleContext::new(&mut ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
        let effect =
            drop_postfix_modifier(&mut cx, modifier, &base::cmd::LIMITS, "base", |_, _| accept);
        ast.assert_invariants();
        (effect, crate::serialize::serialize(&ast))
    }

    fn serialized(input: &str) -> String {
        crate::serialize::serialize(&parse_base(input))
    }

    #[test]
    fn drops_postfix_modifier_and_moves_its_scripts_to_the_predecessor() {
        for (input, expected) in [
            (r"\sum\limits x", r"\sum x"),
            (r"\sum\limits_i^n x", r"\sum_i^n x"),
            (r"a\sum\limits^n", r"a\sum^n"),
            (r"{\sum\limits_i}", r"{\sum_i}"),
        ] {
            assert_eq!(
                drop_limits(input, true),
                (RuleEffect::Applied, serialized(expected)),
                "{input}"
            );
        }
    }

    #[test]
    fn keeps_postfix_modifier_outside_the_supported_shapes() {
        for input in [
            // No predecessor in the same sequence.
            r"\limits_i",
            r"{\sum}{\limits_i}",
            // A single-child slot has no sequence predecessor.
            r"x^\limits",
            r"\hat\limits",
        ] {
            assert_eq!(
                drop_limits(input, true),
                (RuleEffect::Skipped, serialized(input)),
                "{input}"
            );
        }
        // The predecessor predicate decides eligibility.
        assert_eq!(
            drop_limits(r"\sum\limits_i", false),
            (RuleEffect::Skipped, serialized(r"\sum\limits_i"))
        );
    }

    #[test]
    fn keeps_postfix_modifier_with_another_identity_or_arguments() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = parse_base(r"\sum\nolimits");
        let nolimits = find_command(&ast, "nolimits");
        let mut cx = RuleContext::new(&mut ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
        assert_eq!(
            drop_postfix_modifier(&mut cx, nolimits, &base::cmd::LIMITS, "base", |_, _| true),
            RuleEffect::Skipped
        );
        // The modifier must be owned by `package` alone.
        assert_eq!(
            drop_postfix_modifier(&mut cx, nolimits, &base::cmd::NOLIMITS, "ams", |_, _| true),
            RuleEffect::Skipped
        );

        // An argument-taking shape is not the argument-less modifier.
        let mut ast = parse_base(r"\sum");
        let content = ast.new_node(Node::Char('x'));
        let modifier = ast.new_node(Node::Command {
            name: "limits".to_string(),
            args: vec![mandatory_content_slot(content, ContentMode::Math)],
            known: true,
        });
        ast.append_child(ast.root(), modifier);
        let mut cx = RuleContext::new(&mut ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
        assert_eq!(
            drop_postfix_modifier(&mut cx, modifier, &base::cmd::LIMITS, "base", |_, _| true),
            RuleEffect::Skipped
        );
        assert_eq!(ast.children(ast.root()).len(), 2);
        ast.assert_invariants();
    }

    #[test]
    fn removes_group_child_without_leaving_an_empty_group() {
        let mut ast = Ast::new();
        let node_id = ast.new_node(bare_command_node("drop"));
        ast.append_child(ast.root(), node_id);

        remove_node_preserving_slot(&mut ast, node_id);

        assert!(!ast.contains(node_id));
        assert!(ast.children(ast.root()).is_empty());
        ast.assert_invariants();
    }

    #[test]
    fn preserves_script_base_slot_with_an_empty_math_group() {
        let mut ast = Ast::new();
        let base = ast.new_node(bare_command_node("drop"));
        let subscript = ast.new_node(Node::Char('i'));
        let scripted = ast.new_node(Node::Scripted {
            base,
            subscript: Some(subscript),
            superscript: None,
        });
        ast.append_child(ast.root(), scripted);

        remove_node_preserving_slot(&mut ast, base);

        assert_eq!(
            ast.node(base),
            &Node::Group {
                children: Vec::new(),
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            }
        );
        assert_eq!(ast.parent_id(base), Some(scripted));
        ast.assert_invariants();
    }
}
