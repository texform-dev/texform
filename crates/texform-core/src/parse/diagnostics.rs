//! Strongly-typed diagnostic constructors, lightweight site helpers, and
//! final deduplication / priority sorting for parser diagnostics.

use texform_interface::syntax_node::ContentMode;

use crate::lexer::Token;
use crate::parse::result::{ParseDiagnostic, ParseDiagnosticKind, Span};

/// Format a lexed token for the structured `found` field and generic messages.
#[inline]
pub(crate) fn format_token_label(token: Token<'_>) -> String {
    token.to_string()
}

/// Returns `true` when `slice` contains an unescaped `%` character.
///
/// Used at argument-collection failure sites to classify EOF truncation caused
/// by an unescaped LaTeX comment without rescanning the full source.
pub(crate) fn has_unescaped_percent(slice: &str) -> bool {
    let mut escaped = false;
    for ch in slice.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '%' {
            return true;
        }
    }
    false
}

/// Returns `true` when the byte at `offset` in `src` is an ASCII digit.
///
/// Used to add the currency-marker hint for an unexpected `$` inside math mode.
#[inline]
pub(crate) fn followed_by_ascii_digit(src: &str, offset: usize) -> bool {
    src.as_bytes().get(offset).is_some_and(u8::is_ascii_digit)
}

fn build_diagnostic(
    kind: ParseDiagnosticKind,
    message: String,
    span: Span,
    expected: Vec<String>,
    found: Option<String>,
) -> ParseDiagnostic {
    ParseDiagnostic {
        kind: Some(kind),
        message,
        span,
        expected,
        found,
        contexts: Vec::new(),
    }
}

/// Construct an [`ParseDiagnosticKind::UnknownCommand`] diagnostic.
pub(crate) fn unknown_command(name: &str, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::UnknownCommand,
        format!("Unknown command: \\{name}"),
        span,
        Vec::new(),
        None,
    )
}

/// Construct a [`ParseDiagnosticKind::CommandModeError`] diagnostic.
pub(crate) fn command_mode_error(name: &str, mode: ContentMode, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::CommandModeError,
        format!("Command \\{name} is not allowed in {mode} mode"),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::UnknownEnvironment`] diagnostic.
pub(crate) fn unknown_environment(name: &str, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::UnknownEnvironment,
        format!("Unknown environment: {name}"),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::EnvironmentModeError`] diagnostic.
pub(crate) fn environment_mode_error(name: &str, mode: ContentMode, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::EnvironmentModeError,
        format!("Environment {name} is not allowed in {mode} mode"),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::EnvironmentNameMismatch`] diagnostic.
pub(crate) fn environment_name_mismatch(
    expected: &str,
    found: &str,
    span: Span,
) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::EnvironmentNameMismatch,
        format!("Environment name mismatch: expected \\end{{{expected}}}, found \\end{{{found}}}"),
        span,
        vec![format!("\\end{{{expected}}}")],
        Some(format!("\\end{{{found}}}")),
    )
}

/// Construct an [`ParseDiagnosticKind::UnclosedEnvironment`] diagnostic.
pub(crate) fn unclosed_environment(
    expected: &str,
    span: Span,
    found: Option<String>,
) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::UnclosedEnvironment,
        format!("Environment {expected} missing closing \\end{{{expected}}}"),
        span,
        vec![format!("\\end{{{expected}}}")],
        found,
    )
}

/// Construct a [`ParseDiagnosticKind::LeftRightDelimiter`] diagnostic.
pub(crate) fn left_right_delimiter(message: impl Into<String>, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::LeftRightDelimiter,
        message.into(),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::UnclosedInlineMath`] diagnostic pointed
/// at its opening `$`.
pub(crate) fn unclosed_inline_math(opener_span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::UnclosedInlineMath,
        "Unclosed inline math: missing closing '$'".to_string(),
        opener_span,
        vec!["'$'".to_string()],
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::UnexpectedMathShift`] diagnostic,
/// adding a currency hint when `$` is immediately followed by an ASCII digit.
pub(crate) fn unexpected_math_shift(src: &str, span: Span) -> ParseDiagnostic {
    let message = if followed_by_ascii_digit(src, span.end) {
        "Unexpected $ inside a math formula; it looks like a currency marker"
    } else {
        "Unexpected $ inside a math formula"
    };
    build_diagnostic(
        ParseDiagnosticKind::UnexpectedMathShift,
        message.to_string(),
        span,
        Vec::new(),
        Some("$".to_string()),
    )
}

/// Construct a [`ParseDiagnosticKind::TextScriptError`] diagnostic.
pub(crate) fn text_script_error(span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::TextScriptError,
        "Scripted syntax is not allowed in Text mode".to_string(),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::AmbiguousInfix`] diagnostic.
pub(crate) fn ambiguous_infix(name: &str, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::AmbiguousInfix,
        format!("Ambiguous use of \\{name}"),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::ArgumentValidation`] diagnostic.
pub(crate) fn argument_validation(message: impl Into<String>, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::ArgumentValidation,
        message.into(),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an [`ParseDiagnosticKind::ArgumentValidation`] diagnostic with
/// explicit `expected` and `found` metadata.
pub(crate) fn argument_validation_expected(
    message: impl Into<String>,
    span: Span,
    expected: &[&str],
    found: Option<Token<'_>>,
) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::ArgumentValidation,
        message.into(),
        span,
        expected.iter().map(|label| (*label).to_string()).collect(),
        found.map(format_token_label),
    )
}

/// Construct a [`ParseDiagnosticKind::CommentTruncatedArgument`] diagnostic.
pub(crate) fn comment_truncated_argument(span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::CommentTruncatedArgument,
        "Unescaped % starts a comment inside this argument".to_string(),
        span,
        Vec::new(),
        None,
    )
}

/// Construct a [`ParseDiagnosticKind::MaxGroupDepthExceeded`] diagnostic for
/// explicit `{...}` group nesting.
pub(crate) fn max_group_depth_exceeded(max_depth: usize, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::MaxGroupDepthExceeded,
        format!("max group depth ({max_depth}) exceeded"),
        span,
        Vec::new(),
        None,
    )
}

/// Construct a [`ParseDiagnosticKind::MaxGroupDepthExceeded`] diagnostic for
/// global parser recursion-depth overflow.
pub(crate) fn max_recursion_depth_exceeded(max_depth: usize, span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::MaxGroupDepthExceeded,
        format!("max recursion depth ({max_depth}) exceeded"),
        span,
        Vec::new(),
        None,
    )
}

/// Construct an uncategorized (`kind: None`) custom parser diagnostic (e.g.
/// double subscript / double exponent errors).
pub(crate) fn custom_error(message: impl Into<String>, span: Span) -> ParseDiagnostic {
    ParseDiagnostic {
        kind: None,
        message: message.into(),
        span,
        expected: Vec::new(),
        found: None,
        contexts: Vec::new(),
    }
}

/// Construct a generic [`ParseDiagnosticKind::RawExpectedFound`] diagnostic.
pub(crate) fn raw_expected_found(
    span: Span,
    expected: &[&str],
    found: Option<Token<'_>>,
) -> ParseDiagnostic {
    let found_label = found.map(format_token_label);
    let expected_vec: Vec<String> = expected.iter().map(|label| (*label).to_string()).collect();
    let found_desc = match found_label.as_deref() {
        Some(label) => format!("'{label}'"),
        None => "end of input".to_string(),
    };
    let message = match expected {
        [] => format!("unexpected {found_desc}"),
        [single] => format!("found {found_desc} expected {single}"),
        [first, second] => format!("found {found_desc} expected {first}, or {second}"),
        [init @ .., last] => {
            format!("found {found_desc} expected {}, or {last}", init.join(", "))
        }
    };
    build_diagnostic(
        ParseDiagnosticKind::RawExpectedFound,
        message,
        span,
        expected_vec,
        found_label,
    )
}

/// Construct a [`ParseDiagnosticKind::RawExpectedFound`] diagnostic for a
/// superscript (`^`) or subscript (`_`) missing its operand.
pub(crate) fn missing_script_content(
    is_superscript: bool,
    span: Span,
    found: Option<Token<'_>>,
) -> ParseDiagnostic {
    let (message, expected) = if is_superscript {
        ("Missing superscript content", "superscript content")
    } else {
        ("Missing subscript content", "subscript content")
    };
    build_diagnostic(
        ParseDiagnosticKind::RawExpectedFound,
        message.to_string(),
        span,
        vec![expected.to_string()],
        found.map(format_token_label),
    )
}

/// Construct a [`ParseDiagnosticKind::RawExpectedFound`] diagnostic for an
/// unclosed `{ ... }` group at EOF.
pub(crate) fn unclosed_group(span: Span) -> ParseDiagnostic {
    build_diagnostic(
        ParseDiagnosticKind::RawExpectedFound,
        "Unclosed { ... } group".to_string(),
        span,
        vec!["'}'".to_string()],
        None,
    )
}

/// Priority levels for sorting diagnostics (1 = highest).
pub(crate) fn diagnostic_priority(diagnostic: &ParseDiagnostic) -> u8 {
    match diagnostic.kind {
        Some(
            ParseDiagnosticKind::InvalidCharacter
            | ParseDiagnosticKind::UnknownCommand
            | ParseDiagnosticKind::UnknownEnvironment
            | ParseDiagnosticKind::CommentTruncatedArgument
            | ParseDiagnosticKind::UnexpectedMathShift
            | ParseDiagnosticKind::LeftRightDelimiter
            | ParseDiagnosticKind::AmbiguousInfix,
        ) => 1,
        Some(ParseDiagnosticKind::RawExpectedFound) if diagnostic.found.is_none() => 3,
        Some(ParseDiagnosticKind::RawExpectedFound) => 4,
        Some(_) | None => 2,
    }
}

/// Recovery deduplication equivalence across kind, span, message, and contexts.
fn matches_recovery_diagnostic(left: &ParseDiagnostic, right: &ParseDiagnostic) -> bool {
    left.kind == right.kind
        && left.span == right.span
        && left.message == right.message
        && left.contexts == right.contexts
}

/// Deduplicate and stably sort parser diagnostics by priority (1..=4).
pub(crate) fn finish_diagnostics(diags: Vec<ParseDiagnostic>) -> Vec<ParseDiagnostic> {
    let mut unique: Vec<ParseDiagnostic> = Vec::with_capacity(diags.len());
    for candidate in diags {
        if unique
            .iter()
            .any(|existing| matches_recovery_diagnostic(existing, &candidate))
        {
            continue;
        }
        unique.push(candidate);
    }
    unique.sort_by_key(diagnostic_priority);
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::result::ParseDiagnosticContext;

    #[test]
    fn site_helpers_detect_percent_and_currency_digits() {
        assert!(!has_unescaped_percent(""));
        assert!(!has_unescaped_percent(r"100\% ok"));
        assert!(has_unescaped_percent(r"a % comment"));
        assert!(has_unescaped_percent(r"\\ % comment after newline cmd"));
        assert!(!has_unescaped_percent(r"\\\% still escaped"));

        assert!(followed_by_ascii_digit("$100", 1));
        assert!(!followed_by_ascii_digit("$x", 1));
        assert!(!followed_by_ascii_digit("$", 1));
    }

    #[test]
    fn unexpected_math_shift_attaches_currency_hint_only_before_digits() {
        let currency = unexpected_math_shift("$100", Span::new(0, 1));
        assert_eq!(
            currency.message,
            "Unexpected $ inside a math formula; it looks like a currency marker"
        );
        assert_eq!(currency.found.as_deref(), Some("$"));

        let plain = unexpected_math_shift("$x", Span::new(0, 1));
        assert_eq!(plain.message, "Unexpected $ inside a math formula");
        assert_eq!(plain.found.as_deref(), Some("$"));
    }

    #[test]
    fn finish_diagnostics_deduplicates_and_stably_sorts_by_priority() {
        let raw_found = raw_expected_found(Span::new(5, 6), &["end of input"], Some(Token::RBrace));
        let raw_eof = raw_expected_found(Span::point(10), &["'}'"], None);
        let mode_err = command_mode_error("frac", ContentMode::Text, Span::new(1, 6));
        let unknown_a = unknown_command("foo", Span::new(0, 4));
        let unknown_a_dup = unknown_command("foo", Span::new(0, 4));
        let unknown_b = unknown_command("bar", Span::new(7, 11));
        let mut unknown_a_different_ctx = unknown_command("foo", Span::new(0, 4));
        unknown_a_different_ctx.contexts = vec![ParseDiagnosticContext {
            label: "command argument".to_string(),
            span: Span::new(0, 5),
        }];

        assert_eq!(diagnostic_priority(&unknown_a), 1);
        assert_eq!(diagnostic_priority(&mode_err), 2);
        assert_eq!(diagnostic_priority(&raw_eof), 3);
        assert_eq!(diagnostic_priority(&raw_found), 4);

        let finished = finish_diagnostics(vec![
            raw_found.clone(),
            raw_eof.clone(),
            mode_err.clone(),
            unknown_a.clone(),
            unknown_a_dup,
            unknown_b.clone(),
            unknown_a_different_ctx.clone(),
        ]);

        assert_eq!(finished.len(), 6);
        assert_eq!(finished[0], unknown_a);
        assert_eq!(finished[1], unknown_b);
        assert_eq!(finished[2], unknown_a_different_ctx);
        assert_eq!(finished[3], mode_err);
        assert_eq!(finished[4], raw_eof);
        assert_eq!(finished[5], raw_found);
    }

    #[test]
    fn format_token_label_matches_expected_surface_forms() {
        assert_eq!(format_token_label(Token::ControlSeq("frac")), r"\frac");
        assert_eq!(format_token_label(Token::RBrace), "}");
        assert_eq!(format_token_label(Token::MathShift), "$");
        assert_eq!(format_token_label(Token::Prime(2)), "''");
        assert_eq!(format_token_label(Token::Char('x')), "x");
    }
}
