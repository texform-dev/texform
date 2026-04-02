use texform_argspec::{ArgForm, ContentMode, DelimiterToken, ValueKind, parse_arg_specs};
use texform_specs::argspec;

#[test]
fn test_argspec_macro_matches_runtime_parse_for_empty_spec() {
    let expected = parse_arg_specs("", "empty").expect("empty spec should be valid");
    let actual = argspec!("");

    assert_eq!(actual.args, expected.as_slice());
    assert_eq!(actual.source, "");
}

#[test]
fn test_argspec_macro_matches_runtime_parse_for_standard_spec() {
    let expected = parse_arg_specs("o:N m m", "standard").expect("o:N m m should be valid");
    let actual = argspec!("o:N m m");

    assert_eq!(actual.args, expected.as_slice());
    assert_eq!(actual.source, "o:N m m");
}

#[test]
fn test_argspec_macro_matches_runtime_parse_for_complex_spec() {
    let expected = parse_arg_specs(r"!s g:T d<(,)><\langle,\rangle>:K m{}:D?", "complex")
        .expect("complex spec should be valid");
    let actual = argspec!(r"!s g:T d<(,)><\langle,\rangle>:K m{}:D?");

    assert_eq!(actual.args, expected.as_slice());
    assert_eq!(actual.source, r"!s g:T d<(,)><\langle,\rangle>:K m{}:D?");
}

#[test]
fn test_argspec_macro_preserves_paired_and_delimited_forms() {
    let specs = argspec!(r"r<(,)><[,]> d\langle\rangle");

    match &specs[0].form {
        ArgForm::Paired { pairs } => {
            assert_eq!(pairs.len(), 2);
            assert_eq!(
                pairs[0],
                (DelimiterToken::Char('('), DelimiterToken::Char(')'))
            );
            assert_eq!(
                pairs[1],
                (DelimiterToken::Char('['), DelimiterToken::Char(']'))
            );
        }
        other => panic!("expected paired form, got {:?}", other),
    }

    match &specs[1].form {
        ArgForm::Delimited { open, close } => {
            assert_eq!(
                open,
                &DelimiterToken::ControlSeq(std::borrow::Cow::Borrowed("langle"))
            );
            assert_eq!(
                close,
                &DelimiterToken::ControlSeq(std::borrow::Cow::Borrowed("rangle"))
            );
        }
        other => panic!("expected delimited form, got {:?}", other),
    }

    assert_eq!(
        specs[1].kind,
        ValueKind::Content {
            mode: ContentMode::Math
        }
    );
}
