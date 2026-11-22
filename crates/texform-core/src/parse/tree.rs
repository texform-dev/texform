//! Syntax-aligned span tree and parser-private tree assembly helpers.
//!
//! Shared between the parser and [`crate::document::Document`]. Does not depend
//! on `grammar/`.

use texform_interface::syntax_node::{ArgumentSlot, ContentMode, GroupKind, SyntaxNode};

use crate::parse::result::Span;

/// Positional span subtree mirroring a `SyntaxNode`. It carries the node's own
/// source span plus the span subtrees of its spannable children, addressed by
/// structural position rather than a path string.
/// [`crate::document::Document::from_syntax_with_spans`] walks it in lockstep
/// with the converted arena to attach spans, so there is no path formatting or
/// hashing on either the producer or the consumer side.
///
/// `kids` lists child span subtrees in the exact order the consumer visits
/// them:
/// - Group / Root: each child, in order.
/// - Command / Declarative: each content-bearing argument slot, in slot order.
/// - Infix: `left`, then each content-bearing argument slot, then `right`.
/// - Environment: each content-bearing argument slot, then `body`.
/// - Scripted: `base`, then `subscript` if present, then `superscript` if present.
/// - Leaves carry no kids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpanTree {
    pub span: Span,
    pub kids: Vec<SpanTree>,
}

/// Parser-private syntax node paired with its source span and positional child
/// span subtrees.
///
/// Diagnostics are written directly to `TokenParser` rather than carried on
/// every syntax node.
#[derive(Debug, Clone)]
pub(crate) struct ParsedNode {
    pub node: SyntaxNode,
    pub span: Span,
    pub span_kids: Vec<SpanTree>,
}

/// Parsed argument slot bundled with its optional content span subtree.
///
/// The content [`SyntaxNode`] is moved directly into `slot` (zero cloning), so
/// only its [`SpanTree`] is retained alongside the slot.
#[derive(Debug, Clone)]
pub(crate) struct TrackedArgSlot {
    pub slot: ArgumentSlot,
    pub content_span: Option<SpanTree>,
}

impl TrackedArgSlot {
    /// Wrap an argument slot that carries no content subtree.
    #[inline]
    pub(crate) fn untracked(slot: ArgumentSlot) -> Self {
        Self {
            slot,
            content_span: None,
        }
    }

    /// Wrap an argument slot whose content subtree was moved into `slot`.
    #[inline]
    pub(crate) fn with_content(slot: ArgumentSlot, content_span: SpanTree) -> Self {
        Self {
            slot,
            content_span: Some(content_span),
        }
    }
}

impl ParsedNode {
    /// Wrap a syntax node with no spannable children.
    #[inline]
    pub(crate) fn leaf(node: SyntaxNode, span: Span) -> Self {
        Self {
            node,
            span,
            span_kids: Vec::new(),
        }
    }

    /// Wrap a syntax node with pre-ordered child span subtrees.
    #[inline]
    pub(crate) fn with_kids(node: SyntaxNode, span: Span, span_kids: Vec<SpanTree>) -> Self {
        Self {
            node,
            span,
            span_kids,
        }
    }

    /// Decompose into the owned syntax node and its [`SpanTree`].
    #[inline]
    pub(crate) fn into_span_tree(self) -> (SyntaxNode, SpanTree) {
        (
            self.node,
            SpanTree {
                span: self.span,
                kids: self.span_kids,
            },
        )
    }

    /// Promote the top-level implicit group into a [`SyntaxNode::Root`] and
    /// return it together with its rooted [`SpanTree`].
    pub(crate) fn finish_root(self) -> (SyntaxNode, SpanTree) {
        let root_node = promote_to_root(self.node);
        let span_tree = SpanTree {
            span: self.span,
            kids: self.span_kids,
        };
        (root_node, span_tree)
    }
}

/// Promote a top-level implicit group into a real [`SyntaxNode::Root`].
pub(crate) fn promote_to_root(node: SyntaxNode) -> SyntaxNode {
    match node {
        node @ SyntaxNode::Root { .. } => node,
        SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children,
        } => SyntaxNode::Root { mode, children },
        other => panic!(
            "top-level parser must finish as implicit group or root, got {:?}",
            other
        ),
    }
}

/// Returns `true` for explicit or implicit brace groups (not delimited or
/// inline-math groups).
#[inline]
pub(crate) fn is_brace_group(node: &SyntaxNode) -> bool {
    matches!(
        node,
        SyntaxNode::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            ..
        }
    )
}

/// Extract child syntax nodes and one [`SpanTree`] per child, in order.
pub(crate) fn decompose_children(children: Vec<ParsedNode>) -> (Vec<SyntaxNode>, Vec<SpanTree>) {
    let mut nodes = Vec::with_capacity(children.len());
    let mut kids = Vec::with_capacity(children.len());
    for child in children {
        nodes.push(child.node);
        kids.push(SpanTree {
            span: child.span,
            kids: child.span_kids,
        });
    }
    (nodes, kids)
}

/// Extract argument slots and one [`SpanTree`] per content-bearing slot, in
/// slot order.
///
/// Slots without content (e.g. scalar, star, or absent optionals) contribute no
/// span subtree, matching `Document::assign_arg_spans`.
pub(crate) fn decompose_args(slots: Vec<TrackedArgSlot>) -> (Vec<ArgumentSlot>, Vec<SpanTree>) {
    let mut out_slots = Vec::with_capacity(slots.len());
    let mut kids = Vec::new();
    for arg in slots {
        if let Some(tree) = arg.content_span {
            kids.push(tree);
        }
        out_slots.push(arg.slot);
    }
    (out_slots, kids)
}

/// Fold a sequence of parsed items into a single [`ParsedNode`] spanning `span`.
///
/// - 0 items -> empty implicit group
/// - 1 item  -> unwrap (reuse child's span subtrees, override span)
/// - N items -> implicit group with one child span subtree per item
pub(crate) fn fold_items(mode: ContentMode, items: Vec<ParsedNode>, span: Span) -> ParsedNode {
    match items.len() {
        0 => ParsedNode::leaf(
            SyntaxNode::Group {
                mode,
                kind: GroupKind::Implicit,
                children: Vec::new(),
            },
            span,
        ),
        1 => {
            let child = items.into_iter().next().expect("checked len == 1");
            ParsedNode { span, ..child }
        }
        _ => {
            let (children, span_kids) = decompose_children(items);
            ParsedNode::with_kids(
                SyntaxNode::Group {
                    mode,
                    kind: GroupKind::Implicit,
                    children,
                },
                span,
                span_kids,
            )
        }
    }
}

/// Fold argument content without unwrapping a sole user-written brace group.
///
/// The outer implicit group represents the argument slot braces while its sole
/// child preserves the user's explicit braces.
pub(crate) fn fold_argument(mode: ContentMode, items: Vec<ParsedNode>, span: Span) -> ParsedNode {
    if let [item] = items.as_slice()
        && is_brace_group(&item.node)
    {
        let (children, span_kids) = decompose_children(items);
        ParsedNode::with_kids(
            SyntaxNode::Group {
                mode,
                kind: GroupKind::Implicit,
                children,
            },
            span,
            span_kids,
        )
    } else {
        fold_items(mode, items, span)
    }
}

/// Compute the aggregate span covering `items` (`first.start..last.end`), or a
/// zero-width point span at `fallback` when `items` is empty.
#[inline]
pub(crate) fn items_span(items: &[ParsedNode], fallback: usize) -> Span {
    match (items.first(), items.last()) {
        (Some(first), Some(last)) => Span::new(first.span.start, last.span.end),
        _ => Span::point(fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use texform_interface::syntax_node::{Argument, ArgumentKind, ArgumentValue};

    #[test]
    fn fold_items_handles_empty_single_and_multiple() {
        let empty = fold_items(ContentMode::Math, Vec::new(), Span::point(3));
        assert_eq!(
            empty.node,
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Implicit,
                children: Vec::new(),
            }
        );
        assert_eq!(empty.span, Span::point(3));
        assert!(empty.span_kids.is_empty());

        let single = fold_items(
            ContentMode::Math,
            vec![ParsedNode::leaf(SyntaxNode::Char('x'), Span::new(1, 2))],
            Span::new(0, 3),
        );
        assert_eq!(single.node, SyntaxNode::Char('x'));
        assert_eq!(single.span, Span::new(0, 3));

        let multi = fold_items(
            ContentMode::Math,
            vec![
                ParsedNode::leaf(SyntaxNode::Char('a'), Span::new(0, 1)),
                ParsedNode::leaf(SyntaxNode::Char('b'), Span::new(1, 2)),
            ],
            Span::new(0, 2),
        );
        assert_eq!(multi.span_kids.len(), 2);
        assert_eq!(multi.span_kids[0].span, Span::new(0, 1));
        assert_eq!(multi.span_kids[1].span, Span::new(1, 2));
    }

    #[test]
    fn fold_argument_retains_container_around_brace_group() {
        let explicit = ParsedNode::with_kids(
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Explicit,
                children: vec![SyntaxNode::Char('x')],
            },
            Span::new(1, 4),
            vec![SpanTree {
                span: Span::new(2, 3),
                kids: Vec::new(),
            }],
        );
        let folded = fold_argument(ContentMode::Math, vec![explicit], Span::new(0, 5));
        assert!(matches!(
            folded.node,
            SyntaxNode::Group {
                kind: GroupKind::Implicit,
                ref children,
                ..
            } if children.len() == 1
        ));
        assert_eq!(folded.span_kids.len(), 1);
        assert_eq!(folded.span_kids[0].span, Span::new(1, 4));
        assert_eq!(folded.span_kids[0].kids[0].span, Span::new(2, 3));
    }

    #[test]
    fn decompose_args_preserves_content_slot_order_only() {
        let (node, tree) =
            ParsedNode::leaf(SyntaxNode::Char('a'), Span::new(6, 7)).into_span_tree();
        let slots = vec![
            TrackedArgSlot::untracked(None),
            TrackedArgSlot::with_content(
                Some(Argument::from_value(
                    ArgumentKind::Mandatory,
                    ArgumentValue::MathContent(node),
                )),
                tree,
            ),
            TrackedArgSlot::untracked(Some(Argument::from_value(
                ArgumentKind::Star,
                ArgumentValue::Boolean(true),
            ))),
        ];
        let (out_slots, kids) = decompose_args(slots);
        assert_eq!(out_slots.len(), 3);
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].span, Span::new(6, 7));
    }

    #[test]
    fn finish_root_promotes_implicit_group() {
        let child = ParsedNode::leaf(SyntaxNode::Char('x'), Span::new(0, 1));
        let (children, kids) = decompose_children(vec![child]);
        let group = ParsedNode::with_kids(
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Implicit,
                children,
            },
            Span::new(0, 1),
            kids,
        );
        let (root, span_tree) = group.finish_root();
        assert_eq!(
            root,
            SyntaxNode::Root {
                mode: ContentMode::Math,
                children: vec![SyntaxNode::Char('x')],
            }
        );
        assert_eq!(span_tree.span, Span::new(0, 1));
        assert_eq!(span_tree.kids.len(), 1);
        assert_eq!(span_tree.kids[0].span, Span::new(0, 1));
    }
}
