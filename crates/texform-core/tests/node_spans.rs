use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseResult, Parser, ParserBuilder, Span,
};
use texform_interface::syntax_node::{ContentMode, SyntaxNode};

fn parse_ok(src: &str) -> ParseResult {
    let output = Parser::shared().parse(src, &texform_core::parse::ParseConfig::default());
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    output.result.expect("expected parse result")
}

fn parse_ok_with_items(items: &[ContextItem], src: &str) -> ParseResult {
    let mut builder = ParserBuilder::empty();
    for item in items {
        builder = builder.insert_item(item.clone());
    }

    let output = builder
        .build()
        .expect("context items should be valid")
        .parse(src, &texform_core::parse::ParseConfig::default());
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    output.result.expect("expected parse result")
}

fn span_ids(result: &ParseResult) -> Vec<&str> {
    result
        .node_spans
        .iter()
        .map(|entry| entry.id.as_str())
        .collect()
}

fn assert_root_span_covers_source(result: &ParseResult, src: &str) {
    assert_eq!(
        result.span,
        Span {
            start: 0,
            end: src.len()
        }
    );
    assert_eq!(
        result.span_for("root"),
        Some(&Span {
            start: 0,
            end: src.len(),
        })
    );
}

#[test]
fn parse_result_root_span_covers_smoke_cases() {
    for src in [
        "",
        "abc",
        r"\frac{a}{b}",
        r"\sqrt[3]{x}",
        r" a + b ",
        r"\begin{matrix}x\end{matrix}",
        "x^{2}_{i}",
    ] {
        assert_root_span_covers_source(&parse_ok(src), src);
    }
}

#[test]
fn parse_result_exposes_root_node_span() {
    let result = parse_ok("x");

    assert_eq!(result.span, Span { start: 0, end: 1 });
    assert_eq!(span_ids(&result), vec!["root", "root.child.0"]);
    assert_eq!(result.span_for("root"), Some(&Span { start: 0, end: 1 }));
    assert_eq!(
        result.span_for("root.child.0"),
        Some(&Span { start: 0, end: 1 })
    );
}

#[test]
fn parse_result_top_level_node_is_root() {
    // The root path is backed by a real `SyntaxNode::Root`, not a synthetic
    // prefix over an implicit group.
    let result = parse_ok("x");

    match result.node {
        SyntaxNode::Root { mode, children } => {
            assert_eq!(mode, ContentMode::Math);
            assert_eq!(children.len(), 1);
        }
        other => panic!(
            "expected parse root to be SyntaxNode::Root, got {:?}",
            other
        ),
    }
}

#[test]
fn frac_records_command_argument_and_content_paths() {
    let result = parse_ok(r"\frac{a}{bc}");

    assert_eq!(
        span_ids(&result),
        vec![
            "root",
            "root.child.0",
            "root.child.0.arg.0",
            "root.child.0.arg.0.content",
            "root.child.0.arg.1",
            "root.child.0.arg.1.content",
            "root.child.0.arg.1.content.child.0",
            "root.child.0.arg.1.content.child.1",
        ]
    );

    assert_eq!(
        result.span_for("root.child.0"),
        Some(&Span { start: 0, end: 12 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0"),
        Some(&Span { start: 5, end: 8 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content"),
        Some(&Span { start: 6, end: 7 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1"),
        Some(&Span { start: 8, end: 12 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content"),
        Some(&Span { start: 9, end: 11 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content.child.0"),
        Some(&Span { start: 9, end: 10 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content.child.1"),
        Some(&Span { start: 10, end: 11 })
    );
}

#[test]
fn scripted_records_base_sub_sup_in_preorder() {
    let result = parse_ok("x_i^2");

    assert_eq!(
        span_ids(&result),
        vec![
            "root",
            "root.child.0",
            "root.child.0.base",
            "root.child.0.sub",
            "root.child.0.sup",
        ]
    );

    assert_eq!(
        result.span_for("root.child.0"),
        Some(&Span { start: 0, end: 5 })
    );
    assert_eq!(
        result.span_for("root.child.0.base"),
        Some(&Span { start: 0, end: 1 })
    );
    assert_eq!(
        result.span_for("root.child.0.sub"),
        Some(&Span { start: 1, end: 3 })
    );
    assert_eq!(
        result.span_for("root.child.0.sup"),
        Some(&Span { start: 3, end: 5 })
    );
}

#[test]
fn scripted_preserves_nested_records_under_sub() {
    let result = parse_ok(r"x_{\frac{a}{b}}");

    assert_eq!(
        span_ids(&result),
        vec![
            "root",
            "root.child.0",
            "root.child.0.base",
            "root.child.0.sub",
            "root.child.0.sub.child.0",
            "root.child.0.sub.child.0.arg.0",
            "root.child.0.sub.child.0.arg.0.content",
            "root.child.0.sub.child.0.arg.1",
            "root.child.0.sub.child.0.arg.1.content",
        ]
    );

    assert_eq!(
        result.span_for("root.child.0.base"),
        Some(&Span { start: 0, end: 1 })
    );
    assert_eq!(
        result.span_for("root.child.0.sub"),
        Some(&Span { start: 1, end: 15 })
    );
    assert_eq!(
        result.span_for("root.child.0.sub.child.0"),
        Some(&Span { start: 3, end: 14 })
    );
    assert_eq!(
        result.span_for("root.child.0.sub.child.0.arg.0.content"),
        Some(&Span { start: 9, end: 10 })
    );
    assert_eq!(
        result.span_for("root.child.0.sub.child.0.arg.1.content"),
        Some(&Span { start: 12, end: 13 })
    );
}

#[test]
fn unknown_environment_keeps_normal_body_path_in_nonstrict_mode() {
    let result = parse_ok(r"\begin{foo}a\end{foo}");

    assert_eq!(
        result.span_for("root.child.0"),
        Some(&Span { start: 0, end: 21 })
    );
    assert_eq!(
        result.span_for("root.child.0.body"),
        Some(&Span { start: 11, end: 12 })
    );
    assert_eq!(
        result.span_for("root.child.0.body.child.0"),
        Some(&Span { start: 11, end: 12 })
    );
}

#[test]
fn partial_parse_does_not_invent_missing_argument_paths() {
    let output = Parser::shared().parse(r"\frac{a", &texform_core::parse::ParseConfig::default());
    assert!(!output.diagnostics.is_empty());

    let result = output.result.expect("expected partial result");
    assert!(result.span_for("root").is_some());
    assert!(result.span_for("root.child.0.arg.1").is_none());
}

#[test]
fn shorthand_single_token_argument_shares_arg_and_content_span() {
    let result = parse_ok(r"\sqrt 2");

    assert_eq!(
        result.span_for("root.child.0.arg.1"),
        Some(&Span { start: 6, end: 7 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content"),
        Some(&Span { start: 6, end: 7 })
    );
}

#[test]
fn normalized_argument_content_preserves_nested_descendant_paths() {
    let result = parse_ok(r"\sqrt{\frac{a}{b}}");

    assert_eq!(
        result.span_for("root.child.0.arg.1.content"),
        Some(&Span { start: 6, end: 17 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content.arg.0"),
        Some(&Span { start: 11, end: 14 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content.arg.0.content"),
        Some(&Span { start: 12, end: 13 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content.arg.1.content"),
        Some(&Span { start: 15, end: 16 })
    );
}

#[test]
fn environment_records_argument_and_body_paths() {
    let result = parse_ok_with_items(
        &[EnvironmentItem::new("probeenv", AllowedMode::Math, ContentMode::Math, "o m").into()],
        r"\begin{probeenv}[x]{y}z\end{probeenv}",
    );

    assert_eq!(
        result.span_for("root.child.0.arg.0"),
        Some(&Span { start: 16, end: 19 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content"),
        Some(&Span { start: 17, end: 18 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1"),
        Some(&Span { start: 19, end: 22 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.1.content"),
        Some(&Span { start: 20, end: 21 })
    );
    assert_eq!(
        result.span_for("root.child.0.body"),
        Some(&Span { start: 22, end: 23 })
    );
}

#[test]
fn text_mode_argument_content_preserves_descendants_after_normalization() {
    let result = parse_ok(r"\text{ab$x$cd}");

    assert_eq!(
        result.span_for("root.child.0.arg.0.content"),
        Some(&Span { start: 6, end: 13 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.child.1"),
        Some(&Span { start: 8, end: 11 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.child.1.child.0"),
        Some(&Span { start: 9, end: 10 })
    );
}

#[test]
fn control_sequence_delimited_argument_uses_inner_content_span() {
    let result = parse_ok_with_items(
        &[
            CommandItem::new(
                "probe",
                CommandKind::Prefix,
                AllowedMode::Math,
                r"r<\langle,\rangle>",
            )
            .into(),
            DelimiterControlItem::new("langle").into(),
            DelimiterControlItem::new("rangle").into(),
        ],
        r"\probe\langle x\rangle",
    );

    assert_eq!(
        result.span_for("root.child.0.arg.0"),
        Some(&Span { start: 6, end: 22 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content"),
        Some(&Span { start: 13, end: 15 })
    );
}

#[test]
fn text_mode_environment_records_argument_and_body_paths() {
    let result = parse_ok_with_items(
        &[
            CommandItem::new("text", CommandKind::Prefix, AllowedMode::Math, "m:T").into(),
            EnvironmentItem::new("probeenv", AllowedMode::Text, ContentMode::Text, "o m").into(),
        ],
        r"\text{\begin{probeenv}[x]{y}z\end{probeenv}}",
    );

    // Single child: fold unwraps, so content points directly to the Environment node.
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.arg.0"),
        Some(&Span { start: 22, end: 25 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.arg.1.content"),
        Some(&Span { start: 26, end: 27 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.body"),
        Some(&Span { start: 28, end: 29 })
    );
}

#[test]
fn infix_records_left_and_right_descendant_paths() {
    let result = parse_ok(r"a+b \over c+d");

    assert_eq!(
        result.span_for("root.child.0.left"),
        Some(&Span { start: 0, end: 3 })
    );
    assert_eq!(
        result.span_for("root.child.0.left.child.0"),
        Some(&Span { start: 0, end: 1 })
    );
    assert_eq!(
        result.span_for("root.child.0.left.child.2"),
        Some(&Span { start: 2, end: 3 })
    );
    assert_eq!(
        result.span_for("root.child.0.right"),
        Some(&Span { start: 10, end: 13 })
    );
    assert_eq!(
        result.span_for("root.child.0.right.child.0"),
        Some(&Span { start: 10, end: 11 })
    );
    assert_eq!(
        result.span_for("root.child.0.right.child.2"),
        Some(&Span { start: 12, end: 13 })
    );
}

#[test]
fn text_declarative_records_flat_child_paths() {
    let result = parse_ok(r"\text{\bf ab$x$cd}");

    assert_eq!(result.span_for("root.child.0.arg.0.content.scope"), None);
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.child.0"),
        Some(&Span { start: 6, end: 9 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.child.1"),
        Some(&Span { start: 10, end: 12 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.child.2"),
        Some(&Span { start: 12, end: 15 })
    );
    assert_eq!(
        result.span_for("root.child.0.arg.0.content.child.3"),
        Some(&Span { start: 15, end: 17 })
    );
}

#[test]
fn baseless_script_records_zero_width_base_span() {
    let result = parse_ok("_a b");

    assert_eq!(
        result.span_for("root.child.0.base"),
        Some(&Span { start: 0, end: 0 })
    );
    assert_eq!(
        result.span_for("root.child.0.sub"),
        Some(&Span { start: 0, end: 2 })
    );
}
