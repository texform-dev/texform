//! End-to-end behaviour of the LowerDeclarative phase.
//!
//! Tests go through the public `transform_ast` entry point with the standard
//! rule class, so they implicitly check that LowerDeclarative composes
//! cleanly with the existing ApplyRules / Cleanup phases.

use texform_core::ast::{Ast, ContentMode, Node};
use texform_core::parse::ParseContext;
use texform_core::serialize::serialize;
use texform_core::transform::{
    Attr, AttrValue, LowerDeclarativeReport, RuleClass, SizeValue, TransformContextBuilder,
    transform_ast,
};

struct Outcome {
    text: String,
    ast: Ast,
    report: LowerDeclarativeReport,
}

fn run(src: &str) -> Outcome {
    let parse_ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let mut ast = parse_ctx
        .parse_to_ast(src, false)
        .expect("source should parse");
    let transform_ctx = TransformContextBuilder::from_classes(&[RuleClass::Standard])
        .build_with(&parse_ctx)
        .expect("transform context should build");
    let report =
        transform_ast(&mut ast, &parse_ctx, &transform_ctx).expect("transform should succeed");
    ast.assert_invariants();
    Outcome {
        text: serialize(&ast),
        report: report.lower_declarative,
        ast,
    }
}

fn transform(src: &str) -> String {
    run(src).text
}

#[test]
fn lowers_math_font_prefixes() {
    // Single-child segments stay wrapped in the original explicit group: the
    // phase only lowers the declarative; redundant brace stripping is a
    // separate (cleanup-time) concern.
    assert_eq!(transform(r"{\bf x}"), r"{ \mathbf { x } }");
    assert_eq!(transform(r"{\rm x}"), r"{ \mathrm { x } }");
    assert_eq!(transform(r"{\mit x}"), r"{ \symit { x } }");
}

#[test]
fn lowers_text_font_prefixes() {
    assert_eq!(transform(r"\text{{\bf x}}"), r"\text {\textbf{x}}");
    assert_eq!(transform(r"\text{{\rm x}}"), r"\text {\textrm{x}}");
}

#[test]
fn preserves_declaratives_without_prefix() {
    assert_eq!(transform(r"{\large x}"), r"{ \large x }");
    assert_eq!(transform(r"{\displaystyle x}"), r"{ \displaystyle x }");
    assert_eq!(transform(r"\text{{\cal x}}"), r"\text {\cal x}");
}

#[test]
fn splits_at_attribute_change_points() {
    assert_eq!(
        transform(r"{\bf x \rm y}"),
        r"{ \mathbf { x } \mathrm { y } }"
    );
}

#[test]
fn collapses_repeated_same_value_declarations() {
    assert_eq!(transform(r"{\large x \large y}"), r"{ \large x y }");
}

#[test]
fn handles_empty_segments() {
    assert_eq!(transform(r"{\bf}"), "{ }");
    assert_eq!(transform(r"{x \bf}"), "{ x }");
}

#[test]
fn leaves_unregistered_declaratives_unchanged() {
    assert_eq!(transform(r"{\foo x \bf y}"), r"{ \foo x \mathbf { y } }");
}

#[test]
fn handles_single_declarative_content_children() {
    assert_eq!(transform(r"\sqrt{\bf}"), r"\sqrt { }");
    assert_eq!(transform(r"\text{\bf}"), r"\text {}");
    assert_eq!(transform(r"x^{\bf}"), r"x ^ { }");
}

#[test]
fn later_changepoint_at_same_position_wins() {
    // `\rm` immediately overrides the preceding `\bf` so only the final
    // value applies to the segment.
    assert_eq!(transform(r"{\bf \rm x}"), r"{ \mathrm { x } }");
}

#[test]
fn multiple_attributes_wrap_in_fixed_order() {
    // font wraps innermost, size prepends as a declarative (no prefix
    // equivalent), so the size declarative ends up outside the mathbf wrap.
    assert_eq!(transform(r"{\bf \large x}"), r"{ \large \mathbf { x } }");
}

#[test]
fn report_records_dropped_and_collapsed_counts() {
    let outcome = run(r"{\large x \large y} {x \bf} \sqrt{\bf}");

    assert_eq!(outcome.text, r"{ \large x y } { x } \sqrt { }");
    assert_eq!(outcome.report.dropped.get("large"), Some(&2));
    assert_eq!(outcome.report.collapsed.get("large"), Some(&1));
    assert_eq!(outcome.report.dropped.get("bf"), Some(&2));
    assert_eq!(outcome.report.eliminated_empty_segments, 2);
    assert_eq!(
        outcome.report.reinserted.get(&(
            Attr::Size,
            AttrValue::Size(SizeValue(120)),
            ContentMode::Math,
        )),
        Some(&1)
    );
    assert!(
        !outcome.report.wrapped.contains_key(&(
            Attr::Font,
            AttrValue::Font("VARIANT.BOLD"),
            ContentMode::Math,
        )),
        "no segment was wrapped: the only \\bf occurrences are trailing/empty",
    );
}

#[test]
fn single_content_child_removes_old_declarative_subtree() {
    let outcome = run(r"\sqrt{\bf}");

    assert_eq!(outcome.report.dropped.get("bf"), Some(&1));
    assert_eq!(outcome.report.eliminated_empty_segments, 1);
    assert!(
        outcome
            .ast
            .find_all(outcome.ast.root(), |node| matches!(
                node,
                Node::Declarative { name, .. } if name == "bf"
            ))
            .is_empty(),
        "the dropped declarative must be removed from the AST",
    );
}

#[test]
fn container_drop_removes_old_declarative_subtrees() {
    let outcome = run(r"{\bf \rm x \rm y}");

    assert_eq!(outcome.report.dropped.get("bf"), Some(&1));
    assert_eq!(outcome.report.dropped.get("rm"), Some(&2));
    assert_eq!(outcome.report.collapsed.get("rm"), Some(&1));
    assert!(
        outcome
            .ast
            .find_all(outcome.ast.root(), |node| matches!(
                node,
                Node::Declarative { name, .. } if name == "bf" || name == "rm"
            ))
            .is_empty(),
        "all dropped declaratives must be removed from the AST",
    );
}
