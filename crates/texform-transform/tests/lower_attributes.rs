//! End-to-end behavior of the LowerAttributes phase.
//!
//! Tests go through the public `run` entry point with the standard
//! rule class, so they implicitly check that LowerAttributes composes cleanly
//! with the Rewrite phase.

use texform_core::ast::Ast;
use texform_core::parse::ParseContext;
use texform_core::serialize::serialize;
use texform_transform::{RuleClass, RuleClassSet, TransformConfig, run as transform};

struct Outcome {
    text: String,
    ast: Ast,
}

fn run_with_packages(src: &str, packages: &[&str]) -> Outcome {
    run_with_packages_and_classes(src, packages, &[RuleClass::Standard])
}

fn run_with_packages_and_classes(src: &str, packages: &[&str], classes: &[RuleClass]) -> Outcome {
    let parse_ctx = ParseContext::from_packages(packages);
    let mut ast = parse_ctx
        .parse_to_ast(src, &texform_core::parse::ParseConfig::default())
        .expect("source should parse");
    let mut config = TransformConfig::AUTHORING;
    config.rewrite.classes = classes
        .iter()
        .copied()
        .fold(RuleClassSet::empty(), |set, class| set | class.into());
    transform(&mut ast, &parse_ctx, &config).expect("transform should succeed");
    ast.assert_invariants();
    Outcome {
        text: serialize(&ast),
        ast,
    }
}

fn run(src: &str) -> Outcome {
    run_with_packages(src, &["base", "textmacros"])
}

fn serialized_with_packages(src: &str, expected: &str, packages: &[&str]) {
    let actual = run_with_packages(src, packages);
    actual.ast.assert_invariants();

    let parse_ctx = ParseContext::from_packages(packages);
    let expected_ast = parse_ctx
        .parse_to_ast(expected, &texform_core::parse::ParseConfig::default())
        .expect("expected output should parse");
    expected_ast.assert_invariants();

    assert_eq!(actual.text, serialize(&expected_ast));
}

fn serialized(src: &str, expected: &str) {
    serialized_with_packages(src, expected, &["base", "textmacros"]);
}

fn serialized_text(src: &str, expected: &str) {
    assert_eq!(run(src).text, expected);
}

#[test]
fn consumes_explicit_groups_that_scope_declaratives() {
    serialized(r"{\bf x}y", r"\mathbf{x}y");
    serialized(r"{\bf \rm x}", r"\mathrm{x}");
    serialized_text(r"{\bf {\bf x}}", r"\mathbf { x }");
}

#[test]
fn flattens_structural_explicit_groups_after_lowering() {
    serialized_text(r"\mathbf{{x}}", r"\mathbf { x }");
    serialized_text(r"\mathbf{{\mathbf{x}}}", r"\mathbf { x }");
    serialized(r"{{\bf x}}", r"{\mathbf{x}}");
}

#[test]
fn absorbs_nested_math_prefix_wrappers() {
    serialized(r"\mathbf{\mathbf{x}}", r"\mathbf{x}");
    serialized(r"\mathbf{\mathit{x}}", r"\mathit{x}");
    serialized(r"\mathbf{\mathit{x}y}", r"\mathit{x}\mathbf{y}");
    serialized(r"\mathbf{\mathrm{a\mathbf{b}}}", r"\mathrm{a}\mathbf{b}");
}

#[test]
fn falls_back_to_preserving_noop_declarative_groups() {
    serialized(r"{x \bf}", r"x");
    serialized_text(r"\mathbf{{\bf x}}", r"\mathbf { x }");
    serialized_text(r"{\bf}", r"{ }");
    serialized_text(r"{\bf \rm}", r"{ }");
}

#[test]
fn preserves_text_multi_axis_combinations() {
    serialized(r"\text{\textbf{\textit{x}}}", r"\text{\textbf{\textit{x}}}");
    serialized(r"\text{\textbf{\textbf{x}}}", r"\text{\textbf{x}}");
    serialized(r"\text{\textsf{\textrm{x}}}", r"\text{\textrm{x}}");
}

#[test]
fn isolates_math_and_text_attribute_state() {
    serialized(
        r"{\bf \text{\textit{y}}} z",
        r"\mathbf{\text{\textit{y}}} z",
    );
}

#[test]
fn collapses_size_and_orders_mixed_math_axes() {
    serialized(r"\mathbf{\Huge \Huge x}", r"\mathbf{\Huge x}");
    serialized(r"\mathbf{\large \Huge x}", r"\mathbf{\Huge x}");
    serialized(
        r"\mathbf{\large \scriptstyle x}",
        r"\scriptstyle \large \mathbf{x}",
    );
}

#[test]
fn lower_attributes_is_idempotent_on_serialized_output() {
    for src in [
        r"{\bf {\bf x}}",
        r"\mathbf{\mathit{x}y}",
        r"\text{\textbf{\textit{x}}}",
        r"\mathbf{{\mathbf{x}}}",
    ] {
        let once = run(src).text;
        let twice = run(&once).text;
        assert_eq!(
            twice, once,
            "LowerAttributes should be idempotent for {src}"
        );
    }
}

#[test]
fn post_pass_normalizes_prefixes_created_by_apply_rules() {
    let actual = run_with_packages_and_classes(
        r"\vb{\rm x}",
        &["base", "textmacros", "physics", "boldsymbol"],
        &[RuleClass::Standard, RuleClass::Expand],
    );
    assert_eq!(actual.text, r"\mathrm { x }");
}
