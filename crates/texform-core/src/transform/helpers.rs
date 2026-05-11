//! Convenience constructors for building AST argument slots and nodes.
//!
//! Transform rules frequently need to assemble replacement [`Node`] trees.
//! The helpers here eliminate the boilerplate of constructing [`Argument`]
//! wrappers by hand, keeping rule implementations focused on semantics.

use texform_specs::builtin::base;
use texform_specs::specs::BuiltinCommandRecord;

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, Node, NodeId,
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

/// Creates a mandatory delimiter argument slot.
pub fn delimiter_slot(delimiter: Delimiter) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Delimiter(delimiter),
    })
}

/// Creates a mandatory dimension argument slot.
pub fn dimension_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Dimension(value.into()),
    })
}

/// Creates a mandatory integer argument slot.
pub fn integer_slot(value: impl Into<String>) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Integer(value.into()),
    })
}

/// Creates the two mandatory content arguments used when converting an infix node to a prefix command.
pub fn infix_prefix_args(left: NodeId, right: NodeId, mode: ContentMode) -> Vec<ArgumentSlot> {
    vec![
        mandatory_content(left, mode),
        mandatory_content(right, mode),
    ]
}

/// Creates a star (boolean) argument slot, representing a `*` modifier on a command.
pub fn star(value: bool) -> ArgumentSlot {
    Some(Argument {
        kind: ArgumentKind::Star,
        value: ArgumentValue::Boolean(value),
    })
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
pub fn prefix_command_node(record: &'static BuiltinCommandRecord, args: Vec<ArgumentSlot>) -> Node {
    Node::Command {
        name: record.name.to_string(),
        args,
        known: true,
    }
}

/// Creates the parser-shaped linebreak command.
pub fn linebreak_command_node() -> Node {
    prefix_command_node(&base::cmd::_BACKSLASH, vec![star(false), None])
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

/// Extracts a required math-content argument that may be either mandatory or a braced group.
pub fn required_math_content_any(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<NodeId, TransformError> {
    match slot {
        Some(arg) if matches!(arg.kind, ArgumentKind::Mandatory | ArgumentKind::Group) => match arg
            .value
        {
            ArgumentValue::MathContent(node_id) => Ok(node_id),
            _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be math content"))),
        },
        _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be math content"))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Node;
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
            required_math_content_any(TEST_RULE, &cx, &grouped, r"\example", "argument").unwrap(),
            grouped
                .as_ref()
                .and_then(|arg| match arg.value {
                    ArgumentValue::MathContent(id) => Some(id),
                    _ => None,
                })
                .unwrap()
        );
        assert_eq!(
            optional_math_content(TEST_RULE, &cx, &None, r"\example", "order").unwrap(),
            None
        );
    }
}
