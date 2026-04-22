#![allow(dead_code)]

use texform_core::api::parse_with_context_items;
use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseContextBuilder, ParseOutput,
};
use texform_interface::syntax_node::{Argument, ArgumentValue, ContentMode, SyntaxNode};

pub(crate) fn command_item(
    name: &str,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    spec: &str,
) -> ContextItem {
    CommandItem::new(name, kind, allowed_mode, spec).into()
}

pub(crate) fn environment_item(
    name: &str,
    allowed_mode: AllowedMode,
    body_mode: ContentMode,
    spec: &str,
) -> ContextItem {
    EnvironmentItem::new(name, allowed_mode, body_mode, spec).into()
}

pub(crate) fn delimiter_control_item(name: &str) -> ContextItem {
    DelimiterControlItem::new(name).into()
}

pub(crate) fn parse_with_items(items: &[ContextItem], src: &str, strict: bool) -> ParseOutput {
    let mut builder = ParseContextBuilder::empty();
    for item in items {
        builder = builder.insert_item(item.clone());
    }
    let ctx = builder.build().expect("context items should be valid");
    ctx.parse(src, strict)
}

pub(crate) fn parse_single_via_public_api(
    items: &[ContextItem],
    src: &str,
    strict: bool,
) -> ParseOutput {
    let mut outputs = parse_with_context_items(items, &[src], None, strict);
    assert_eq!(outputs.len(), 1);
    outputs.remove(0).output
}

pub(crate) fn collect_messages(output: &ParseOutput) -> Vec<&str> {
    output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

pub(crate) fn assert_first_diagnostic_span_eq(output: &ParseOutput, src: &str, expected: &str) {
    let diagnostic = output
        .diagnostics
        .first()
        .expect("expected at least one diagnostic");
    assert_eq!(&src[diagnostic.span.start..diagnostic.span.end], expected);
}

fn slot_contains_error(slot: &Option<Argument>) -> bool {
    slot.as_ref().is_some_and(|arg| match &arg.value {
        ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
            contains_error_node(node)
        }
        _ => false,
    })
}

pub(crate) fn contains_error_node(node: &SyntaxNode) -> bool {
    match node {
        SyntaxNode::Error { .. } => true,
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => {
            children.iter().any(contains_error_node)
        }
        SyntaxNode::Command { args, .. } => args.iter().any(slot_contains_error),
        SyntaxNode::Declarative { args, .. } => args.iter().any(slot_contains_error),
        SyntaxNode::Environment { args, body, .. } => {
            args.iter().any(slot_contains_error) || contains_error_node(body)
        }
        SyntaxNode::Infix {
            args, left, right, ..
        } => {
            args.iter().any(slot_contains_error)
                || contains_error_node(left)
                || contains_error_node(right)
        }
        _ => false,
    }
}

fn slot_contains_command_named(slot: &Option<Argument>, name: &str) -> bool {
    slot.as_ref().is_some_and(|arg| match &arg.value {
        ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
            contains_command_named(node, name)
        }
        _ => false,
    })
}

pub(crate) fn contains_command_named(node: &SyntaxNode, name: &str) -> bool {
    match node {
        SyntaxNode::Command {
            name: node_name, ..
        } if node_name == name => true,
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => children
            .iter()
            .any(|child| contains_command_named(child, name)),
        SyntaxNode::Command { args, .. } => args
            .iter()
            .any(|slot| slot_contains_command_named(slot, name)),
        SyntaxNode::Declarative { args, .. } => args
            .iter()
            .any(|slot| slot_contains_command_named(slot, name)),
        SyntaxNode::Environment { args, body, .. } => {
            args.iter()
                .any(|slot| slot_contains_command_named(slot, name))
                || contains_command_named(body, name)
        }
        SyntaxNode::Infix {
            args, left, right, ..
        } => {
            args.iter()
                .any(|slot| slot_contains_command_named(slot, name))
                || contains_command_named(left, name)
                || contains_command_named(right, name)
        }
        _ => false,
    }
}
