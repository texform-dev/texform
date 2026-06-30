//! End-to-end behavior of the LowerAttributes phase.
//!
//! Tests go through the public `run` entry point with the standard
//! normalization level, so they implicitly check that LowerAttributes composes cleanly
//! with the Rewrite phase.

use texform_core::ast::Ast;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_core::serialize::serialize;
use texform_transform::lower_attributes::MathFontValue;
use texform_transform::{
    Attr, AttrValue, AttributeSet, BuildConfig, LowerAttributesConfig, LowerAttributesReport,
    NormalizationLevel, NormalizationLevelSet, Profile, TransformContext,
};

struct Outcome {
    text: String,
    ast: Ast,
}

fn run_with_packages(src: &str, packages: &[&str]) -> Outcome {
    run_with_packages_and_levels(src, packages, &[NormalizationLevel::Standard])
}

fn run_with_packages_and_levels(
    src: &str,
    packages: &[&str],
    levels: &[NormalizationLevel],
) -> Outcome {
    let parse_ctx = ParseContext::from_packages(packages);
    let mut ast = parse_to_ast(&parse_ctx, src);
    let levels = levels
        .iter()
        .copied()
        .fold(NormalizationLevelSet::empty(), |set, level| {
            set | level.into()
        });
    let context = TransformContext::from_build_config(
        BuildConfig::profile(Profile::Authoring).rewrite_levels(levels),
        &parse_ctx,
    )
    .expect("transform context should build");
    context
        .run(&mut ast, &parse_ctx)
        .expect("transform should succeed");
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
    let expected_ast = parse_to_ast(&parse_ctx, expected);
    expected_ast.assert_invariants();

    assert_eq!(actual.text, serialize(&expected_ast));
}

fn serialized(src: &str, expected: &str) {
    serialized_with_packages(src, expected, &["base", "textmacros"]);
}

fn serialized_text(src: &str, expected: &str) {
    assert_eq!(run(src).text, expected);
}

fn parse_to_ast(parse_ctx: &ParseContext, src: &str) -> Ast {
    let document = parse_ctx
        .parse(src, &ParseConfig::default())
        .try_into_document()
        .expect("source should parse")
        .0;
    Ast::from_syntax_root(&document.to_syntax())
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
fn lower_attributes_preserves_whitespace_only_text_prefix_body() {
    serialized_text(r"\textrm{ }", r"\textrm { }");
    serialized_text(r"\textbf{ }", r"\textbf { }");
}

#[test]
fn lower_attributes_preserves_text_prefix_edge_spaces() {
    serialized_text(r"\textbf{ a }", r"\textbf { a }");
    serialized_text(r"\textbf{ a}", r"\textbf { a}");
    serialized_text(r"\textbf{a }", r"\textbf {a }");
}

#[test]
fn lower_attributes_keeps_empty_prefix_body_empty() {
    serialized_text(r"\textrm{}", r"");
    serialized_text(r"\textbf{}", r"");
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
    let actual = run_with_packages_and_levels(
        r"\vb{\rm x}",
        &["base", "textmacros", "physics", "boldsymbol"],
        &[NormalizationLevel::Standard, NormalizationLevel::Expand],
    );
    assert_eq!(actual.text, r"\mathrm { x }");
}

#[test]
fn lower_attributes_report_counts_declarative_and_prefix_forms_for_same_attribute() {
    let parse_ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let mut ast = parse_to_ast(&parse_ctx, r"\bf \mathbf{x}");
    let mut report = LowerAttributesReport::default();
    texform_transform::lower_attributes::run(
        &mut ast,
        &LowerAttributesConfig::ENABLED,
        &mut report,
    );
    let bold = AttributeSet::new(
        Attr::MathFont,
        AttrValue::MathFont(MathFontValue("VARIANT.BOLD")),
    );
    let stat = report
        .attributes
        .get(&bold)
        .expect("bold math font should be reported");

    assert_eq!(stat.consumed.declaratives, 1);
    assert_eq!(stat.consumed.prefixes, 1);
    assert_eq!(stat.redundant.prefixes, 1);
    assert_eq!(stat.emitted.prefixes, 1);
}

#[test]
fn lower_attributes_report_counts_empty_prefix_body_as_redundant() {
    let parse_ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let mut ast = parse_to_ast(&parse_ctx, r"\mathbf{}");
    let mut report = LowerAttributesReport::default();
    texform_transform::lower_attributes::run(
        &mut ast,
        &LowerAttributesConfig::ENABLED,
        &mut report,
    );
    let bold = AttributeSet::new(
        Attr::MathFont,
        AttrValue::MathFont(MathFontValue("VARIANT.BOLD")),
    );
    let stat = report
        .attributes
        .get(&bold)
        .expect("empty bold prefix should be reported");

    assert_eq!(stat.consumed.prefixes, 1);
    assert_eq!(stat.redundant.prefixes, 1);
    assert_eq!(stat.emitted.prefixes, 0);
}
