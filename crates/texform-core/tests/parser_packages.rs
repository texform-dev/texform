mod support;

use support::parser::*;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_interface::syntax_node::ArgumentValue;

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
        assert!(
            output.document().is_some(),
            "expected parse result for {src}"
        );
    }

    let output = ctx.parse(r"\bra{x}\|\ket{y}", &ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for braket sample: {:?}",
        output.diagnostics
    );
    let result = output
        .document()
        .expect("expected parse result for braket sample");
    assert!(
        extract_command_args(&result.to_syntax(), "|").is_some(),
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
        assert!(
            output.document().is_some(),
            "expected parse result for {src}"
        );
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
            .document()
            .unwrap_or_else(|| panic!("expected parse result for {src}"));
        assert!(
            extract_command_args(&result.to_syntax(), command_name).is_some(),
            "expected package-backed command {command_name:?} in {src}"
        );
    }
}

#[test]
fn test_package_hspace_accepts_optional_star_in_math_and_text_modes() {
    let ctx = ParseContext::from_packages(&["base", "textmacros"]);

    for (src, starred) in [
        (r"\hspace*{1em}", true),
        (r"\hspace{1em}", false),
        (r"\text{a\hspace*{1em}b}", true),
        (r"\text{a\hspace{1em}b}", false),
    ] {
        let output = ctx.parse(src, &ParseConfig::STRICT);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let syntax = output.document().expect("parse result").to_syntax();
        let args = extract_command_args(&syntax, "hspace").expect("hspace command");
        assert_eq!(
            args.iter()
                .map(|arg| expect_arg(arg).value.clone())
                .collect::<Vec<_>>(),
            [
                ArgumentValue::Boolean(starred),
                ArgumentValue::Dimension("1em".to_string())
            ],
            "{src}"
        );
    }
}
