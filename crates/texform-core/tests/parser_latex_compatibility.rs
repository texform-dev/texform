//! LaTeX records beyond the MathJax baseline: text containers, `\ensuremath`,
//! and extra delimiters.

mod support;

use support::parser::{expect_arg, extract_command_args, serialize_node};
use texform_core::parse::{ParseConfig, ParseContext};
use texform_interface::syntax_node::{
    ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};

/// Parse without diagnostics and check that the serialized output reparses to
/// the same tree.
fn parse_stable(source: &str) -> SyntaxNode {
    let ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let parse = |source: &str| {
        let output = ctx.parse(source, &ParseConfig::STRICT);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {source}: {:?}",
            output.diagnostics
        );
        output
            .document()
            .unwrap_or_else(|| panic!("expected complete parse for {source}"))
            .to_syntax()
    };
    let syntax = parse(source);
    let serialized = serialize_node(&syntax);
    assert_eq!(parse(&serialized), syntax, "{source} => {serialized}");
    syntax
}

fn content_nodes(node: &SyntaxNode) -> &[SyntaxNode] {
    match node {
        SyntaxNode::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            children,
            ..
        } => children,
        node => std::slice::from_ref(node),
    }
}

fn is_squared(node: &SyntaxNode) -> bool {
    matches!(
        node,
        SyntaxNode::Scripted {
            superscript: Some(_),
            ..
        }
    )
}

/// Return the last argument of `command`, which must hold text content.
fn text_body<'a>(syntax: &'a SyntaxNode, command: &str) -> &'a [SyntaxNode] {
    let args = extract_command_args(syntax, command)
        .unwrap_or_else(|| panic!("expected `{command}` in {syntax:?}"));
    let value = &expect_arg(args.last().expect("body argument")).value;
    let ArgumentValue::TextContent(body) = value else {
        panic!("expected text-mode body for `{command}`, got {value:?}");
    };
    content_nodes(body)
}

fn contains_inline_square(nodes: &[SyntaxNode]) -> bool {
    nodes.iter().any(|node| {
        matches!(
            node,
            SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::InlineMath,
                children,
            } if matches!(&children[..], [child] if is_squared(child))
        )
    })
}

#[test]
fn text_command_nests_in_text_mode() {
    let syntax = parse_stable(r"\text{a \text{b} c}");
    let outer = extract_command_args(&syntax, "text").expect("outer text command");
    let ArgumentValue::TextContent(body) = &expect_arg(&outer[0]).value else {
        panic!("expected text-mode content");
    };
    assert_eq!(text_body(body, "text"), [SyntaxNode::Text("b".to_string())]);
}

#[test]
fn ensuremath_parses_math_content_in_both_modes() {
    for source in [r"\ensuremath{x^{2}}", r"\text{\ensuremath{x^{2}}}"] {
        let syntax = parse_stable(source);
        let args = extract_command_args(&syntax, "ensuremath").expect("ensuremath command");
        assert!(
            matches!(
                &expect_arg(&args[0]).value,
                ArgumentValue::MathContent(body) if matches!(content_nodes(body), [child] if is_squared(child))
            ),
            "{source}: {args:?}"
        );
    }
}

#[test]
fn emph_keeps_its_text_body_when_called_from_math_mode() {
    for source in [r"\emph{x}", r"\text{\emph{x}}"] {
        let syntax = parse_stable(source);
        assert_eq!(
            text_body(&syntax, "emph"),
            [SyntaxNode::Text("x".to_string())]
        );
    }
}

#[test]
fn raisebox_reads_optional_height_and_depth() {
    for (source, height, depth) in [
        (r"\raisebox{1ex}{$x^{2}$}", None, None),
        (r"\raisebox{1ex}[2ex]{$x^{2}$}", Some("2ex"), None),
        (
            r"\text{\raisebox{1ex}[2ex][0pt]{$x^{2}$}}",
            Some("2ex"),
            Some("0pt"),
        ),
    ] {
        let syntax = parse_stable(source);
        let args = extract_command_args(&syntax, "raisebox").expect("raisebox command");
        let dimension =
            |value: Option<&str>| value.map(|value| ArgumentValue::Dimension(value.to_string()));
        let values: Vec<_> = args[..3]
            .iter()
            .map(|arg| arg.as_ref().map(|arg| arg.value.clone()))
            .collect();
        assert_eq!(
            values,
            [dimension(Some("1ex")), dimension(height), dimension(depth)],
            "{source}"
        );
        assert!(contains_inline_square(text_body(&syntax, "raisebox")));
    }
}

#[test]
fn scalebox_reads_signed_decimal_scales() {
    for (source, horizontal, vertical) in [
        (r"\scalebox{2}{$x^{2}$}", "2", None),
        (r"\scalebox{.5}[-1.25]{$x^{2}$}", ".5", Some("-1.25")),
        (
            r"\text{\scalebox{-2.5}[+.75]{$x^{2}$}}",
            "-2.5",
            Some("+.75"),
        ),
    ] {
        let syntax = parse_stable(source);
        let args = extract_command_args(&syntax, "scalebox").expect("scalebox command");
        let scale = |value: &str| ArgumentValue::CSName(value.to_string());
        assert_eq!(expect_arg(&args[0]).value, scale(horizontal), "{source}");
        assert_eq!(
            args[1].as_ref().map(|arg| arg.value.clone()),
            vertical.map(scale),
            "{source}"
        );
        assert!(contains_inline_square(text_body(&syntax, "scalebox")));
    }
}

#[test]
fn text_boxes_are_allowed_inside_text_containers() {
    for (command, boxed) in [
        ("mbox", r"\mbox{$x^{2}$}"),
        ("fbox", r"\fbox{$x^{2}$}"),
        ("hbox", r"\hbox{$x^{2}$}"),
        ("parbox", r"\parbox[t]{2em}{$x^{2}$}"),
        ("makebox", r"\makebox[2em][l]{$x^{2}$}"),
    ] {
        let syntax = parse_stable(&format!(r"\raisebox{{1ex}}{{{boxed}}}"));
        assert!(
            contains_inline_square(text_body(&syntax, command)),
            "{boxed}"
        );
    }
}

#[test]
fn label_is_allowed_inside_text_containers() {
    for source in [
        r"\text{\label{eq:key}x}",
        r"\scalebox{2}{\label{eq:key}$x^{2}$}",
    ] {
        let syntax = parse_stable(source);
        let args = extract_command_args(&syntax, "label").expect("label command");
        assert_eq!(
            expect_arg(&args[0]).value,
            ArgumentValue::CSName("eq:key".to_string())
        );
    }
}

#[test]
fn math_environment_enters_math_mode_inside_text_containers() {
    for (command, wrapper) in [("text", r"\text"), ("scalebox", r"\scalebox{.8}")] {
        let source = format!(r"{wrapper}{{before \begin{{math}}x^{{2}}\end{{math}} after}}");
        let syntax = parse_stable(&source);
        let [
            SyntaxNode::Text(before),
            SyntaxNode::Environment {
                name, body, known, ..
            },
            SyntaxNode::Text(after),
        ] = text_body(&syntax, command)
        else {
            panic!("expected a math environment between text nodes for {source}");
        };
        assert_eq!((before.as_str(), after.as_str()), ("before ", " after"));
        assert_eq!(name, "math");
        assert!(*known);
        assert!(
            matches!(
                body.as_ref(),
                SyntaxNode::Group { mode: ContentMode::Math, children, .. }
                    if matches!(&children[..], [child] if is_squared(child))
            ),
            "{source}: {body:?}"
        );
    }
    // Math-mode entry still works.
    parse_stable(r"\begin{math}x^{2}\end{math}");
}

#[test]
fn double_square_brackets_are_registered_delimiters() {
    let syntax = parse_stable(r"\left\llbracket x\right\rrbracket");
    let SyntaxNode::Root { children, .. } = &syntax else {
        panic!("expected root");
    };
    assert!(
        matches!(
            &children[..],
            [SyntaxNode::Group {
                kind: GroupKind::Delimited {
                    left: Delimiter::Control(left),
                    right: Delimiter::Control(right),
                },
                ..
            }] if *left == "llbracket" && *right == "rrbracket"
        ),
        "{children:?}"
    );
}
