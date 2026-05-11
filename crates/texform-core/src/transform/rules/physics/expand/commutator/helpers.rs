use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind, Node, NodeId,
    Slot,
};
use crate::transform::engine::TransformError;
use crate::transform::rule::RuleKey;
use crate::transform::rule_context::RuleContext;

#[derive(Clone, Copy)]
pub(super) enum FixedFenceToken {
    Char(char),
    Control(&'static str),
}

pub(super) struct BinaryFencePair {
    pub(super) auto_left: Delimiter,
    pub(super) auto_right: Delimiter,
    pub(super) fixed_left: FixedFenceToken,
    pub(super) fixed_right: FixedFenceToken,
}

impl FixedFenceToken {
    fn node(self) -> Node {
        match self {
            Self::Char(ch) => Node::Char(ch),
            Self::Control(name) => Node::Command {
                name: name.to_string(),
                args: Vec::new(),
                known: true,
            },
        }
    }
}

pub(super) fn required_braced_math_arg(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<NodeId, TransformError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Group => match arg.value {
            ArgumentValue::MathContent(node_id) => Ok(node_id),
            _ => Err(cx.invalid_shape(rule, format!("{subject} {label} should be math content"))),
        },
        _ => Err(cx.invalid_shape(
            rule,
            format!("{subject} {label} should be a required braced math group"),
        )),
    }
}

pub(super) fn required_math_arg(
    rule: RuleKey,
    cx: &RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
    label: &str,
) -> Result<NodeId, TransformError> {
    match slot {
        Some(arg)
            if matches!(arg.kind, ArgumentKind::Mandatory | ArgumentKind::Group)
                && matches!(arg.value, ArgumentValue::MathContent(_)) =>
        {
            match arg.value {
                ArgumentValue::MathContent(node_id) => Ok(node_id),
                _ => unreachable!("math content was checked above"),
            }
        }
        _ => Err(cx.invalid_shape(
            rule,
            format!("{subject} {label} should be math content"),
        )),
    }
}

pub(super) fn replace_with_binary_bracket_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    starred: bool,
    left: NodeId,
    right: NodeId,
    fences: BinaryFencePair,
) {
    if starred {
        replace_with_fixed_fence(
            cx,
            node_id,
            left,
            right,
            fences.fixed_left,
            fences.fixed_right,
        );
    } else {
        replace_with_auto_fence(cx, node_id, left, right, fences.auto_left, fences.auto_right);
    }
}

fn replace_with_auto_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    left: NodeId,
    right: NodeId,
    open: Delimiter,
    close: Delimiter,
) {
    let mut children = Vec::new();
    append_binary_bracket_body(cx, &mut children, left, right);

    cx.ast.replace_node_drop_detached_children(node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited {
                left: open,
                right: close,
            },
            mode: ContentMode::Math,
        },
    );
}

fn replace_with_fixed_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    left: NodeId,
    right: NodeId,
    open: FixedFenceToken,
    close: FixedFenceToken,
) {
    if matches!(cx.ast.slot(node_id), Some(Slot::ScriptBase)) {
        replace_scripted_base_with_fixed_fence(cx, node_id, left, right, open, close);
        return;
    }

    let mut rest = Vec::new();
    append_binary_bracket_body(cx, &mut rest, left, right);
    rest.push(cx.ast.new_node(close.node()));

    let open = cx.ast.new_node(open.node());
    cx.ast
        .replace_with_math_sequence(node_id, Vec::new(), open, rest);
}

fn replace_scripted_base_with_fixed_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    left: NodeId,
    right: NodeId,
    open: FixedFenceToken,
    close: FixedFenceToken,
) {
    let Some(parent) = cx.ast.parent_id(node_id) else {
        return;
    };

    let (subscript, superscript) = match cx.ast.node(parent) {
        Node::Scripted {
            subscript,
            superscript,
            ..
        } => (*subscript, *superscript),
        _ => return,
    };
    let subscript = subscript.map(|id| cx.ast.clone_subtree(id));
    let superscript = superscript.map(|id| cx.ast.clone_subtree(id));

    let mut rest = Vec::new();
    append_binary_bracket_body(cx, &mut rest, left, right);

    let close_base = cx.ast.new_node(close.node());
    let close = cx.ast.new_node(Node::Scripted {
        base: close_base,
        subscript,
        superscript,
    });
    rest.push(close);

    let open = cx.ast.new_node(open.node());
    cx.ast
        .replace_with_math_sequence(parent, Vec::new(), open, rest);
}

fn append_binary_bracket_body(
    cx: &mut RuleContext<'_>,
    out: &mut Vec<NodeId>,
    left: NodeId,
    right: NodeId,
) {
    cx.ast.append_cloned_math_content(out, left);
    out.push(cx.ast.new_node(Node::Char(',')));
    cx.ast.append_cloned_math_content(out, right);
}
