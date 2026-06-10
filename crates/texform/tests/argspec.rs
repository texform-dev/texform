#[test]
fn validate_argspec_reports_success() {
    let result = texform::validate_argspec("s o m");

    assert!(result.valid);
    assert!(result.error.is_none());
    assert_eq!(result.arg_count, Some(3));
    assert_eq!(result.parsed.as_ref().map(Vec::len), Some(3));
}

#[test]
fn validate_argspec_reports_parse_errors() {
    let result = texform::validate_argspec("s:T");

    assert!(!result.valid);
    assert_eq!(result.arg_count, None);
    assert_eq!(result.parsed, None);
    assert!(
        result
            .error
            .as_deref()
            .is_some_and(|error| error.contains("does not accept value type annotation"))
    );
}
