//! Stress and safety tests for the LaTeX parser.
//!
//! Verifies linear scaling on large formulas, graceful depth-limit handling on
//! deeply nested inputs (no panics or stack overflows), and termination on
//! pathological truncated/malformed inputs under both `STRICT` and `LENIENT`.

use texform_core::parse::{ParseConfig, ParseContext, ParseDiagnosticKind};
use texform_interface::syntax_node::SyntaxNode;

fn parse_shared(src: &str, config: &ParseConfig) -> texform_core::parse::ParseResult {
    ParseContext::shared().parse(src, config)
}

#[test]
fn stress_long_flat_formula_10k_chars() {
    // 2500 * 4 + 1 = 10,001 characters.
    let src = format!("{}a", "a + ".repeat(2500));
    assert!(src.len() > 10_000);

    for config in [ParseConfig::STRICT, ParseConfig::LENIENT] {
        let output = parse_shared(&src, &config);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics on 10k formula (abort_on_error={}): {:?}",
            config.abort_on_error,
            output.diagnostics
        );
        let doc = output
            .document()
            .expect("10k formula should produce a document");
        assert!(!doc.has_errors());
        let SyntaxNode::Root { children, .. } = doc.to_syntax() else {
            panic!("expected root node");
        };
        assert_eq!(children.len(), 5001);
    }
}

#[test]
fn stress_deeply_nested_braces_1000_levels() {
    // Exactly `DEFAULT_MAX_GROUP_DEPTH` (128) nested groups must parse cleanly
    // without hitting the global recursion guard.
    let at_limit = format!(
        "{}x{}",
        "{".repeat(ParseConfig::DEFAULT_MAX_GROUP_DEPTH),
        "}".repeat(ParseConfig::DEFAULT_MAX_GROUP_DEPTH)
    );
    let ok_output = parse_shared(&at_limit, &ParseConfig::STRICT);
    assert!(
        ok_output.diagnostics.is_empty(),
        "braces at DEFAULT_MAX_GROUP_DEPTH must succeed: {:?}",
        ok_output.diagnostics
    );
    assert!(ok_output.document().is_some());

    let src = format!("{}x{}", "{".repeat(1000), "}".repeat(1000));

    for config in [ParseConfig::STRICT, ParseConfig::LENIENT] {
        let output = parse_shared(&src, &config);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.kind == ParseDiagnosticKind::MaxGroupDepthExceeded),
            "expected MaxGroupDepthExceeded (abort_on_error={}): {:?}",
            config.abort_on_error,
            output.diagnostics
        );
        // The max-group-depth guard emits an in-place Error node unconditionally.
        let doc = output
            .document()
            .expect("max_group_depth guard should still yield an error tree");
        assert!(doc.has_errors());
    }
}

#[test]
fn stress_deeply_nested_frac_and_left_right_1000_levels() {
    let nested_frac = format!("{}x{}", r"\frac{1}{".repeat(1000), "}".repeat(1000));
    let nested_shorthand_frac = format!("{}x", r"\frac 1 ".repeat(1000));
    let nested_left_right = format!("{}x{}", r"\left(".repeat(1000), r"\right)".repeat(1000));

    for src in [&nested_frac, &nested_shorthand_frac, &nested_left_right] {
        for config in [ParseConfig::STRICT, ParseConfig::LENIENT] {
            let output = parse_shared(src, &config);
            assert!(
                output.document().is_none(),
                "global recursion overflow must abort cleanly (abort_on_error={})",
                config.abort_on_error
            );
            assert!(
                output
                    .diagnostics
                    .iter()
                    .any(|d| d.kind == ParseDiagnosticKind::MaxRecursionDepthExceeded),
                "expected MaxRecursionDepthExceeded (abort_on_error={}): {:?}",
                config.abort_on_error,
                output.diagnostics
            );
        }
    }
}

#[test]
fn stress_pathological_truncated_and_malformed_inputs() {
    let unclosed_braces = "{".repeat(1000);
    let stray_rbraces = "}".repeat(1000);
    let unclosed_lefts = r"\left(".repeat(200);
    let unclosed_begins = r"\begin{matrix}".repeat(200);
    let truncated_fracs = r"\frac{".repeat(200);
    let dangling_scripts = "^_".repeat(500);

    let cases: &[&str] = &[
        "{",
        "}",
        &unclosed_braces,
        &stray_rbraces,
        r"\begin",
        r"\begin{",
        r"\begin{matrix",
        r"\begin{matrix}",
        r"\begin{matrix} a \end{",
        r"\begin{matrix} a \end{align}",
        r"\end{matrix}",
        &unclosed_begins,
        r"\left",
        r"\left(",
        r"\left( x",
        r"\right)",
        &unclosed_lefts,
        r"\frac",
        r"\frac{",
        r"\frac{a",
        r"\frac{a}{",
        r"\frac{a}{b",
        &truncated_fracs,
        r"\sqrt[",
        r"\sqrt[3",
        r"\sqrt[3]{",
        r"\frac{a % truncated by comment",
        "\\frac{a % comment\n",
        r"\sqrt[x % truncated",
        r"\text{$x",
        r"\text{\(x",
        "$$$",
        &dangling_scripts,
        r"a \over b \over c \over d",
        r"\begin{array}{*{100}{c}} a & b",
    ];

    for src in cases {
        for config in [ParseConfig::STRICT, ParseConfig::LENIENT] {
            let output = parse_shared(src, &config);
            assert!(
                !output.diagnostics.is_empty(),
                "malformed input must emit at least one diagnostic (abort_on_error={}): {:?}",
                config.abort_on_error,
                &src[..src.len().min(60)]
            );
            if config.abort_on_error
                && !output
                    .diagnostics
                    .iter()
                    .any(|d| d.kind == ParseDiagnosticKind::MaxGroupDepthExceeded)
            {
                assert!(
                    output.document().is_none(),
                    "STRICT parse must not return a document on non-depth error: {:?} -> {:?}",
                    &src[..src.len().min(60)],
                    output.diagnostics
                );
            }
        }
    }
}
