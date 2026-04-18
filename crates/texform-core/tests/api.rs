use texform_core::api::{
    parse_latex, parse_with_context_items, serialize_latex, serialize_latex_with,
};
use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseContextBuilder, ParseOutput,
};
use texform_core::serialize::SerializeOptions;
use texform_interface::syntax_node::{ArgumentValue, ContentMode, Delimiter, SyntaxNode};

fn command_item(
    name: &str,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    spec: &str,
) -> ContextItem {
    CommandItem::new(name, kind, allowed_mode, spec).into()
}

fn environment_item(
    name: &str,
    allowed_mode: AllowedMode,
    body_mode: ContentMode,
    spec: &str,
) -> ContextItem {
    EnvironmentItem::new(name, allowed_mode, body_mode, spec).into()
}

fn delimiter_control_item(name: &str) -> ContextItem {
    DelimiterControlItem::new(name).into()
}

fn text_command_item() -> ContextItem {
    command_item("text", CommandKind::Prefix, AllowedMode::Math, "m:T")
}

fn frac_command_item() -> ContextItem {
    command_item("frac", CommandKind::Prefix, AllowedMode::Math, "m m")
}

fn matrix_environment_item() -> ContextItem {
    environment_item("matrix", AllowedMode::Math, ContentMode::Math, "")
}

fn parse_with_items(items: &[ContextItem], src: &str, strict: bool) -> ParseOutput {
    let mut builder = ParseContextBuilder::new().core_only();
    for item in items {
        builder = builder.insert_item(item.clone());
    }
    let ctx = builder.build().expect("context items should be valid");
    ctx.parse(src, strict)
}

fn assert_first_diagnostic_span_eq(output: &ParseOutput, src: &str, expected: &str) {
    let diagnostic = output
        .diagnostics
        .first()
        .expect("expected at least one diagnostic");
    assert_eq!(&src[diagnostic.span.start..diagnostic.span.end], expected);
}

fn assert_first_diagnostic_expected_found(
    output: &ParseOutput,
    expected: &[&str],
    found: Option<&str>,
) {
    let diagnostic = output
        .diagnostics
        .first()
        .expect("expected at least one diagnostic");
    assert_eq!(diagnostic.expected, expected);
    assert_eq!(diagnostic.found.as_deref(), found);
}

fn slot_contains_error(slot: &Option<texform_interface::syntax_node::Argument>) -> bool {
    slot.as_ref().is_some_and(|arg| match &arg.value {
        ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
            contains_error_node(node)
        }
        _ => false,
    })
}

fn slot_contains_command_named(
    slot: &Option<texform_interface::syntax_node::Argument>,
    name: &str,
) -> bool {
    slot.as_ref().is_some_and(|arg| match &arg.value {
        ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
            contains_command_named(node, name)
        }
        _ => false,
    })
}

fn collect_messages(output: &ParseOutput) -> Vec<&str> {
    output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

fn contains_error_node(node: &SyntaxNode) -> bool {
    match node {
        SyntaxNode::Error { .. } => true,
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => {
            children.iter().any(contains_error_node)
        }
        SyntaxNode::Command { args, .. } => args.iter().any(slot_contains_error),
        SyntaxNode::Declarative { args, scope, .. } => {
            args.iter().any(slot_contains_error) || contains_error_node(scope)
        }
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

fn contains_command_named(node: &SyntaxNode, name: &str) -> bool {
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
        SyntaxNode::Declarative { args, scope, .. } => {
            args.iter()
                .any(|slot| slot_contains_command_named(slot, name))
                || contains_command_named(scope, name)
        }
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

#[test]
fn full_success() {
    let output = parse_latex(r"\\*[1cm]", false);
    assert!(output.result.is_some(), "should produce a result");
    assert!(output.diagnostics.is_empty(), "no diagnostics expected");

    let res = output.result.unwrap();
    assert_eq!(res.span.start, 0);
    assert_eq!(res.span.end, 8);

    let json = serde_json::to_value(&res).unwrap();
    assert!(json.get("node").is_some());
    assert!(json.get("span").is_some());
}

#[test]
fn pure_failure_strict() {
    let output = parse_latex(r"\unknowncmd", true);
    assert!(output.result.is_none(), "strict unknown should fail");
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");
}

#[test]
fn partial_success_or_failure() {
    let output = parse_with_items(&[frac_command_item()], r"\frac{a}{", false);
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");

    let diagnostic = &output.diagnostics[0];
    assert!(!diagnostic.message.is_empty());
}

#[test]
fn mode_error_for_math_only_command_in_text() {
    let output = parse_with_items(
        &[text_command_item(), frac_command_item()],
        r"\text{\frac{a}{b}}",
        true,
    );
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");
}

#[test]
fn content_argument_partial_result_keeps_outer_text_command() {
    let output = parse_with_items(
        &[text_command_item(), frac_command_item()],
        r"\text{\frac{a}{b}}",
        false,
    );

    assert_eq!(
        collect_messages(&output),
        vec![r"Command \frac is not allowed in text mode"]
    );

    let result = output
        .result
        .as_ref()
        .expect("should produce a partial result");

    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };

    let text_args = match root_children.first() {
        Some(SyntaxNode::Command { name, args, .. }) => {
            assert_eq!(name, "text");
            args
        }
        other => panic!("expected first child to be \\text, got {:?}", other),
    };

    let text_arg = text_args[0]
        .as_ref()
        .expect("text should keep its argument slot");
    let content = match &text_arg.value {
        ArgumentValue::TextContent(node) => node,
        other => panic!("expected text content argument, got {:?}", other),
    };

    assert!(
        contains_error_node(content),
        "recoverable content subparse should keep an Error placeholder"
    );
}

#[test]
fn nested_recoverable_content_keeps_inner_diagnostics() {
    let output = parse_with_items(
        &[text_command_item(), frac_command_item()],
        r"\frac{\text{\frac{a}{b}}}{c}",
        false,
    );

    assert!(output.result.is_some(), "should produce a partial result");
    assert!(
        collect_messages(&output).contains(&r"Command \frac is not allowed in text mode"),
        "nested recoverable content should keep the inner mode diagnostic"
    );
}

#[test]
fn text_scripted_content_reports_only_direct_error() {
    let output = parse_with_items(
        &[
            text_command_item(),
            command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m"),
            command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T"),
        ],
        r"\text{\underline{a^2}}",
        false,
    );

    assert_eq!(
        collect_messages(&output),
        vec!["Scripted syntax is not allowed in Text mode"]
    );

    let result = output
        .result
        .as_ref()
        .expect("scripted text should still keep a partial result");

    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };

    let text_args = match root_children.first() {
        Some(SyntaxNode::Command { name, args, .. }) => {
            assert_eq!(name, "text");
            args
        }
        other => panic!("expected first child to be \\text, got {:?}", other),
    };

    let text_arg = text_args[0]
        .as_ref()
        .expect("text should keep its argument slot");
    let underline_args = match &text_arg.value {
        ArgumentValue::TextContent(SyntaxNode::Command { name, args, .. }) => {
            assert_eq!(name, "underline");
            args
        }
        other => panic!("expected recoverable underline command, got {:?}", other),
    };

    let underline_arg = underline_args[0]
        .as_ref()
        .expect("underline should keep its text argument");
    let underline_content = match &underline_arg.value {
        ArgumentValue::TextContent(node) => node,
        other => panic!("expected underline text content, got {:?}", other),
    };
    assert!(contains_error_node(underline_content));
}

#[test]
fn nonstrict_direct_error_survives_trailing_outer_generic() {
    let output = parse_with_items(
        &[
            text_command_item(),
            command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m"),
            command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T"),
        ],
        r"\text{\underline{a^2}$}",
        false,
    );

    assert_eq!(
        collect_messages(&output),
        vec!["Scripted syntax is not allowed in Text mode"]
    );
    assert!(output.result.is_some(), "should keep a partial result");

    let result = output
        .result
        .as_ref()
        .expect("mixed direct/generic error should keep a partial result");
    assert!(contains_command_named(&result.node, "underline"));
    assert!(contains_error_node(&result.node));
}

#[test]
fn nonstrict_command_direct_error_survives_trailing_outer_generic() {
    let output = parse_with_items(
        &[text_command_item(), frac_command_item()],
        r"\text{\frac{a}{b}$}",
        false,
    );

    assert_eq!(
        collect_messages(&output),
        vec![r"Command \frac is not allowed in text mode"]
    );
    assert!(output.result.is_some(), "should keep a partial result");

    let result = output
        .result
        .as_ref()
        .expect("command mixed error should keep a partial result");
    assert!(contains_command_named(&result.node, "text"));

    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };
    let text_args = match root_children.first() {
        Some(SyntaxNode::Command { name, args, .. }) => {
            assert_eq!(name, "text");
            args
        }
        other => panic!("expected first child to be \\text, got {:?}", other),
    };
    let text_arg = text_args[0]
        .as_ref()
        .expect("text should keep its argument slot");
    let text_content = match &text_arg.value {
        ArgumentValue::TextContent(node) => node,
        other => panic!("expected text content, got {:?}", other),
    };
    assert!(contains_error_node(text_content));
}

#[test]
fn generic_only_content_error_is_not_filtered_out() {
    let output = parse_with_items(&[text_command_item()], r"\text{$x}", false);

    assert_eq!(
        collect_messages(&output),
        vec!["found '$' expected something else, or end of input"]
    );
    assert!(
        output.result.is_some(),
        "generic-only content error should still keep outer text shell"
    );

    let result = output
        .result
        .as_ref()
        .expect("generic-only content error should keep a partial result");
    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };
    let text_args = match root_children.first() {
        Some(SyntaxNode::Command { name, args, .. }) => {
            assert_eq!(name, "text");
            args
        }
        other => panic!("expected first child to be \\text, got {:?}", other),
    };
    let text_arg = text_args[0]
        .as_ref()
        .expect("text should keep its argument slot");
    let text_content = match &text_arg.value {
        ArgumentValue::TextContent(node) => node,
        other => panic!("expected text content argument, got {:?}", other),
    };
    match text_content {
        SyntaxNode::Error { message, .. } => {
            assert_eq!(
                message,
                "found '$' expected something else, or end of input"
            );
        }
        other => panic!("expected recovered Error node, got {:?}", other),
    }
}

#[test]
fn nested_content_arguments_merge_inner_direct_error_once() {
    let output = parse_with_items(
        &[
            text_command_item(),
            command_item("fbox", CommandKind::Prefix, AllowedMode::Text, "m:T"),
            command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m"),
            command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T"),
        ],
        r"\text{\fbox{\underline{a^2}}}",
        false,
    );

    assert_eq!(
        collect_messages(&output),
        vec!["Scripted syntax is not allowed in Text mode"]
    );

    let result = output
        .result
        .as_ref()
        .expect("nested content arguments should still keep a partial result");

    assert!(contains_command_named(&result.node, "text"));
    assert!(contains_command_named(&result.node, "fbox"));
    assert!(contains_error_node(&result.node));
}

#[test]
fn empty_text_content_argument_stays_on_success_path() {
    let output = parse_with_items(&[text_command_item()], r"\text{}", false);

    assert!(output.diagnostics.is_empty());

    let result = output
        .result
        .as_ref()
        .expect("empty text content should stay on the success path");

    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };
    let text_args = match root_children.first() {
        Some(SyntaxNode::Command { name, args, .. }) => {
            assert_eq!(name, "text");
            args
        }
        other => panic!("expected first child to be \\text, got {:?}", other),
    };
    let text_arg = text_args[0]
        .as_ref()
        .expect("text should keep its argument slot");
    match &text_arg.value {
        ArgumentValue::TextContent(SyntaxNode::Group { children, .. }) => {
            assert!(children.is_empty(), "expected an empty content group");
        }
        other => panic!("expected empty text content group, got {:?}", other),
    }
}

#[test]
fn generic_only_content_error_in_strict_mode_keeps_inner_error() {
    let output = parse_with_items(&[text_command_item()], r"\text{$x}", true);

    assert!(
        output.result.is_none(),
        "strict mode should not keep a partial result"
    );
    assert_eq!(
        collect_messages(&output),
        vec!["found '$' expected something else, or end of input"]
    );
    assert!(
        !output.diagnostics[0].expected.is_empty(),
        "strict generic-only diagnostic should keep structured expected values"
    );
}

#[test]
fn strict_content_command_error_keeps_inner_span() {
    let output = parse_with_items(
        &[text_command_item(), frac_command_item()],
        r"\text{\frac{a}{b}}",
        true,
    );

    assert_first_diagnostic_span_eq(&output, r"\text{\frac{a}{b}}", r"\frac");
}

#[test]
fn strict_content_scripted_error_keeps_inner_span() {
    let output = parse_with_items(
        &[
            text_command_item(),
            command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m"),
            command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T"),
        ],
        r"\text{\underline{a^2}}",
        true,
    );

    assert_first_diagnostic_span_eq(&output, r"\text{\underline{a^2}}", "^");
}

#[test]
fn nonstrict_content_command_error_keeps_original_inner_span() {
    let src = r"\text{\frac{\frac}{b}}";
    let output = parse_with_items(&[text_command_item(), frac_command_item()], src, false);

    assert_eq!(
        collect_messages(&output),
        vec![r"Command \frac is not allowed in text mode"]
    );
    assert_first_diagnostic_span_eq(&output, src, r"\frac");

    let outer_frac_start = src.find(r"\frac{").expect("should find outer frac");
    assert_eq!(output.diagnostics[0].span.start, outer_frac_start);
}

#[test]
fn mode_error_for_math_only_environment_in_text() {
    let output = parse_with_items(
        &[text_command_item(), matrix_environment_item()],
        r"\text\begin{matrix}a\end{matrix}",
        true,
    );
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");
}

#[test]
fn diagnostics_serialize() {
    let output = parse_latex(r"\unknowncmd", true);
    let json = serde_json::to_value(&output).unwrap();
    let diagnostics = json.get("diagnostics").unwrap().as_array().unwrap();
    assert!(!diagnostics.is_empty());
    let diagnostic = &diagnostics[0];
    assert!(diagnostic.get("message").is_some());
    assert!(diagnostic.get("span").is_some());
    assert!(diagnostic.get("expected").is_some());
}

#[test]
fn diagnostics_serialize_includes_contexts_field() {
    let output = parse_latex(r"\unknowncmd", true);
    let json = serde_json::to_value(&output).unwrap();
    let diagnostics = json.get("diagnostics").unwrap().as_array().unwrap();
    assert!(!diagnostics.is_empty());
    let diagnostic = &diagnostics[0];
    assert!(diagnostic.get("message").is_some());
    assert!(diagnostic.get("span").is_some());
    assert!(diagnostic.get("expected").is_some());
    assert!(diagnostic.get("contexts").is_some());
}

#[test]
fn invalid_left_delimiter_reports_root_cause_and_contexts() {
    let output = parse_latex(r"\begin{aligned}\left\foo x \right)\end{aligned}", false);
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");

    let diagnostic = &output.diagnostics[0];
    assert_eq!(diagnostic.message, "invalid \\left delimiter");

    let labels: Vec<&str> = diagnostic
        .contexts
        .iter()
        .map(|context| context.label.as_str())
        .collect();
    assert!(labels.contains(&"left-delimited group"));
    assert!(labels.contains(&"environment body"));
}

#[test]
fn invalid_left_delimiter_reports_bare_left_context_only() {
    let output = parse_latex(r"\left\foo x \right)", false);
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");

    let diagnostic = &output.diagnostics[0];
    assert_eq!(diagnostic.message, "invalid \\left delimiter");

    let labels: Vec<&str> = diagnostic
        .contexts
        .iter()
        .map(|context| context.label.as_str())
        .collect();
    assert!(labels.contains(&"left-delimited group"));
    assert!(!labels.contains(&"environment body"));
}

#[test]
fn partial_result_keeps_outer_delimited_group_and_following_siblings() {
    let output = parse_latex(r"\left( \begin{matrix} a \end{align} \right) + z", false);
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");

    let result = output
        .result
        .as_ref()
        .expect("should produce a partial result");

    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };

    let delimited_children = match root_children.first() {
        Some(SyntaxNode::Group {
            kind: texform_interface::syntax_node::GroupKind::Delimited { .. },
            children,
            ..
        }) => children,
        other => panic!(
            "expected first child to be a delimited group, got {:?}",
            other
        ),
    };

    assert!(
        delimited_children
            .iter()
            .any(|child| matches!(child, SyntaxNode::Error { .. })),
        "delimited group should keep an error placeholder"
    );
    assert!(
        root_children
            .iter()
            .any(|child| matches!(child, SyntaxNode::Char('+'))),
        "following siblings should still contain '+'"
    );
    assert!(
        root_children
            .iter()
            .any(|child| matches!(child, SyntaxNode::Char('z'))),
        "following siblings should still contain 'z'"
    );
}

#[test]
fn partial_result_json_contains_error_node() {
    let output = parse_latex(r"\left( \begin{matrix} a \end{align} \right) + z", false);
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");

    let json = serde_json::to_value(&output).expect("parse output should serialize to JSON");
    let node_json = json
        .get("result")
        .and_then(|result| result.get("node"))
        .expect("partial result JSON should contain result.node");
    let node_text = serde_json::to_string(node_json).expect("result.node JSON should stringify");

    assert!(
        node_text.contains("\"Error\""),
        "result.node JSON should expose the recovered Error node: {node_text}"
    );
    assert!(
        node_text.contains("Environment name mismatch")
            && node_text.contains("\\end{matrix}")
            && node_text.contains("\\end{align}"),
        "result.node JSON should preserve the normalized recovered environment mismatch message: {node_text}"
    );
}

#[test]
fn partial_result_keeps_outer_environment_on_inner_environment_error() {
    let src = r"\begin{matrix} \begin{align} x \end{matrix}";
    let output = parse_latex(src, false);
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");
    assert_eq!(
        output.diagnostics[0].message,
        "Environment name mismatch: expected \\end{align}, found \\end{matrix}",
        "inner environment mismatch should stay more specific than an outer missing-end error"
    );
    assert_first_diagnostic_span_eq(&output, src, r"\end{matrix}");
    assert_first_diagnostic_expected_found(&output, &[r"\end{align}"], Some(r"\end{matrix}"));

    let result = output
        .result
        .as_ref()
        .expect("should produce a partial result");

    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };

    let body_children = match root_children.first() {
        Some(SyntaxNode::Environment { name, body, .. }) => {
            assert_eq!(name, "matrix");
            match body.as_ref() {
                SyntaxNode::Group { children, .. } => children,
                other => panic!("expected matrix body group, got {:?}", other),
            }
        }
        other => panic!("expected first child to be an environment, got {:?}", other),
    };

    assert!(
        body_children
            .iter()
            .any(|child| matches!(child, SyntaxNode::Error { .. })),
        "matrix body should keep an error placeholder for the broken inner environment"
    );
}

#[test]
fn partial_result_keeps_following_siblings_after_environment_mismatch() {
    let src = r"\begin{matrix} x \end{align} + z";
    let output = parse_latex(src, false);
    assert!(!output.diagnostics.is_empty(), "should have diagnostics");
    assert_eq!(
        output.diagnostics[0].message,
        "Environment name mismatch: expected \\end{matrix}, found \\end{align}"
    );
    assert_first_diagnostic_span_eq(&output, src, r"\end{align}");
    assert_first_diagnostic_expected_found(&output, &[r"\end{matrix}"], Some(r"\end{align}"));

    let result = output
        .result
        .as_ref()
        .expect("should produce a partial result");

    let root_children = match &result.node {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {:?}", other),
    };

    assert!(
        root_children
            .iter()
            .any(|child| matches!(child, SyntaxNode::Error { .. })),
        "root should keep an error placeholder for the broken environment"
    );
    assert!(
        root_children
            .iter()
            .any(|child| matches!(child, SyntaxNode::Char('+'))),
        "following siblings should still contain '+'"
    );
    assert!(
        root_children
            .iter()
            .any(|child| matches!(child, SyntaxNode::Char('z'))),
        "following siblings should still contain 'z'"
    );
}

#[test]
fn environment_mismatch_rewrite_does_not_capture_later_generic_errors() {
    let src = r"\begin{matrix} x \end{align}}";
    let output = parse_latex(src, false);
    assert_eq!(
        output.diagnostics.len(),
        2,
        "should keep two distinct diagnostics"
    );

    let mismatch = &output.diagnostics[0];
    assert_eq!(
        mismatch.message,
        "Environment name mismatch: expected \\end{matrix}, found \\end{align}"
    );
    assert_eq!(&src[mismatch.span.start..mismatch.span.end], r"\end{align}");

    let trailing_brace = &output.diagnostics[1];
    assert_eq!(
        trailing_brace.message,
        "found '}' expected something else, or end of input"
    );
    assert_eq!(
        &src[trailing_brace.span.start..trailing_brace.span.end],
        "}"
    );
    assert_eq!(trailing_brace.expected, ["something else", "end of input"]);
    assert_eq!(trailing_brace.found.as_deref(), Some("}"));
}

#[test]
#[should_panic(expected = "cannot serialize syntax tree containing Error node")]
fn serialize_latex_rejects_error_nodes() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![SyntaxNode::Error {
            message: "invalid \\left delimiter".to_string(),
            snippet: "\\left\\foo x \\right)".to_string(),
        }],
    );

    let _ = serialize_latex(&node);
}

#[test]
#[should_panic(expected = "cannot serialize syntax tree containing Error node")]
fn serialize_latex_rejects_nested_error_nodes() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![SyntaxNode::implicit_group(
            ContentMode::Math,
            vec![SyntaxNode::Error {
                message: "invalid \\left delimiter".to_string(),
                snippet: "\\left\\foo x \\right)".to_string(),
            }],
        )],
    );

    let _ = serialize_latex(&node);
}

#[test]
#[should_panic(expected = "serialize_latex expects SyntaxNode::Root")]
fn serialize_latex_rejects_non_root_top_level_node() {
    let node = SyntaxNode::implicit_group(ContentMode::Math, vec![SyntaxNode::Char('x')]);
    let _ = serialize_latex(&node);
}

#[test]
#[should_panic(expected = "serialize_latex expects SyntaxNode::Root")]
fn serialize_latex_with_rejects_non_root_top_level_node() {
    let node = SyntaxNode::implicit_group(ContentMode::Math, vec![SyntaxNode::Char('x')]);
    let _ = serialize_latex_with(&node, &SerializeOptions::default());
}

#[test]
#[should_panic(expected = "serialize_latex does not accept nested SyntaxNode::Root")]
fn serialize_latex_rejects_nested_root_nodes() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![SyntaxNode::root(
            ContentMode::Math,
            vec![SyntaxNode::Char('x')],
        )],
    );

    let _ = serialize_latex(&node);
}

#[test]
#[should_panic(expected = "serialize_latex does not accept nested SyntaxNode::Root")]
fn serialize_latex_with_rejects_nested_root_nodes() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![SyntaxNode::root(
            ContentMode::Math,
            vec![SyntaxNode::Char('x')],
        )],
    );

    let _ = serialize_latex_with(&node, &SerializeOptions::default());
}

#[test]
fn serialize_latex_accepts_syntax_root() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![
            SyntaxNode::Char('a'),
            SyntaxNode::Char('+'),
            SyntaxNode::Char('b'),
        ],
    );

    assert_eq!(serialize_latex(&node), "a + b");
}

#[test]
fn parse_with_context_items_command_target() {
    let output = parse_with_context_items(
        &[command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        )],
        &[r"\probe{a}"],
        None,
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.result.is_some(),
        "command target should parse"
    );
    assert!(
        output[0].output.diagnostics.is_empty(),
        "no diagnostics expected"
    );
}

#[test]
fn parse_with_context_items_environment_target() {
    let output = parse_with_context_items(
        &[environment_item(
            "probeenv",
            AllowedMode::Math,
            ContentMode::Math,
            "",
        )],
        &[r"\begin{probeenv}a\end{probeenv}"],
        None,
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.result.is_some(),
        "environment target should parse"
    );
    assert!(
        output[0].output.diagnostics.is_empty(),
        "no diagnostics expected"
    );
}

#[test]
fn parse_with_context_items_reports_invalid_spec() {
    let output = parse_with_context_items(
        &[command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "s:T",
        )],
        &[r"\probe", r"\probe*"],
        None,
        true,
    );
    assert_eq!(output.len(), 2);
    assert!(
        output[0].output.diagnostics[0]
            .message
            .contains("spec validation failed"),
        "unexpected diagnostic: {}",
        output[0].output.diagnostics[0].message
    );
}

#[test]
fn parse_with_context_items_defaults_to_core_only_context() {
    let output = parse_with_context_items(
        &[command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        )],
        &[r"\probe{\text{a}}"],
        None,
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        !output[0].output.diagnostics.is_empty(),
        "core-only default should not enable \\text"
    );
}

#[test]
fn parse_with_context_items_supports_explicit_text_command() {
    let output = parse_with_context_items(
        &[
            command_item("probe", CommandKind::Prefix, AllowedMode::Math, "m"),
            text_command_item(),
        ],
        &[r"\probe{\text{a}}"],
        None,
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.result.is_some(),
        "explicit text command should enable \\text"
    );
    assert!(
        output[0].output.diagnostics.is_empty(),
        "no diagnostics expected when text is injected"
    );
}

#[test]
fn parse_with_context_items_supports_explicit_control_delimiter_args() {
    let output = parse_with_context_items(
        &[
            command_item("probe", CommandKind::Prefix, AllowedMode::Math, "m:D"),
            delimiter_control_item("langle"),
            delimiter_control_item("rangle"),
            delimiter_control_item("|"),
        ],
        &[r"\probe\langle", r"\probe\rangle", r"\probe\|"],
        None,
        true,
    );
    assert_eq!(output.len(), 3);

    let expected = [
        Delimiter::Control("langle"),
        Delimiter::Control("rangle"),
        Delimiter::Control("|"),
    ];

    for (item, expected_delimiter) in output.iter().zip(expected) {
        assert!(
            item.output.diagnostics.is_empty(),
            "unexpected diagnostics for {}: {:?}",
            item.input,
            item.output.diagnostics
        );

        let result = item
            .output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result for {}", item.input));

        match &result.node {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => match &args[0]
                    .as_ref()
                    .unwrap_or_else(|| panic!("expected argument for {}", item.input))
                    .value
                {
                    ArgumentValue::Delimiter(value) => {
                        assert_eq!(*value, expected_delimiter);
                    }
                    other => panic!(
                        "expected delimiter argument for {}, got {:?}",
                        item.input, other
                    ),
                },
                other => panic!("expected command node for {}, got {:?}", item.input, other),
            },
            other => panic!("expected root node for {}, got {:?}", item.input, other),
        }
    }
}

#[test]
fn parse_with_context_items_supports_runtime_delimiter_controls() {
    let output = parse_with_context_items(
        &[
            delimiter_control_item("langle"),
            delimiter_control_item("rangle"),
        ],
        &[r"\left\langle x\right\rangle"],
        Some(&[]),
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output[0].output.diagnostics
    );

    let result = output[0]
        .output
        .result
        .as_ref()
        .expect("runtime delimiter controls should parse");

    match &result.node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Group { kind, .. } => match kind {
                texform_interface::syntax_node::GroupKind::Delimited { left, right } => {
                    assert_eq!(*left, Delimiter::Control("langle"));
                    assert_eq!(*right, Delimiter::Control("rangle"));
                }
                other => panic!("expected delimited group, got {:?}", other),
            },
            other => panic!("expected child group, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

#[test]
fn parse_with_context_items_supports_nullable_delimiter_arguments() {
    let output = parse_with_context_items(
        &[command_item(
            "genfracprobe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:D? m:D? m m m m",
        )],
        &[
            r"\genfracprobe{}{}{0}{1}{a}{b}",
            r"\genfracprobe{(}{)}{0}{1}{a}{b}",
        ],
        None,
        true,
    );
    assert_eq!(output.len(), 2);

    let expected = [
        [Delimiter::None, Delimiter::None],
        [Delimiter::Char('('), Delimiter::Char(')')],
    ];

    for (item, expected_pair) in output.iter().zip(expected) {
        assert!(
            item.output.diagnostics.is_empty(),
            "unexpected diagnostics for {}: {:?}",
            item.input,
            item.output.diagnostics
        );
        let result = item
            .output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result for {}", item.input));

        match &result.node {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => {
                    for (slot, expected_delimiter) in args.iter().take(2).zip(expected_pair) {
                        match &slot
                            .as_ref()
                            .unwrap_or_else(|| panic!("expected argument for {}", item.input))
                            .value
                        {
                            ArgumentValue::Delimiter(value) => {
                                assert_eq!(*value, expected_delimiter);
                            }
                            other => panic!(
                                "expected delimiter argument for {}, got {:?}",
                                item.input, other
                            ),
                        }
                    }
                }
                other => panic!("expected command node for {}, got {:?}", item.input, other),
            },
            other => panic!("expected root node for {}, got {:?}", item.input, other),
        }
    }
}

#[test]
fn parse_with_context_items_can_use_empty_package_list() {
    let output = parse_with_context_items(
        &[command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        )],
        &[r"\probe{\text{a}}"],
        Some(&[]),
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        !output[0].output.diagnostics.is_empty(),
        "\\text should fail when the caller explicitly requests a core-only knowledge base"
    );
}

#[test]
fn parse_with_context_items_can_load_explicit_packages() {
    let output = parse_with_context_items(
        &[command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        )],
        &[r"\probe{\arccos}"],
        Some(&["base"]),
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.result.is_some(),
        "explicit base package should enable \\arccos"
    );
    assert!(
        output[0].output.diagnostics.is_empty(),
        "no diagnostics expected when base is loaded"
    );
}

#[test]
fn parse_with_context_items_uses_public_package_loading_order() {
    let output = parse_with_context_items(&[], &[r"\div{a}"], Some(&["physics", "base"]), true);
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output[0].output.diagnostics
    );

    let result = output[0]
        .output
        .result
        .as_ref()
        .expect("expected parse result for canonicalized package load");

    match &result.node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "div");
                assert_eq!(
                    args.len(),
                    1,
                    "canonical package loading should keep the physics explicit command active"
                );
            }
            other => panic!("expected command node, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

#[test]
fn parse_with_context_items_reports_unknown_package() {
    let output = parse_with_context_items(
        &[command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        )],
        &[r"\probe{a}"],
        Some(&["missing-package"]),
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.diagnostics[0]
            .message
            .contains("package loading failed"),
        "unexpected diagnostic: {}",
        output[0].output.diagnostics[0].message
    );
}

#[test]
fn parse_with_context_items_multiple_specs() {
    let output = parse_with_context_items(
        &[
            command_item("foo", CommandKind::Prefix, AllowedMode::Math, "m"),
            environment_item("bar", AllowedMode::Math, ContentMode::Math, ""),
        ],
        &[r"\foo{\begin{bar}x\end{bar}}"],
        None,
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(output[0].output.result.is_some(), "multi-spec should parse");
    assert!(
        output[0].output.diagnostics.is_empty(),
        "no diagnostics expected"
    );
}

#[test]
fn parse_with_context_items_duplicate_name_rejected() {
    let output = parse_with_context_items(
        &[
            command_item("foo", CommandKind::Prefix, AllowedMode::Math, "m"),
            command_item("foo", CommandKind::Prefix, AllowedMode::Math, "o m"),
        ],
        &[r"\foo{x}"],
        None,
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.diagnostics[0]
            .message
            .contains("duplicate command name: foo"),
        "unexpected diagnostic: {}",
        output[0].output.diagnostics[0].message
    );
}

#[test]
fn parse_with_context_items_duplicate_delimiter_rejected() {
    let output = parse_with_context_items(
        &[
            delimiter_control_item("langle"),
            delimiter_control_item("langle"),
        ],
        &[r"\left\langle x\right\rangle"],
        Some(&[]),
        true,
    );
    assert_eq!(output.len(), 1);
    assert!(
        output[0].output.diagnostics[0]
            .message
            .contains("duplicate delimiter control name: langle"),
        "unexpected diagnostic: {}",
        output[0].output.diagnostics[0].message
    );
}
