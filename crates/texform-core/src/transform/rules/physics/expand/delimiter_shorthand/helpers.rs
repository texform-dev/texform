use crate::ast::{ContentMode, Delimiter, GroupKind, Node, NodeId, Slot};
use crate::transform::helpers::{
    append_cloned_math_content, replace_node_discarding_detached_children,
    replace_with_math_sequence,
};
use crate::transform::rule_context::RuleContext;

#[derive(Clone, Copy)]
pub(super) enum FixedFenceToken {
    Char(char),
    Control(&'static str),
}

pub(super) struct FencePair {
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

pub(super) fn replace_with_delimiter_shorthand(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    starred: bool,
    body: NodeId,
    fences: FencePair,
) {
    if starred {
        replace_with_fixed_fence(cx, node_id, body, fences.fixed_left, fences.fixed_right);
    } else {
        replace_with_auto_fence(cx, node_id, body, fences.auto_left, fences.auto_right);
    }
}

fn replace_with_auto_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    body: NodeId,
    left: Delimiter,
    right: Delimiter,
) {
    let mut children = Vec::new();
    append_cloned_math_content(cx, &mut children, body);

    replace_node_discarding_detached_children(
        cx,
        node_id,
        Node::Group {
            children,
            kind: GroupKind::Delimited { left, right },
            mode: ContentMode::Math,
        },
    );
}

fn replace_with_fixed_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    body: NodeId,
    left: FixedFenceToken,
    right: FixedFenceToken,
) {
    if matches!(cx.ast.slot(node_id), Some(Slot::ScriptBase)) {
        replace_scripted_base_with_fixed_fence(cx, node_id, body, left, right);
        return;
    }

    let mut rest = Vec::new();
    append_cloned_math_content(cx, &mut rest, body);
    rest.push(cx.ast.new_node(right.node()));

    replace_with_math_sequence(cx, node_id, Vec::new(), left.node(), rest);
}

fn replace_scripted_base_with_fixed_fence(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    body: NodeId,
    left: FixedFenceToken,
    right: FixedFenceToken,
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
    append_cloned_math_content(cx, &mut rest, body);

    let close_base = cx.ast.new_node(right.node());
    let close = cx.ast.new_node(Node::Scripted {
        base: close_base,
        subscript,
        superscript,
    });
    rest.push(close);

    replace_with_math_sequence(cx, parent, Vec::new(), left.node(), rest);
}
