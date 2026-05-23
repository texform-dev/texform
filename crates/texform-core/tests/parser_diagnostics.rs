mod support;

use std::time::{Duration, Instant};

use support::{
    assert_first_diagnostic_span_eq, collect_messages, command_item, contains_command_named,
    contains_error_node, parse_with_items,
};
use texform_core::parse::{
    AllowedMode, CommandKind, ContextItem, ParseConfig, ParseDiagnosticKind, ParseOutput, Parser,
};
use texform_interface::syntax_node::{ArgumentValue, SyntaxNode};

fn parse_shared(src: &str, config: &ParseConfig) -> ParseOutput {
    Parser::shared().parse(src, config)
}

fn text_command_item() -> ContextItem {
    command_item("text", CommandKind::Prefix, AllowedMode::Math, "m:T")
}

fn frac_command_item() -> ContextItem {
    command_item("frac", CommandKind::Prefix, AllowedMode::Math, "m m")
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
fn recover_false_keeps_nonstrict_unknowns_without_partial_recovery() {
    let config = ParseConfig::NONSTRICT_NO_RECOVER;
    let output = parse_shared(r"\unknowncmd {", &config);

    assert!(
        output.result.is_none(),
        "recover=false should not keep a partial tree for malformed input"
    );
    assert_eq!(collect_messages(&output), vec!["not a command"]);
}

#[test]
fn nonstrict_recover_handles_unclosed_nested_groups_without_exponential_retry() {
    let src = format!("{}x", "{".repeat(18));

    let started = Instant::now();
    let output = parse_shared(&src, &ParseConfig::NONSTRICT_RECOVER);
    let elapsed = started.elapsed();

    assert!(
        output.result.is_some(),
        "recovery should keep a partial tree"
    );
    assert!(
        !output.diagnostics.is_empty(),
        "unclosed groups should report diagnostics"
    );
    assert!(
        elapsed < Duration::from_millis(250),
        "recover=true should not retry exponentially for unclosed nested groups; elapsed={elapsed:?}"
    );
}

#[test]
fn max_group_depth_exceeded_reports_public_kind() {
    let config = ParseConfig {
        max_group_depth: 1,
        ..ParseConfig::NONSTRICT_RECOVER
    };
    let output = parse_shared("{{x}}", &config);

    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == Some(ParseDiagnosticKind::MaxGroupDepthExceeded)),
        "expected max depth diagnostic, got {:?}",
        output.diagnostics
    );
    let result = output.result.expect("max depth should keep an error node");
    assert!(
        contains_error_node(&result.node),
        "max depth should produce an Error node"
    );
}

#[test]
fn max_group_depth_restored_between_sibling_groups() {
    // Sequential sibling groups each enter and exit a fresh scope; the depth
    // counter must be restored on group exit so that with max_group_depth = 2
    // a string of single-level groups parses cleanly.  Regression guard for
    // ParserState::enter_group's RAII drop.
    let config = ParseConfig {
        max_group_depth: 2,
        ..ParseConfig::NONSTRICT_RECOVER
    };
    let output = parse_shared("{a}{b}{c}", &config);
    assert!(
        output.diagnostics.is_empty(),
        "depth must be restored between sibling groups, got {:?}",
        output.diagnostics
    );
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
fn diagnostics_serialize_includes_contexts_field() {
    let output = parse_shared(
        r"\unknowncmd",
        &texform_core::parse::ParseConfig::STRICT_NO_RECOVER,
    );
    let json = serde_json::to_value(&output).unwrap();
    let diagnostics = json.get("diagnostics").unwrap().as_array().unwrap();
    assert!(!diagnostics.is_empty());
    let diagnostic = &diagnostics[0];
    assert!(diagnostic.get("message").is_some());
    assert!(diagnostic.get("span").is_some());
    assert!(diagnostic.get("expected").is_some());
    assert!(diagnostic.get("contexts").is_some());
    assert!(diagnostic.get("kind").is_some());
}

#[test]
fn invalid_left_delimiter_reports_root_cause_and_contexts() {
    let output = parse_shared(
        r"\begin{aligned}\left\foo x \right)\end{aligned}",
        &texform_core::parse::ParseConfig::default(),
    );
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
    let output = parse_shared(
        r"\left\foo x \right)",
        &texform_core::parse::ParseConfig::default(),
    );
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
    let output = parse_shared(
        r"\left( \begin{matrix} a \end{align} \right) + z",
        &texform_core::parse::ParseConfig::default(),
    );
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
    let output = parse_shared(
        r"\left( \begin{matrix} a \end{align} \right) + z",
        &texform_core::parse::ParseConfig::default(),
    );
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
    let output = parse_shared(src, &texform_core::parse::ParseConfig::default());
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
    let output = parse_shared(src, &texform_core::parse::ParseConfig::default());
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
    let output = parse_shared(src, &texform_core::parse::ParseConfig::default());
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
