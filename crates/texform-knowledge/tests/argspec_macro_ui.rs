#[test]
fn test_argspec_macro_ui() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/argspec-invalid.rs");
}
