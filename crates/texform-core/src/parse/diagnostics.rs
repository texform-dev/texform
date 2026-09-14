//! Conversion of parser failures into public diagnostics.

use super::context::*;
use super::error::ParseFailure;
use crate::lexer::Token;
use logos::Logos;
type LexedSource = Vec<(Token, std::ops::Range<usize>)>;

fn lex_source(src: &str) -> LexedSource {
    Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (token, span)
        })
        .collect()
}

pub(super) fn convert_diagnostic(
    ctx: &ParseContext,
    src: &str,
    err: ParseFailure<'static>,
) -> (u8, ParseDiagnostic) {
    let span = {
        let s = err
            .direct
            .as_ref()
            .map_or(err.span(), |direct| &direct.span);
        Span {
            start: s.start,
            end: s.end,
        }
    };

    let reason = err.reason();
    let raw_eof = matches!(
        reason,
        chumsky::error::RichReason::ExpectedFound { found: None, .. }
    );
    let kind = err.kind;
    let contexts = err
        .contexts()
        .map(|(label, span)| ParseDiagnosticContext {
            label: label.to_string(),
            span: Span {
                start: span.start,
                end: span.end,
            },
        })
        .collect();

    let (message, expected, found) = match reason {
        chumsky::error::RichReason::ExpectedFound {
            expected: exp,
            found: f,
        } => {
            let expected: Vec<String> = exp.iter().map(|p| format!("{p}")).collect();
            let found = f.as_ref().map(|t| format!("{}", **t));

            let msg = format!("{reason}");
            (msg, expected, found)
        }
        chumsky::error::RichReason::Custom(msg) => (msg.clone(), Vec::new(), None),
    };

    let mut kind = kind.or_else(|| infer_raw_diagnostic_kind(&err));

    let mut diagnostic = ParseDiagnostic {
        kind,
        message,
        span,
        expected,
        found,
        contexts,
    };

    supplement_comment_truncated_argument(src, raw_eof, &mut kind, &mut diagnostic);
    if let Some(direct) = &err.direct
        && let Some(environment) = &direct.environment
    {
        let expected = &environment.expected_name;
        let found = &environment.name;
        diagnostic.expected = vec![format!("\\end{{{expected}}}")];
        diagnostic.found = Some(format!("\\end{{{found}}}"));
    }
    let normalized_eof = supplement_diagnostic_contexts(
        ctx,
        src,
        kind,
        err.direct.is_some(),
        raw_eof,
        &mut diagnostic,
    );
    (
        parse_diagnostic_priority(&diagnostic, raw_eof && !normalized_eof),
        diagnostic,
    )
}

fn parse_diagnostic_priority(diagnostic: &ParseDiagnostic, raw_eof: bool) -> u8 {
    match diagnostic.kind {
        Some(
            ParseDiagnosticKind::UnknownCommand
            | ParseDiagnosticKind::UnknownEnvironment
            | ParseDiagnosticKind::CommentTruncatedArgument
            | ParseDiagnosticKind::UnexpectedMathShift
            | ParseDiagnosticKind::LeftRightDelimiter
            | ParseDiagnosticKind::AmbiguousInfix,
        ) => 1,
        Some(ParseDiagnosticKind::ArgumentValidation) => 2,
        Some(ParseDiagnosticKind::EnvironmentNameMismatch) => 2,
        Some(ParseDiagnosticKind::RawExpectedFound) if raw_eof => 3,
        Some(ParseDiagnosticKind::RawExpectedFound) => 4,
        Some(_) | None => 2,
    }
}

/// Best-effort fallback for chumsky-generated `ExpectedFound` errors that carry
/// no explicit `ParseDiagnosticKind`.  The heuristics here match the token
/// patterns that chumsky emits for known parser structures (e.g. `}` from an
/// environment-name mismatch, `\begin` from an unknown environment).
fn infer_raw_diagnostic_kind(err: &ParseFailure<'_>) -> Option<ParseDiagnosticKind> {
    use chumsky::error::{RichPattern, RichReason};
    let RichReason::ExpectedFound { expected, found } = err.reason() else {
        return None;
    };
    let found = found.as_deref();
    if expected
        .iter()
        .any(|pattern| matches!(pattern, RichPattern::Token(token) if **token == Token::MathShift))
        && (matches!(found, None | Some(Token::MathShift))
            || matches!(found, Some(Token::ControlSeq(name)) if name == "text"))
    {
        return Some(ParseDiagnosticKind::UnclosedInlineMath);
    }
    match found {
        Some(Token::MathShift) => Some(ParseDiagnosticKind::UnexpectedMathShift),
        Some(Token::RBrace) => Some(ParseDiagnosticKind::EnvironmentNameMismatch),
        Some(Token::ControlSeq(name)) if name == "begin" => {
            Some(ParseDiagnosticKind::UnknownEnvironment)
        }
        _ if !expected.is_empty() => Some(ParseDiagnosticKind::RawExpectedFound),
        _ => None,
    }
}

fn supplement_diagnostic_contexts(
    ctx: &ParseContext,
    src: &str,
    kind: Option<ParseDiagnosticKind>,
    direct: bool,
    raw_eof: bool,
    diagnostic: &mut ParseDiagnostic,
) -> bool {
    let mut lexed = None;

    supplement_unclosed_inline_math_message(kind, src, diagnostic);
    supplement_unexpected_math_shift_message(kind, src, diagnostic);
    let mut normalized_eof = supplement_generic_unclosed_message(kind, src, raw_eof, diagnostic);
    if !direct {
        normalized_eof |=
            supplement_environment_mode_error_message(kind, ctx, src, &mut lexed, diagnostic);
        supplement_environment_mismatch_message(kind, src, &mut lexed, diagnostic);
        supplement_unknown_environment_message(kind, ctx, src, &mut lexed, diagnostic);
    }
    supplement_argument_validation_span(kind, src, &mut lexed, diagnostic);

    let needs_left_context = kind == Some(ParseDiagnosticKind::LeftRightDelimiter);
    if !needs_left_context {
        return normalized_eof;
    }

    let Some((left_span, env_span)) =
        find_invalid_left_context(ctx, lexed.get_or_insert_with(|| lex_source(src)))
    else {
        return normalized_eof;
    };

    if !diagnostic
        .contexts
        .iter()
        .any(|context| context.label == "left-delimited group")
    {
        diagnostic.contexts.push(ParseDiagnosticContext {
            label: "left-delimited group".to_string(),
            span: left_span,
        });
    }

    if let Some(env_span) = env_span
        && !diagnostic
            .contexts
            .iter()
            .any(|context| context.label == "environment body")
    {
        diagnostic.contexts.push(ParseDiagnosticContext {
            label: "environment body".to_string(),
            span: env_span,
        });
    }
    normalized_eof
}

/// Normalize the lone inline-math opener message so recoverable content
/// subparses report the same generic tail error shape as the top-level parser.
fn supplement_unclosed_inline_math_message(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::UnclosedInlineMath) {
        return;
    }

    diagnostic.message = "found '$' expected something else, or end of input".to_string();
    if diagnostic.expected.iter().any(|value| value == "'$'") {
        diagnostic.expected = vec!["something else".to_string(), "end of input".to_string()];
    }
    if diagnostic.found.as_deref() == Some("\\text")
        && let Some(span) = find_inline_math_shift_after_command(src, diagnostic.span.clone())
    {
        diagnostic.span = span;
        diagnostic.found = Some("$".to_string());
    }
}

fn supplement_comment_truncated_argument(
    src: &str,
    raw_eof: bool,
    kind: &mut Option<ParseDiagnosticKind>,
    diagnostic: &mut ParseDiagnostic,
) {
    if !matches!(
        *kind,
        Some(ParseDiagnosticKind::ArgumentValidation | ParseDiagnosticKind::RawExpectedFound)
            | None
    ) {
        return;
    }

    if !matches!(
        diagnostic.message.as_str(),
        "unclosed brace argument" | "unclosed bracket argument" | "unclosed delimited argument"
    ) && !raw_eof
    {
        return;
    }

    let tail_span = Span {
        start: diagnostic.span.start,
        end: src.len(),
    };
    let candidate_spans = std::iter::once(diagnostic.span.clone())
        .chain(std::iter::once(tail_span))
        .chain(
            diagnostic
                .contexts
                .iter()
                .filter(|context| context.label.contains("argument"))
                .map(|context| context.span.clone()),
        );

    if !candidate_spans
        .filter_map(|span| src.get(span.start..span.end))
        .any(has_unescaped_percent)
    {
        return;
    }

    *kind = Some(ParseDiagnosticKind::CommentTruncatedArgument);
    diagnostic.kind = *kind;
    diagnostic.message = "Unescaped % starts a comment inside this argument".to_string();
    diagnostic.expected.clear();
    diagnostic.found = None;
}

fn has_unescaped_percent(slice: &str) -> bool {
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

fn supplement_unexpected_math_shift_message(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::UnexpectedMathShift) {
        return;
    }

    diagnostic.message = if src
        .as_bytes()
        .get(diagnostic.span.end)
        .is_some_and(u8::is_ascii_digit)
    {
        "Unexpected $ inside a math formula; it looks like a currency marker".to_string()
    } else {
        "Unexpected $ inside a math formula".to_string()
    };
    diagnostic.expected.clear();
    diagnostic.found = Some("$".to_string());
}

fn supplement_generic_unclosed_message(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    raw_eof: bool,
    diagnostic: &mut ParseDiagnostic,
) -> bool {
    if kind != Some(ParseDiagnosticKind::RawExpectedFound) || !raw_eof {
        return false;
    }

    if let Some(argument_context) = diagnostic
        .contexts
        .iter()
        .find(|context| context.label.contains("argument"))
        && let Some(command_name) = command_name_before(src, argument_context.span.start)
    {
        diagnostic.message = format!("Command \\{} has an unclosed argument", command_name);
        return true;
    }

    if let Some(env_name) = last_unclosed_environment_name(src) {
        diagnostic.message = format!(
            "Environment {} missing closing \\end{{{}}}",
            env_name, env_name
        );
        return true;
    }

    if diagnostic
        .span
        .start
        .checked_sub(1)
        .and_then(|index| src.as_bytes().get(index))
        == Some(&b'{')
    {
        diagnostic.message = "Unclosed { ... } group".to_string();
        return true;
    }
    false
}

fn command_name_before(src: &str, offset: usize) -> Option<&str> {
    let prefix = src.get(..offset)?;
    let slash = prefix.rfind('\\')?;
    let rest = prefix.get(slash + 1..)?;
    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_ascii_alphabetic()).then_some(index))
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

fn last_unclosed_environment_name(src: &str) -> Option<String> {
    let lexed = lex_source(src);
    let mut stack = Vec::new();
    let mut index = 0;

    while index < lexed.len() {
        let Token::ControlSeq(head) = &lexed[index].0 else {
            index += 1;
            continue;
        };
        if !matches!(head.as_str(), "begin" | "end") {
            index += 1;
            continue;
        }

        let mut next = index + 1;
        while matches!(lexed.get(next), Some((Token::Whitespaces, _))) {
            next += 1;
        }
        if !matches!(lexed.get(next), Some((Token::LBrace, _))) {
            index += 1;
            continue;
        }
        next += 1;

        let mut env_name = String::new();
        while let Some((token, _)) = lexed.get(next) {
            match token {
                Token::Char(ch) => env_name.push(*ch),
                Token::Star => env_name.push('*'),
                Token::RBrace => break,
                _ => {
                    env_name.clear();
                    break;
                }
            }
            next += 1;
        }

        if env_name.is_empty() {
            index += 1;
            continue;
        }

        if head == "begin" {
            stack.push(env_name);
        } else if let Some(pos) = stack.iter().rposition(|open| open == &env_name) {
            stack.truncate(pos);
        }
        index += 1;
    }

    stack.pop()
}

/// Locate the `$` that immediately starts a braced inline-math argument after a command span.
fn find_inline_math_shift_after_command(src: &str, command_span: Span) -> Option<Span> {
    let mut offset = command_span.end;
    while matches!(src.as_bytes().get(offset), Some(b' ' | b'\t' | b'\n')) {
        offset += 1;
    }
    if src.as_bytes().get(offset) != Some(&b'{') || src.as_bytes().get(offset + 1) != Some(&b'$') {
        return None;
    }

    Some(Span {
        start: offset + 1,
        end: offset + 2,
    })
}

fn supplement_environment_mode_error_message(
    kind: Option<ParseDiagnosticKind>,
    ctx: &ParseContext,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) -> bool {
    // Fallback: raw ExpectedFound errors come from chumsky before
    // TeXForm has a parser-private diagnostic kind to attach.
    if !matches!(
        kind,
        Some(ParseDiagnosticKind::RawExpectedFound | ParseDiagnosticKind::EnvironmentNameMismatch)
    ) {
        return false;
    }

    let Some((name, disallowed_mode, span)) = find_environment_mode_error_at_span(
        ctx,
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.clone(),
    )
    .or_else(|| {
        if diagnostic.span.start == 0 {
            find_first_known_but_disallowed_environment(
                ctx,
                lexed.get_or_insert_with(|| lex_source(src)),
            )
        } else {
            None
        }
    }) else {
        return false;
    };

    diagnostic.message = format!(
        "Environment {} is not allowed in {} mode",
        name, disallowed_mode
    );
    diagnostic.span = span;
    diagnostic.expected.clear();
    diagnostic.found = None;
    true
}

fn supplement_environment_mismatch_message(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::EnvironmentNameMismatch) {
        return;
    }

    let Some((expected, found, span)) = find_environment_name_mismatch(
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.clone(),
    ) else {
        return;
    };

    diagnostic.message = format!(
        "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
        expected, found
    );
    diagnostic.span = span;
    diagnostic.expected = vec![format!("\\end{{{}}}", expected)];
    diagnostic.found = Some(format!("\\end{{{}}}", found));
}

fn supplement_unknown_environment_message(
    kind: Option<ParseDiagnosticKind>,
    ctx: &ParseContext,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::UnknownEnvironment) {
        return;
    }

    let Some((name, span)) = find_unknown_environment_at_span(
        ctx,
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.clone(),
    ) else {
        return;
    };

    diagnostic.message = format!("Unknown environment: {}", name);
    diagnostic.span = span;
    diagnostic.expected.clear();
    diagnostic.found = None;
}

fn supplement_argument_validation_span(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::ArgumentValidation) {
        return;
    }

    let Some(span_text) = src.get(diagnostic.span.start..diagnostic.span.end) else {
        return;
    };
    if !span_text.starts_with('\\') {
        return;
    }

    let Some(argument_span) = find_argument_surface_span(
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.end,
    ) else {
        return;
    };
    diagnostic.span = argument_span;
}

fn find_argument_surface_span(tokens: &LexedSource, after: usize) -> Option<Span> {
    let mut index = 0;
    while index < tokens.len() && tokens[index].1.end <= after {
        index += 1;
    }
    while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
        index += 1;
    }

    let (token, span) = tokens.get(index)?;

    match token {
        Token::LBracket => {
            let mut brace_depth = 0usize;
            let mut bracket_depth = 0usize;
            let start = span.start;
            for (token, span) in tokens.iter().skip(index + 1) {
                match token {
                    Token::LBracket if brace_depth == 0 => bracket_depth += 1,
                    Token::RBracket if brace_depth == 0 => {
                        if bracket_depth == 0 {
                            return Some(Span {
                                start,
                                end: span.end,
                            });
                        }
                        bracket_depth -= 1;
                    }
                    Token::LBrace => brace_depth += 1,
                    Token::RBrace if brace_depth > 0 => brace_depth -= 1,
                    _ => {}
                }
            }
            None
        }
        Token::LBrace => {
            let mut depth = 0usize;
            let start = span.start;
            for (token, span) in tokens.iter().skip(index + 1) {
                match token {
                    Token::LBrace => depth += 1,
                    Token::RBrace => {
                        if depth == 0 {
                            return Some(Span {
                                start,
                                end: span.end,
                            });
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

fn find_invalid_left_context(
    ctx: &ParseContext,
    tokens: &LexedSource,
) -> Option<(Span, Option<Span>)> {
    let mut environment_stack = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        match &tokens[index].0 {
            Token::ControlSeq(name) if name == "begin" => {
                environment_stack.push(environment_body_start(tokens, index));
            }
            Token::ControlSeq(name) if name == "end" => {
                environment_stack.pop();
            }
            Token::ControlSeq(name) if name == "left" => {
                let mut next = index + 1;
                while matches!(tokens.get(next), Some((Token::Whitespaces, _))) {
                    next += 1;
                }

                let Some((token, token_span)) = tokens.get(next) else {
                    let left_span = Span {
                        start: tokens[index].1.start,
                        end: tokens[index].1.end,
                    };
                    let env_span = environment_stack.last().map(|start| Span {
                        start: *start,
                        end: left_span.end,
                    });
                    return Some((left_span, env_span));
                };

                let is_valid_delimiter = match token {
                    Token::Char(c) => ctx
                        .lookup_delimiter(c.to_string().as_str(), false, ContentMode::Math)
                        .is_some(),
                    Token::LBracket => ctx
                        .lookup_delimiter("[", false, ContentMode::Math)
                        .is_some(),
                    Token::RBracket => ctx
                        .lookup_delimiter("]", false, ContentMode::Math)
                        .is_some(),
                    Token::ControlSeq(name) => ctx
                        .lookup_delimiter(name.as_str(), true, ContentMode::Math)
                        .is_some(),
                    _ => false,
                };

                if !is_valid_delimiter {
                    let left_span = Span {
                        start: tokens[index].1.start,
                        end: token_span.end,
                    };
                    let env_span = environment_stack.last().map(|start| Span {
                        start: *start,
                        end: token_span.end,
                    });
                    return Some((left_span, env_span));
                }
            }
            _ => {}
        }

        index += 1;
    }

    None
}

fn find_environment_name_mismatch(
    tokens: &LexedSource,
    target_span: Span,
) -> Option<(String, String, Span)> {
    let mut stack = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        let Some((Token::ControlSeq(head), _)) = tokens.get(index) else {
            index += 1;
            continue;
        };

        if !matches!(head.as_str(), "begin" | "end") {
            index += 1;
            continue;
        }

        let mut next = index + 1;
        while matches!(tokens.get(next), Some((Token::Whitespaces, _))) {
            next += 1;
        }
        if !matches!(tokens.get(next), Some((Token::LBrace, _))) {
            index += 1;
            continue;
        }
        next += 1;

        let mut env_name = String::new();
        while let Some((token, _)) = tokens.get(next) {
            match token {
                Token::Char(c) => env_name.push(*c),
                Token::Star => env_name.push('*'),
                Token::RBrace => break,
                _ => {
                    env_name.clear();
                    break;
                }
            }
            next += 1;
        }

        if env_name.is_empty() {
            index += 1;
            continue;
        }

        if head == "begin" {
            stack.push(env_name);
        } else if let Some(expected) = stack.last() {
            if expected == &env_name {
                stack.pop();
            } else {
                let mismatch_closer_span = Span {
                    start: tokens[next].1.start,
                    end: tokens[next].1.end,
                };
                if mismatch_closer_span.start != target_span.start
                    || mismatch_closer_span.end != target_span.end
                {
                    index += 1;
                    continue;
                }

                return Some((
                    expected.clone(),
                    env_name,
                    Span {
                        start: tokens[index].1.start,
                        end: tokens[next].1.end,
                    },
                ));
            }
        }

        index += 1;
    }

    None
}

fn find_unknown_environment_at_span(
    ctx: &ParseContext,
    tokens: &LexedSource,
    target_span: Span,
) -> Option<(String, Span)> {
    let mut index = 0;
    while index < tokens.len() {
        let Some((Token::ControlSeq(name), begin_span)) = tokens.get(index) else {
            index += 1;
            continue;
        };

        if name != "begin"
            || begin_span.start != target_span.start
            || begin_span.end != target_span.end
        {
            index += 1;
            continue;
        }

        index += 1;
        while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
            index += 1;
        }

        let Some((Token::LBrace, _)) = tokens.get(index) else {
            return None;
        };
        index += 1;

        let name_start = tokens.get(index)?.1.start;
        let mut parsed_name = String::new();
        let mut name_end = name_start;
        while let Some((token, span)) = tokens.get(index) {
            match token {
                Token::Char(ch) => {
                    parsed_name.push(*ch);
                    name_end = span.end;
                    index += 1;
                }
                Token::Star => {
                    parsed_name.push('*');
                    name_end = span.end;
                    index += 1;
                }
                Token::RBrace => break,
                _ => return None,
            }
        }

        if parsed_name.is_empty() || ctx.knows_env_name(parsed_name.as_str()) {
            return None;
        }

        return Some((
            parsed_name,
            Span {
                start: name_start,
                end: name_end,
            },
        ));
    }

    None
}

fn find_first_known_but_disallowed_environment(
    ctx: &ParseContext,
    tokens: &LexedSource,
) -> Option<(String, ContentMode, Span)> {
    let mut index = 0;
    while index < tokens.len() {
        let Some((Token::ControlSeq(name), head_span)) = tokens.get(index) else {
            index += 1;
            continue;
        };
        if name != "begin" {
            index += 1;
            continue;
        }

        let begin_start = head_span.start;
        index += 1;
        while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
            index += 1;
        }
        if !matches!(tokens.get(index), Some((Token::LBrace, _))) {
            continue;
        }
        index += 1;

        let mut parsed_name = String::new();
        while let Some((token, _)) = tokens.get(index) {
            match token {
                Token::Char(ch) => {
                    parsed_name.push(*ch);
                    index += 1;
                }
                Token::Star => {
                    parsed_name.push('*');
                    index += 1;
                }
                Token::RBrace => break,
                _ => return None,
            }
        }

        let Some((Token::RBrace, close_span)) = tokens.get(index) else {
            return None;
        };
        if parsed_name.is_empty() {
            index += 1;
            continue;
        }

        let math_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Math)
            .is_some();
        let text_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Text)
            .is_some();
        let disallowed_mode = match (math_known, text_known) {
            (false, true) => ContentMode::Math,
            (true, false) => ContentMode::Text,
            _ => {
                index += 1;
                continue;
            }
        };

        return Some((
            parsed_name,
            disallowed_mode,
            Span {
                start: begin_start,
                end: close_span.end,
            },
        ));
    }

    None
}

fn find_environment_mode_error_at_span(
    ctx: &ParseContext,
    tokens: &LexedSource,
    target_span: Span,
) -> Option<(String, ContentMode, Span)> {
    let mut index = 0;
    while index < tokens.len() {
        let Some((Token::ControlSeq(name), _)) = tokens.get(index) else {
            index += 1;
            continue;
        };
        if name != "begin" {
            index += 1;
            continue;
        }

        let begin_start = tokens[index].1.start;
        index += 1;
        while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
            index += 1;
        }
        if !matches!(tokens.get(index), Some((Token::LBrace, _))) {
            continue;
        }
        index += 1;

        let mut parsed_name = String::new();
        while let Some((token, _)) = tokens.get(index) {
            match token {
                Token::Char(ch) => {
                    parsed_name.push(*ch);
                    index += 1;
                }
                Token::Star => {
                    parsed_name.push('*');
                    index += 1;
                }
                Token::RBrace => break,
                _ => return None,
            }
        }

        let Some((Token::RBrace, close_span)) = tokens.get(index) else {
            return None;
        };

        let matches_target =
            close_span.start == target_span.start || close_span.end == target_span.end;
        if !matches_target || parsed_name.is_empty() {
            index += 1;
            continue;
        }

        let math_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Math)
            .is_some();
        let text_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Text)
            .is_some();
        let disallowed_mode = match (math_known, text_known) {
            (false, true) => ContentMode::Math,
            (true, false) => ContentMode::Text,
            _ => return None,
        };

        return Some((
            parsed_name,
            disallowed_mode,
            Span {
                start: begin_start,
                end: close_span.end,
            },
        ));
    }

    None
}

fn environment_body_start(tokens: &[(Token, std::ops::Range<usize>)], begin_index: usize) -> usize {
    let mut index = begin_index + 1;
    while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
        index += 1;
    }

    if !matches!(tokens.get(index), Some((Token::LBrace, _))) {
        return tokens[begin_index].1.start;
    }
    index += 1;

    while let Some((token, span)) = tokens.get(index) {
        if matches!(token, Token::RBrace) {
            return span.end;
        }
        index += 1;
    }

    tokens[begin_index].1.start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eof_unclosed_inline_math_is_normalized() {
        let expected = vec!["something else".to_string(), "'$'".to_string()];
        let mut diagnostic = ParseDiagnostic {
            kind: Some(ParseDiagnosticKind::UnclosedInlineMath),
            message: "found end of input expected something else, or '$'".to_string(),
            span: Span { start: 0, end: 2 },
            expected,
            found: None,
            contexts: Vec::new(),
        };

        supplement_diagnostic_contexts(
            &ParseContext::empty(),
            "$x",
            Some(ParseDiagnosticKind::UnclosedInlineMath),
            false,
            false,
            &mut diagnostic,
        );

        assert_eq!(
            diagnostic.message,
            "found '$' expected something else, or end of input"
        );
        assert_eq!(diagnostic.expected, ["something else", "end of input"]);
        assert_eq!(diagnostic.found, None);
    }

    #[test]
    fn argument_validation_span_uses_kind_not_message() {
        let mut diagnostic = ParseDiagnostic {
            kind: Some(ParseDiagnosticKind::ArgumentValidation),
            message: "argument value was rejected".to_string(),
            span: Span { start: 0, end: 7 },
            expected: Vec::new(),
            found: None,
            contexts: Vec::new(),
        };

        supplement_diagnostic_contexts(
            &ParseContext::empty(),
            "\\hspace{bad}",
            Some(ParseDiagnosticKind::ArgumentValidation),
            false,
            false,
            &mut diagnostic,
        );

        assert_eq!(diagnostic.span, Span { start: 7, end: 12 });
    }
    #[test]
    fn direct_positions_and_names_do_not_depend_on_message_or_source_search() {
        use super::super::error::custom_error;
        let ctx = ParseContext::empty();
        let error = custom_error(
            (0..1).into(),
            "reworded command error",
            ParseDiagnosticKind::CommandModeError,
        )
        .at_source((7..12).into());
        let (_, diagnostic) = convert_diagnostic(&ctx, "unrelated source", error);
        assert_eq!(diagnostic.span, Span { start: 7, end: 12 });
        assert_eq!(diagnostic.message, "reworded command error");
        let error = custom_error(
            (0..1).into(),
            "reworded mismatch",
            ParseDiagnosticKind::EnvironmentNameMismatch,
        )
        .at_environment((7..12).into(), "align".into(), "matrix".into());
        let (_, diagnostic) = convert_diagnostic(&ctx, "unrelated source", error);
        assert_eq!(diagnostic.span, Span { start: 7, end: 12 });
        assert_eq!(diagnostic.message, "reworded mismatch");
        assert_eq!(diagnostic.expected, [r"\end{matrix}"]);
        assert_eq!(diagnostic.found.as_deref(), Some(r"\end{align}"));
    }
}
