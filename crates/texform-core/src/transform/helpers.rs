//! Convenience constructors for building AST argument slots and nodes.
//!
//! Transform rules frequently need to assemble replacement [`Node`] trees.
//! The helpers here eliminate the boilerplate of constructing [`Argument`]
//! wrappers by hand, keeping rule implementations focused on semantics.

use texform_specs::specs::BuiltinCommandRecord;

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, GroupKind, Node, NodeId, Slot,
};
use crate::transform::engine::TransformError;
use crate::transform::rule::RuleKey;
use crate::transform::rule_context::RuleContext;

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
        known: true,
    }
}

/// Extracts a boolean star argument from a parsed star slot.
pub fn star_arg_value(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
) -> Result<bool, TransformError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Star => match arg.value {
            ArgumentValue::Boolean(value) => Ok(value),
            _ => Err(cx.invalid_shape(
                rule,
                format!("{subject} star slot should carry a boolean value"),
            )),
        },
        _ => Err(cx.invalid_shape(rule, format!("{subject} should carry a star slot"))),
    }
}

/// Extracts an optional math-content argument.
pub fn optional_math_content(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<Option<NodeId>, TransformError> {
    match slot {
        None => Ok(None),
        Some(arg) if arg.kind == ArgumentKind::Optional => match arg.value {
            ArgumentValue::MathContent(node_id) => Ok(Some(node_id)),
            _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be math content"))),
        },
        _ => Err(cx.invalid_shape(
            rule,
            format!("{subject} {label} should be an optional math argument"),
        )),
    }
}

/// Extracts an optional braced-group math-content argument.
pub fn optional_group_math_content(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<Option<NodeId>, TransformError> {
    match slot {
        None => Ok(None),
        Some(arg) if arg.kind == ArgumentKind::Group => match arg.value {
            ArgumentValue::MathContent(node_id) => Ok(Some(node_id)),
            _ => Err(cx.invalid_shape(
                rule,
                format!("{subject} optional {label} should be math content"),
            )),
        },
        _ => Err(cx.invalid_shape(
            rule,
            format!("{subject} optional {label} should be a braced group"),
        )),
    }
}

/// Extracts a required mandatory math-content argument.
pub fn required_math_content(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<NodeId, TransformError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Mandatory => match arg.value {
            ArgumentValue::MathContent(node_id) => Ok(node_id),
            _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be math content"))),
        },
        _ => Err(cx.invalid_shape(
            rule,
            format!("{subject} {label} should be a mandatory math argument"),
        )),
    }
}

/// Appends cloned math content into `out`, flattening implicit math groups.
///
/// Parser-created content arguments often wrap multiple items in an implicit
/// math group. Flattening that wrapper lets rules compose output such as
/// `\partial f` without introducing extra braces around `f`.
pub fn append_cloned_math_content(cx: &mut RuleContext<'_>, out: &mut Vec<NodeId>, source: NodeId) {
    match cx.ast.node(source) {
        Node::Group {
            children,
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        } => {
            let children = children.clone();
            out.extend(
                children
                    .into_iter()
                    .map(|child| cx.ast.clone_subtree(child)),
            );
        }
        _ => out.push(cx.ast.clone_subtree(source)),
    }
}

/// Creates an implicit math group containing `children`.
pub fn implicit_math_group(cx: &mut RuleContext<'_>, children: Vec<NodeId>) -> NodeId {
    cx.ast.new_node(Node::Group {
        children,
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    })
}

/// Creates a scripted node with only a superscript.
pub fn superscript(cx: &mut RuleContext<'_>, base: NodeId, superscript: NodeId) -> NodeId {
    cx.ast.new_node(Node::Scripted {
        base,
        subscript: None,
        superscript: Some(superscript),
    })
}

/// Replaces `node_id` and removes any old child subtrees detached by the replacement.
pub fn replace_node_discarding_detached_children(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    replacement: Node,
) {
    let old_children: Vec<NodeId> = cx
        .ast
        .edges(node_id)
        .into_iter()
        .map(|(child, _)| child)
        .collect();
    cx.ast.replace_node(node_id, replacement);
    for child in old_children {
        if cx.ast.parent(child).is_none() {
            cx.ast.remove_detached(child);
        }
    }
}

/// Replaces a node with a math-mode sequence.
///
/// If `node_id` is a group child, `before` and `after` are inserted as real
/// siblings around the replacement. In single-child slots, the sequence is
/// wrapped in an implicit math group because those slots cannot hold siblings.
pub fn replace_with_math_sequence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    before: Vec<NodeId>,
    replacement: Node,
    after: Vec<NodeId>,
) {
    match cx.ast.parent(node_id).map(|link| link.slot) {
        Some(Slot::GroupChild(index)) => {
            let parent = cx
                .ast
                .parent_id(node_id)
                .expect("group child should have a parent");
            let before_len = before.len();

            replace_node_discarding_detached_children(cx, node_id, replacement);
            for (offset, child) in before.into_iter().enumerate() {
                cx.ast.insert_child(parent, index + offset, child);
            }
            for (offset, child) in after.into_iter().enumerate() {
                cx.ast
                    .insert_child(parent, index + before_len + 1 + offset, child);
            }
        }
        _ => {
            let old_children: Vec<NodeId> = cx
                .ast
                .edges(node_id)
                .into_iter()
                .map(|(child, _)| child)
                .collect();

            cx.ast.replace_node(node_id, Node::Text(String::new()));
            let replacement = cx.ast.new_node(replacement);
            let mut children = before;
            children.push(replacement);
            children.extend(after);

            replace_node_discarding_detached_children(
                cx,
                node_id,
                Node::Group {
                    children,
                    kind: GroupKind::Implicit,
                    mode: ContentMode::Math,
                },
            );

            for child in old_children {
                if cx.ast.parent(child).is_none() {
                    cx.ast.remove_detached(child);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{GroupKind, Node};
    use crate::parse::ParseContext;
    use crate::transform::engine::TransformReport;
    use crate::transform::rule::{PackageName, RuleKey};
    use crate::transform::rule_context::RuleContext;

    const TEST_RULE: RuleKey = RuleKey {
        package: PackageName::Base,
        name: "helper-test",
    };

    #[test]
    fn extracts_common_prefix_argument_shapes() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut report = TransformReport {
            applied: Vec::new(),
            iterations: 0,
        };
        let mut ast = crate::ast::Ast::new();
        let required = ast.new_node(Node::Char('x'));
        let optional = ast.new_node(Node::Char('2'));
        let grouped = ast.new_node(Node::Char('t'));
        let cx = RuleContext::new(
            &mut ast,
            parse_ctx.math_kb(),
            parse_ctx.text_kb(),
            &mut report,
        );

        let star = star(true);
        let required = mandatory_content(required, ContentMode::Math);
        let optional = Some(Argument {
            kind: ArgumentKind::Optional,
            value: ArgumentValue::MathContent(optional),
        });
        let grouped = Some(Argument {
            kind: ArgumentKind::Group,
            value: ArgumentValue::MathContent(grouped),
        });

        assert_eq!(
            star_arg_value(TEST_RULE, &cx, &star, r"\example").unwrap(),
            true
        );
        assert_eq!(
            required_math_content(TEST_RULE, &cx, &required, r"\example", "argument").unwrap(),
            required
                .as_ref()
                .and_then(|arg| match arg.value {
                    ArgumentValue::MathContent(id) => Some(id),
                    _ => None,
                })
                .unwrap()
        );
        assert_eq!(
            optional_math_content(TEST_RULE, &cx, &optional, r"\example", "order").unwrap(),
            optional.as_ref().and_then(|arg| match arg.value {
                ArgumentValue::MathContent(id) => Some(id),
                _ => None,
            })
        );
        assert_eq!(
            optional_group_math_content(TEST_RULE, &cx, &grouped, r"\example", "denominator")
                .unwrap(),
            grouped.as_ref().and_then(|arg| match arg.value {
                ArgumentValue::MathContent(id) => Some(id),
                _ => None,
            })
        );
        assert_eq!(
            optional_math_content(TEST_RULE, &cx, &None, r"\example", "order").unwrap(),
            None
        );
    }

    #[test]
    fn append_cloned_math_content_flattens_implicit_groups() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut report = TransformReport {
            applied: Vec::new(),
            iterations: 0,
        };
        let mut ast = crate::ast::Ast::new();
        let x = ast.new_node(Node::Char('x'));
        let y = ast.new_node(Node::Char('y'));
        let source = ast.new_node(Node::Group {
            children: vec![x, y],
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        let mut cx = RuleContext::new(
            &mut ast,
            parse_ctx.math_kb(),
            parse_ctx.text_kb(),
            &mut report,
        );
        let mut out = Vec::new();

        append_cloned_math_content(&mut cx, &mut out, source);

        assert_eq!(out.len(), 2);
        assert_ne!(out[0], x);
        assert_ne!(out[1], y);
        assert_eq!(cx.ast.node(out[0]), &Node::Char('x'));
        assert_eq!(cx.ast.node(out[1]), &Node::Char('y'));
        assert_eq!(cx.ast.parent(out[0]), None);
        assert_eq!(cx.ast.parent(out[1]), None);
        cx.ast.assert_invariants();
    }

    #[test]
    fn constructs_common_math_nodes() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut report = TransformReport {
            applied: Vec::new(),
            iterations: 0,
        };
        let mut ast = crate::ast::Ast::new();
        let x = ast.new_node(Node::Char('x'));
        let y = ast.new_node(Node::Char('y'));
        let base = ast.new_node(Node::Char('a'));
        let power = ast.new_node(Node::Char('2'));
        let mut cx = RuleContext::new(
            &mut ast,
            parse_ctx.math_kb(),
            parse_ctx.text_kb(),
            &mut report,
        );

        let group = implicit_math_group(&mut cx, vec![x, y]);
        let scripted = superscript(&mut cx, base, power);

        assert_eq!(
            cx.ast.node(group),
            &Node::Group {
                children: vec![x, y],
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            }
        );
        assert_eq!(cx.ast.parent_id(x), Some(group));
        assert_eq!(cx.ast.parent_id(y), Some(group));
        assert_eq!(
            cx.ast.node(scripted),
            &Node::Scripted {
                base,
                subscript: None,
                superscript: Some(power),
            }
        );
        assert_eq!(cx.ast.parent_id(base), Some(scripted));
        assert_eq!(cx.ast.parent_id(power), Some(scripted));
        cx.ast.assert_invariants();
    }

    #[test]
    fn replace_node_discarding_detached_children_removes_old_subtree() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut report = TransformReport {
            applied: Vec::new(),
            iterations: 0,
        };
        let mut ast = crate::ast::Ast::new();
        let old_child = ast.new_node(Node::Char('x'));
        let old_grandchild = ast.new_node(Node::Char('y'));
        let old_child = ast.new_node(Node::Group {
            children: vec![old_child, old_grandchild],
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        let target = ast.new_node(Node::Group {
            children: vec![old_child],
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
        });
        ast.append_child(ast.root(), target);
        let new_child = ast.new_node(Node::Char('z'));
        let mut cx = RuleContext::new(
            &mut ast,
            parse_ctx.math_kb(),
            parse_ctx.text_kb(),
            &mut report,
        );

        replace_node_discarding_detached_children(
            &mut cx,
            target,
            Node::Group {
                children: vec![new_child],
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            },
        );

        assert!(!cx.ast.contains(old_child));
        assert!(!cx.ast.contains(old_grandchild));
        assert_eq!(cx.ast.parent_id(new_child), Some(target));
        assert_eq!(cx.ast.children(target), &[new_child]);
        cx.ast.assert_invariants();
    }
}
