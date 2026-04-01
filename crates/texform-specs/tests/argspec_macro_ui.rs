#[test]
fn test_argspec_macro_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/argspec-ok.rs");
    cases.compile_fail("tests/ui/argspec-invalid.rs");
}
