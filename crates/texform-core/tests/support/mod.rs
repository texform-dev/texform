#![allow(dead_code)]

pub(crate) mod parser;

use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseConfig, ParseContextBuildError, ParseContextBuilder, ParseDiagnostic, ParseResult,
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

pub(crate) fn parse_with_items(
    items: &[ContextItem],
    src: &str,
    reject_unknown: bool,
) -> ParseResult {
    let mut builder = ParseContextBuilder::empty();
    for item in items {
        builder = builder.insert_item(item.clone());
    }
    let ctx = builder.build().expect("context items should be valid");
    let config = if reject_unknown {
        ParseConfig::STRICT
    } else {
        ParseConfig::LENIENT
    };
    ctx.parse(src, &config)
}

pub(crate) fn parse_single_with_items(
    items: &[ContextItem],
    src: &str,
    reject_unknown: bool,
) -> ParseResult {
    let config = if reject_unknown {
        ParseConfig::STRICT
    } else {
        ParseConfig::LENIENT
    };
    let mut outputs = parse_many_with_items(items, &[src], None, &config);
    assert_eq!(outputs.len(), 1);
    outputs.remove(0).output
}

pub(crate) struct ParseCaseOutput {
    pub(crate) input: String,
    pub(crate) output: ParseResult,
}

pub(crate) type ParseManyOutput = Vec<ParseCaseOutput>;

pub(crate) fn parse_many_with_items(
    items: &[ContextItem],
    inputs: &[&str],
    packages: Option<&[&str]>,
    config: &ParseConfig,
) -> ParseManyOutput {
    let mut builder = match packages {
        Some(package_names) => ParseContextBuilder::empty().packages(package_names),
        None => ParseContextBuilder::empty(),
    };

    for item in items {
        builder = builder.insert_item(item.clone());
    }

    let parse_ctx = match builder.build() {
        Ok(parse_ctx) => parse_ctx,
        Err(ParseContextBuildError::PackageLoad(error)) => {
            return invalid_inputs_output(inputs, format!("package loading failed: {error}"));
        }
        Err(ParseContextBuildError::InvalidContextItem { name, source }) => {
            return invalid_inputs_output(
                inputs,
                format!("spec validation failed for {name}: {source}"),
            );
        }
    };

    inputs
        .iter()
        .map(|input| ParseCaseOutput {
            input: (*input).to_string(),
            output: parse_ctx.parse(input, config),
        })
        .collect()
}

fn invalid_input_output(message: String) -> ParseResult {
    ParseResult {
        document: None,
        diagnostics: vec![ParseDiagnostic::new(
            message,
            texform_core::parse::Span { start: 0, end: 0 },
            Vec::new(),
            None,
            Vec::new(),
        )],
    }
}

fn invalid_inputs_output(inputs: &[&str], message: String) -> ParseManyOutput {
    inputs
        .iter()
        .map(|input| ParseCaseOutput {
            input: (*input).to_string(),
            output: invalid_input_output(message.clone()),
        })
        .collect()
}

pub(crate) fn collect_messages(output: &ParseResult) -> Vec<&str> {
    output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

pub(crate) fn assert_first_diagnostic_span_eq(output: &ParseResult, src: &str, expected: &str) {
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
