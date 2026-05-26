#[test]
fn validate_argspec_reports_success() {
    let result = texform::validate_argspec("s o m");

    assert!(result.ok);
    assert!(result.error.is_none());
}

#[test]
fn validate_argspec_reports_parse_errors() {
    let result = texform::validate_argspec("s:T");

    assert!(!result.ok);
    assert!(
        result
            .error
            .as_deref()
            .is_some_and(|error| error.contains("does not accept value type annotation"))
    );
}
