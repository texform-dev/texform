mod support;

use support::parser::*;
use texform_core::parse::{ParseConfig, ParseContext};

#[test]
fn test_package_loaded_non_alpha_math_commands_support_representative_forms() {
    let ctx = ParseContext::from_packages(&["ams", "base", "braket", "physics"]);

    for src in [
        r"a\,b", r"a\!b", r"a\;b", r"a\:b", r"a\>b", r"a\*b", r"a\ b",
    ] {
        let output = ctx.parse(src, &ParseConfig::STRICT);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        assert!(output.result.is_some(), "expected parse result for {src}");
    }

    let output = ctx.parse(r"\bra{x}\|\ket{y}", &ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for braket sample: {:?}",
        output.diagnostics
    );
    let result = output
        .result
        .as_ref()
        .expect("expected parse result for braket sample");
    assert!(
        extract_command_args(&result.node, "|").is_some(),
        "expected package-backed \\| command"
    );
}

#[test]
fn test_package_loaded_non_alpha_text_commands_support_representative_forms() {
    let ctx = ParseContext::from_packages(&["base", "textmacros"]);

    for src in [r"\text{a\,b}", r"\text{a\ b}"] {
        let output = ctx.parse(src, &ParseConfig::STRICT);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        assert!(output.result.is_some(), "expected parse result for {src}");
    }

    for (src, command_name) in [
        (r"\text{\'e}", "'"),
        (r"\text{\~n}", "~"),
        (r#"\text{\"o}"#, "\""),
    ] {
        let output = ctx.parse(src, &ParseConfig::STRICT);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let result = output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result for {src}"));
        assert!(
            extract_command_args(&result.node, command_name).is_some(),
            "expected package-backed command {command_name:?} in {src}"
        );
    }
}
