use std::sync::OnceLock;

use texform_core::ast::Ast;
use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseConfig, ParseContext, ParseContextBuilder,
};
use texform_interface::syntax_node::{Argument, ArgumentValue, ContentMode, SyntaxNode};
use texform_knowledge::specs::load_package_specs_from_str;

pub(crate) fn serialize_node(node: &SyntaxNode) -> String {
    let ast = Ast::from_syntax_root(node);
    texform_core::serialize::serialize(&ast)
}

pub(crate) fn parse(
    src: &str,
    reject_unknown: bool,
) -> Result<(SyntaxNode, chumsky::span::SimpleSpan), Vec<String>> {
    let config = if reject_unknown {
        ParseConfig::STRICT
    } else {
        ParseConfig::LENIENT
    };
    let output = test_context().parse(src, &config);
    if output.diagnostics.is_empty() {
        let result = output
            .try_into_document()
            .expect("parse succeeded without diagnostics but produced no document")
            .0;
        let root_span = result
            .root()
            .span()
            .expect("parsed document root should have a span");
        Ok((
            result.to_syntax(),
            chumsky::span::SimpleSpan::from(root_span.start..root_span.end),
        ))
    } else {
        Err(output
            .diagnostics
            .into_iter()
            .map(|diag| diag.message)
            .collect())
    }
}

pub(crate) fn linebreak_test_items() -> [ContextItem; 2] {
    [
        command_item("\\", CommandKind::Prefix, AllowedMode::Both, "!s !o:L").into(),
        command_item("newline", CommandKind::Prefix, AllowedMode::Both, "!s !o:L").into(),
    ]
}

pub(crate) fn test_context() -> ParseContext {
    static BASE_CTX: OnceLock<ParseContext> = OnceLock::new();
    BASE_CTX
        .get_or_init(|| {
            let mut builder = ParseContextBuilder::empty().packages(&["base"]);
            for item in shared_test_items() {
                builder = builder.insert_item(item.clone());
            }
            for item in linebreak_test_items() {
                builder = builder.insert_item(item);
            }
            builder.build().expect("shared test items should be valid")
        })
        .clone()
}

pub(crate) fn test_context_with_items<I, T>(items: I) -> ParseContext
where
    I: IntoIterator<Item = T>,
    T: Into<ContextItem>,
{
    let mut builder = ParseContextBuilder::empty().packages(&["base"]);
    for item in shared_test_items() {
        builder = builder.insert_item(item.clone());
    }
    for item in linebreak_test_items() {
        builder = builder.insert_item(item);
    }
    for item in items {
        builder = builder.insert_item(item);
    }
    builder
        .build()
        .expect("test items should have valid xparse specs")
}

pub(crate) fn shared_test_items() -> &'static [ContextItem] {
    static ITEMS: OnceLock<Vec<ContextItem>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        let specs = load_package_specs_from_str(
            r#"
commands:
  - name: frac
    kind: prefix
    allowed_mode: math
    argspec: 'm m'
  - name: sqrt
    kind: prefix
    allowed_mode: math
    argspec: 'o m'
  - name: text
    kind: prefix
    allowed_mode: math
    argspec: 'm:T'
  - name: alpha
    kind: prefix
    allowed_mode: math
    argspec: ''
  - name: beta
    kind: prefix
    allowed_mode: math
    argspec: ''
  - name: gamma
    kind: prefix
    allowed_mode: math
    argspec: ''
  - name: delim
    kind: prefix
    allowed_mode: math
    argspec: 'm:D'
  - name: hspace
    kind: prefix
    allowed_mode: both
    argspec: 'm:L'
  - name: romannumeral
    kind: prefix
    allowed_mode: both
    argspec: 'm:I'
  - name: includegraphics
    kind: prefix
    allowed_mode: both
    argspec: 'o:K m:T'
  - name: label
    kind: prefix
    allowed_mode: both
    argspec: 'm:N'
  - name: over
    kind: infix
    allowed_mode: math
    argspec: ''
  - name: choose
    kind: infix
    allowed_mode: math
    argspec: ''
  - name: bfseries
    kind: declarative
    allowed_mode: both
    argspec: ''
  - name: qty
    kind: prefix
    allowed_mode: math
    argspec: 'd<(,)><[,]><{,}><|,|>'
  - name: pqty
    kind: prefix
    allowed_mode: math
    argspec: 's r{}'
  - name: abs
    kind: prefix
    allowed_mode: math
    argspec: 's r{}'
  - name: eval
    kind: prefix
    allowed_mode: math
    argspec: 's d<(,|><[,|><{,}>'
  - name: mqty
    kind: prefix
    allowed_mode: math
    argspec: 's d<(,)><[,]><{,}><|,|>'
  - name: dd
    kind: prefix
    allowed_mode: math
    argspec: 'o d<(,)><{,}>'
  - name: dv
    kind: prefix
    allowed_mode: math
    argspec: 's o m g'
  - name: pdv
    kind: prefix
    allowed_mode: math
    argspec: 's o m g g'
  - name: braket
    kind: prefix
    allowed_mode: math
    argspec: 's m g'
  - name: exp
    kind: prefix
    allowed_mode: math
    argspec: ''

environments:
  - name: matrix
    allowed_mode: math
    argspec: ''
    body_mode: math
  - name: align
    allowed_mode: math
    argspec: ''
    body_mode: math
  - name: align*
    allowed_mode: math
    argspec: ''
    body_mode: math

delimiters:
  - name: "."
    is_control_sequence: false
    allowed_mode: math
    unicode_value: ""
    attributes: {}
  - name: "("
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "("
    attributes: {}
  - name: ")"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: ")"
    attributes: {}
  - name: "["
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "["
    attributes: {}
  - name: "]"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "]"
    attributes: {}
  - name: "|"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "|"
    attributes: {}
  - name: "<"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "<"
    attributes: {}
  - name: ">"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: ">"
    attributes: {}
  - name: "/"
    is_control_sequence: false
    allowed_mode: math
    unicode_value: "/"
    attributes: {}
  - name: langle
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟨"
    attributes: {}
  - name: rangle
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟩"
    attributes: {}
  - name: "{"
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "{"
    attributes: {}
  - name: "}"
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "}"
    attributes: {}
  - name: lfloor
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌊"
    attributes: {}
  - name: rfloor
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌋"
    attributes: {}
  - name: lceil
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌈"
    attributes: {}
  - name: rceil
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⌉"
    attributes: {}
  - name: lvert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "|"
    attributes: {}
  - name: rvert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "|"
    attributes: {}
  - name: lVert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "‖"
    attributes: {}
  - name: rVert
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "‖"
    attributes: {}
  - name: lgroup
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟮"
    attributes: {}
  - name: rgroup
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⟯"
    attributes: {}
  - name: lmoustache
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⎰"
    attributes: {}
  - name: rmoustache
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "⎱"
    attributes: {}
  - name: "|"
    is_control_sequence: true
    allowed_mode: math
    unicode_value: "‖"
    attributes: {}
"#,
            "inline-test",
        );

        let mut items = Vec::new();
        for command in specs.commands {
            items.push(
                CommandItem::new(
                    command.name,
                    command.kind,
                    command.allowed_mode,
                    command.argspec.source,
                )
                .with_tags(command.tags)
                .into(),
            );
        }
        for environment in specs.environments {
            items.push(
                EnvironmentItem::new(
                    environment.name,
                    environment.allowed_mode,
                    environment.body_mode,
                    environment.argspec.source,
                )
                .with_tags(environment.tags)
                .into(),
            );
        }
        for delimiter in specs.delimiters {
            if delimiter.is_control_sequence {
                items.push(DelimiterControlItem::new(delimiter.name).into());
            }
        }
        items
    })
}

pub(crate) fn command_item(
    name: &str,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    spec: &str,
) -> CommandItem {
    CommandItem::new(name, kind, allowed_mode, spec)
}

pub(crate) fn environment_item(
    name: &str,
    allowed_mode: AllowedMode,
    body_mode: ContentMode,
    spec: &str,
) -> EnvironmentItem {
    EnvironmentItem::new(name, allowed_mode, body_mode, spec)
}

pub(crate) fn label_command_item() -> CommandItem {
    command_item("label", CommandKind::Prefix, AllowedMode::Both, "m:N")
}

pub(crate) fn expect_arg(slot: &Option<Argument>) -> &Argument {
    slot.as_ref()
        .unwrap_or_else(|| panic!("Expected argument slot to be present"))
}

pub(crate) fn unwrap_content(slot: &Option<Argument>) -> &SyntaxNode {
    match &expect_arg(slot).value {
        ArgumentValue::MathContent(node)
        | ArgumentValue::TextContent(node)
        | ArgumentValue::OperatorNameContent(node) => node,
        _ => panic!("Expected content argument"),
    }
}

pub(crate) fn assert_same_structure(with_spaces: &str, compact: &str) {
    let (result_spaces, _) = parse(with_spaces, false).unwrap();
    let (result_compact, _) = parse(compact, false).unwrap();
    assert_eq!(result_spaces, result_compact);
}

pub(crate) fn extract_first_command(node: SyntaxNode) -> (String, Vec<Option<Argument>>) {
    match node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => (name.clone(), args.clone()),
            other => panic!("Expected command node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

pub(crate) fn extract_command_args<'a>(
    node: &'a SyntaxNode,
    name: &str,
) -> Option<&'a [Option<Argument>]> {
    match node {
        SyntaxNode::Command {
            name: command_name,
            args,
            ..
        } if command_name == name => Some(args.as_slice()),
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => children
            .iter()
            .find_map(|child| extract_command_args(child, name)),
        SyntaxNode::Command { args, .. } => args.iter().find_map(|slot| {
            slot.as_ref().and_then(|arg| match &arg.value {
                ArgumentValue::MathContent(node)
                | ArgumentValue::TextContent(node)
                | ArgumentValue::OperatorNameContent(node) => extract_command_args(node, name),
                _ => None,
            })
        }),
        SyntaxNode::Declarative { args, .. } => args.iter().find_map(|slot| {
            slot.as_ref().and_then(|arg| match &arg.value {
                ArgumentValue::MathContent(node)
                | ArgumentValue::TextContent(node)
                | ArgumentValue::OperatorNameContent(node) => extract_command_args(node, name),
                _ => None,
            })
        }),
        SyntaxNode::Environment { args, body, .. } => args
            .iter()
            .find_map(|slot| {
                slot.as_ref().and_then(|arg| match &arg.value {
                    ArgumentValue::MathContent(node)
                    | ArgumentValue::TextContent(node)
                    | ArgumentValue::OperatorNameContent(node) => extract_command_args(node, name),
                    _ => None,
                })
            })
            .or_else(|| extract_command_args(body, name)),
        SyntaxNode::Infix {
            args, left, right, ..
        } => args
            .iter()
            .find_map(|slot| {
                slot.as_ref().and_then(|arg| match &arg.value {
                    ArgumentValue::MathContent(node)
                    | ArgumentValue::TextContent(node)
                    | ArgumentValue::OperatorNameContent(node) => extract_command_args(node, name),
                    _ => None,
                })
            })
            .or_else(|| extract_command_args(left, name))
            .or_else(|| extract_command_args(right, name)),
        _ => None,
    }
}
