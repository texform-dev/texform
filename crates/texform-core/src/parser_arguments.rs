//! Argument parsing for commands and environments.
//!
//! Each argument slot in a command spec ([`ArgSpec`]) describes a form (standard,
//! star, group, delimited, paired) and a value kind (content, delimiter,
//! dimension, integer, key-value, column, CS-name). The [`argument_parser`]
//! function dispatches on form × kind to build the appropriate chumsky parser,
//! while [`arguments_parser`] sequences them for a full argument list.
//!
//! Delimited and paired forms collect raw tokens between the matched
//! delimiters, then re-parse them as a sub-stream. This two-phase approach
//! avoids exposing delimiter nesting to the main combinator graph.

use chumsky::{label::LabelError, prelude::*};

use crate::column_parser::parse_column_template;
use crate::dimension::is_valid_dimension_unit;
use crate::knowledge::{ArgForm, ArgSpec, DelimiterToken, ValueKind};
use crate::lexer::Token;
use crate::parse::{ParseContext, ParseDiagnosticKind};
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind,
    SyntaxNode,
};

use super::{
    ArgumentParser, ContentParser, ParserError, ParserInput, ParserInputExt, TokenStream,
    TrackedNode, build_token_stream, content_block_parser_with_source, delimiter,
    insignificant_whitespace, math_item_parser, maybe_braced, maybe_braced_or_empty,
    optional_bracketed, optional_bracketed_or_empty, shift_owned_rich_span, text_item_parser,
};

/// Parsed argument slot bundled with span metadata for node-span tracking.
///
/// - `slot`: the public argument value (unchanged from before tracking)
/// - `span`: byte range of the full argument consumption (including delimiters)
/// - `content`: tracked content subtree for `arg.N.content` paths (content args only)
#[derive(Debug, Clone)]
pub(crate) struct TrackedArgumentSlot {
    pub slot: ArgumentSlot,
    pub span: Option<SimpleSpan>,
    pub content: Option<TrackedNode>,
}

impl TrackedArgumentSlot {
    /// Wrap a plain slot with no tracking metadata.
    fn untracked(slot: ArgumentSlot) -> Self {
        Self {
            slot,
            span: None,
            content: None,
        }
    }

    /// Wrap a slot with span but no content subtree (non-content arguments).
    fn with_span(slot: ArgumentSlot, span: SimpleSpan) -> Self {
        Self {
            slot,
            span: Some(span),
            content: None,
        }
    }
}

/// Check whether a lexed token matches a spec-defined delimiter token.
fn token_matches_delimiter(token: &Token, delimiter: &DelimiterToken) -> bool {
    match delimiter {
        DelimiterToken::Char('{') => matches!(token, Token::LBrace),
        DelimiterToken::Char('}') => matches!(token, Token::RBrace),
        DelimiterToken::Char('[') => matches!(token, Token::LBracket),
        DelimiterToken::Char(']') => matches!(token, Token::RBracket),
        DelimiterToken::Char(c) => matches!(token, Token::Char(tc) if *tc == *c),
        DelimiterToken::ControlSeq(name) => {
            matches!(token, Token::ControlSeq(token_name) if token_name == name.as_ref())
        }
    }
}

/// Convert a spec-level [`DelimiterToken`] to a syntax-tree [`Delimiter`].
fn syntax_delimiter(delimiter: &'static DelimiterToken) -> Delimiter {
    match delimiter {
        DelimiterToken::Char(c) => Delimiter::Char(*c),
        DelimiterToken::ControlSeq(name) => Delimiter::Control(name.as_ref()),
    }
}

/// Consume tokens between matched `open` and `close` delimiters.
///
/// When `open != close`, nesting is tracked so that inner pairs are collected
/// as part of the content. When `open == close` (e.g. `|…|`), nesting is
/// disabled and the first closing delimiter ends collection.
fn collect_delimited_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    open: &DelimiterToken,
    close: &DelimiterToken,
) -> Result<Vec<Token>, Rich<'src, Token>> {
    let start = input.cursor();
    let next = match input.peek() {
        Some(token) => token,
        None => return Err(input.err_since(&start, "expected delimited argument")),
    };
    if !token_matches_delimiter(&next, open) {
        return Err(input.err_since(&start, "missing opening delimiter"));
    }
    input.next();

    let allow_nested = open != close;
    let mut depth = 0usize;
    let mut tokens = Vec::new();

    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed delimited argument")),
        };

        if allow_nested && token_matches_delimiter(&token, open) {
            depth += 1;
            tokens.push(token);
            continue;
        }

        if token_matches_delimiter(&token, close) {
            if allow_nested && depth > 0 {
                depth -= 1;
                tokens.push(token);
                continue;
            }
            break;
        }

        tokens.push(token);
    }

    Ok(tokens)
}

/// Re-parse a collected token sequence as a full content sub-expression.
///
/// The tokens are serialized back to a string, re-lexed, and fed through
/// the appropriate mode parser. This two-phase approach isolates delimiter
/// nesting from the main combinator graph.
///
/// Re-parse a delimited content slice in isolation and return both the best
/// recoverable tree and the diagnostics that still need to surface at the top.
///
/// Keeping the diagnostics alongside the optional tree lets the caller decide
/// whether this subparse should stay recoverable or fall back to the outer
/// argument error path without re-running the parser.
fn parse_content_substream(
    ctx: &ParseContext,
    mode: ContentMode,
    tokens: &[Token],
    strict: bool,
    source_offset: usize,
) -> (Option<TrackedNode>, Vec<Rich<'static, Token>>) {
    let src = tokens_to_string(&tokens);
    let token_stream = build_token_stream(src.as_str());
    let parser = content_block_parser_with_source(mode, ctx, strict, src.as_str());

    let (tracked, errors) = parser
        .then_ignore(end())
        .parse(token_stream)
        .into_output_errors();

    let shifted_errors: Vec<_> = errors
        .into_iter()
        .map(|err| shift_owned_rich_span(err.into_owned(), source_offset))
        .collect();
    let diagnostics = filter_outer_errors(shifted_errors.clone(), source_offset + src.len());

    let (tracked, diagnostics) = if let Some(tracked) = tracked {
        (Some(tracked), diagnostics)
    } else if let Some((tracked, recover_diagnostics)) = recover_direct_error_substream(
        mode,
        ctx,
        strict,
        src.as_str(),
        source_offset,
        shifted_errors.as_slice(),
    ) {
        (Some(tracked), recover_diagnostics)
    } else {
        (None, diagnostics)
    };

    let tracked = tracked.map(|tracked| {
        normalize_content_subparse(mode, tracked.offset(source_offset))
            .with_diagnostics(diagnostics.clone())
    });

    (tracked, diagnostics)
}

// Retry the block parser without `end()` and only accept it when it surfaces a direct inner error.
fn recover_direct_error_substream(
    mode: ContentMode,
    ctx: &ParseContext,
    strict: bool,
    src: &str,
    source_offset: usize,
    shifted_errors: &[Rich<'static, Token>],
) -> Option<(TrackedNode, Vec<Rich<'static, Token>>)> {
    let recover_end = shifted_errors
        .iter()
        .filter(|err| {
            matches!(
                err.reason(),
                chumsky::error::RichReason::ExpectedFound { .. }
            )
        })
        .map(|err| err.span().start.saturating_sub(source_offset))
        .min()
        .unwrap_or(src.len());
    let recover_src = src.get(..recover_end).unwrap_or(src);
    let token_stream = build_token_stream(recover_src);
    let parser = content_block_parser_with_source(mode, ctx, strict, recover_src);
    let (tracked, errors) = parser.parse(token_stream).into_output_errors();
    let diagnostics: Vec<_> = errors
        .into_iter()
        .map(|err| shift_owned_rich_span(err.into_owned(), source_offset))
        .collect();

    let tracked_has_direct = tracked
        .as_ref()
        .is_some_and(|tracked| tracked.diagnostics.iter().any(is_direct_custom_error));

    if !tracked_has_direct && !diagnostics.iter().any(is_direct_custom_error) {
        return None;
    }

    tracked.map(|tracked| (tracked, diagnostics))
}

/// Drop only the synthetic `then_ignore(end())` tail error when the subparse
/// already produced a more specific direct diagnostic for the same content.
fn filter_outer_errors(
    diagnostics: Vec<Rich<'static, Token>>,
    subparse_end: usize,
) -> Vec<Rich<'static, Token>> {
    let first_direct_start = diagnostics
        .iter()
        .filter(|err| is_direct_custom_error(err))
        .map(|err| err.span().start)
        .min();

    diagnostics
        .into_iter()
        .filter(|err| !is_trailing_outer_error(err, subparse_end, first_direct_start))
        .collect()
}

/// Only custom diagnostics that describe a real inner parse failure should
/// suppress the trailing outer `ExpectedFound` wrapper error.
fn is_direct_custom_error(err: &Rich<'static, Token>) -> bool {
    match err.reason() {
        chumsky::error::RichReason::Custom(message) => {
            let (_, message) = ParseDiagnosticKind::split_message(message.as_str());
            !matches!(
                message,
                "not a command" | "unknown" | "content recovery must consume at least one token"
            )
        }
        chumsky::error::RichReason::ExpectedFound { .. } => false,
    }
}

/// Keep recoverable error nodes in sync with the public diagnostic wording for
/// inner generic content failures that get normalized later during conversion.
fn normalized_inner_generic_message(err: &Rich<'static, Token>) -> String {
    let message = format!("{err}");
    if matches!(
        message.as_str(),
        "found '$' expected something else, or '$'"
            | "found end of input expected something else, or '$'"
    ) {
        "found '$' expected something else, or end of input".to_string()
    } else {
        message
    }
}

/// Rebuild the inner generic parse error in the caller lifetime without
/// degrading it to a custom message-only diagnostic.
fn rebuild_generic_expected_found<'src>(err: &Rich<'static, Token>) -> Rich<'src, Token> {
    match err.reason() {
        chumsky::error::RichReason::ExpectedFound { expected, found } => {
            <Rich<'src, Token> as LabelError<
                'src,
                TokenStream<'src>,
                chumsky::error::RichPattern<'src, Token>,
            >>::expected_found(expected.iter().cloned(), found.clone(), *err.span())
        }
        chumsky::error::RichReason::Custom(_) => unreachable!(),
    }
}

// Rebuild an owned inner error in the caller lifetime without changing its span or contexts.
fn rebuild_owned_rich<'src>(err: &Rich<'static, Token>) -> Rich<'src, Token> {
    let mut rebuilt = match err.reason() {
        chumsky::error::RichReason::Custom(message) => Rich::custom(*err.span(), message.clone()),
        chumsky::error::RichReason::ExpectedFound { .. } => rebuild_generic_expected_found(err),
    };

    for (label, span) in err.contexts() {
        <Rich<'src, Token> as LabelError<'src, TokenStream<'src>, String>>::in_context(
            &mut rebuilt,
            label.to_string(),
            *span,
        );
    }

    rebuilt
}

/// Keep this predicate intentionally narrow so we do not swallow generic parse
/// errors that still carry useful information away from the subparse tail.
fn is_trailing_outer_error(
    err: &Rich<'static, Token>,
    subparse_end: usize,
    first_direct_start: Option<usize>,
) -> bool {
    let Some(first_direct_start) = first_direct_start else {
        return false;
    };

    if !matches!(
        err.reason(),
        chumsky::error::RichReason::ExpectedFound { .. }
    ) {
        return false;
    }

    let span = err.span();
    span.end == subparse_end && span.start > first_direct_start
}

/// Content argument parsing wants the inner items, not the block parser's
/// wrapper group, but the aggregated diagnostics still belong to the returned
/// content node after the wrapper is stripped away.
fn normalize_content_subparse(mode: ContentMode, tracked: TrackedNode) -> TrackedNode {
    let content_span = tracked.span;
    let existing_diagnostics = tracked.diagnostics.clone();

    let normalized = match tracked.node {
        SyntaxNode::Group { children, .. } => {
            let mut items = Vec::with_capacity(children.len());
            for (i, child_node) in children.into_iter().enumerate() {
                let prefix = format!("child.{i}");
                let child_span = tracked
                    .records
                    .iter()
                    .find(|r| r.path == prefix)
                    .map(|r| r.span)
                    .unwrap_or(content_span);
                let child_records: Vec<super::RelativeSpanEntry> = tracked
                    .records
                    .iter()
                    .filter_map(|r| {
                        r.path.strip_prefix(&format!("{prefix}.")).map(|suffix| {
                            super::RelativeSpanEntry {
                                path: suffix.to_string(),
                                span: r.span,
                            }
                        })
                    })
                    .collect();
                items.push(TrackedNode {
                    node: child_node,
                    span: child_span,
                    records: child_records,
                    diagnostics: Vec::new(),
                });
            }
            TrackedNode::fold(mode, items, content_span)
        }
        other => TrackedNode::leaf(other, content_span),
    };

    normalized.with_diagnostics(existing_diagnostics)
}

/// Convert a collected token slice into an argument value while preserving the
/// most specific inner diagnostic we have. The generic outer argument error is
/// only a last resort once the subparse produced neither a recoverable tree nor
/// a useful direct diagnostic.
fn parse_tokens_as_content<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    ctx: &'parse ParseContext,
    mode: ContentMode,
    tokens: Vec<Token>,
    strict: bool,
    source_offset: usize,
) -> Result<TrackedNode, Rich<'src, Token>> {
    let (content, diagnostics) = parse_content_substream(ctx, mode, &tokens, strict, source_offset);
    if let Some(content) = content {
        return Ok(content);
    }

    let propagated_error = diagnostics
        .iter()
        .filter(|err| is_direct_custom_error(err))
        .max_by_key(|err| {
            let span = err.span();
            (span.end, span.start)
        });

    if let Some(inner_error) = propagated_error {
        return Err(rebuild_owned_rich(inner_error));
    }

    let generic_error = diagnostics
        .iter()
        .filter(|err| {
            matches!(
                err.reason(),
                chumsky::error::RichReason::ExpectedFound { .. }
            )
        })
        .max_by_key(|err| {
            let span = err.span();
            (span.end, span.start)
        });

    if let Some(generic_error) = generic_error {
        if strict {
            return Err(rebuild_owned_rich(generic_error));
        }

        let snippet = tokens_to_string(tokens.as_slice());
        let span = SimpleSpan::new((), source_offset..source_offset + snippet.len());
        return Ok(TrackedNode::leaf(
            SyntaxNode::Error {
                message: normalized_inner_generic_message(generic_error),
                snippet,
            },
            span,
        )
        .with_diagnostics(diagnostics));
    }

    let cursor = input.cursor();
    Err(input.err_peek_or_point(&cursor, "failed to parse delimited argument content"))
}

/// Parse collected tokens into the [`ArgumentValue`] dictated by `kind`.
///
/// Dispatches to content parsing, CS-name extraction, dimension/integer
/// combinators, key-value validation, column-spec validation, or delimiter
/// parsing depending on the spec.
fn parse_delimited_value<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    ctx: &'parse ParseContext,
    kind: ValueKind,
    tokens: Vec<Token>,
    strict: bool,
    nullable: bool,
) -> Result<ArgumentValue, Rich<'src, Token>> {
    match kind {
        ValueKind::Content { mode } => {
            let content = parse_tokens_as_content(input, ctx, mode, tokens, strict, 0)?;
            Ok(argument_content_value(mode, content.node))
        }
        ValueKind::CSName => {
            if nullable && tokens.iter().all(|t| matches!(t, Token::Whitespaces)) {
                return Ok(ArgumentValue::CSName(String::new()));
            }
            let value = parse_tokens_as_cs_name(input, &tokens)?;
            Ok(ArgumentValue::CSName(value))
        }
        ValueKind::Dimension => {
            let src = tokens_to_string(&tokens);
            if nullable && src.trim().is_empty() {
                return Ok(ArgumentValue::Dimension(String::new()));
            }
            let value = insignificant_whitespace()
                .ignore_then(dimension())
                .then_ignore(insignificant_whitespace())
                .then_ignore(end())
                .parse(build_token_stream(src.as_str()))
                .into_result()
                .map_err(|_| {
                    let cursor = input.cursor();
                    input.err_peek_or_point(&cursor, "invalid dimension argument")
                })?;
            Ok(ArgumentValue::Dimension(value))
        }
        ValueKind::Integer => {
            let src = tokens_to_string(&tokens);
            if nullable && src.trim().is_empty() {
                return Ok(ArgumentValue::Integer(String::new()));
            }
            let value = insignificant_whitespace()
                .ignore_then(integer())
                .then_ignore(insignificant_whitespace())
                .then_ignore(end())
                .parse(build_token_stream(src.as_str()))
                .into_result()
                .map_err(|_| {
                    let cursor = input.cursor();
                    input.err_peek_or_point(&cursor, "invalid integer argument")
                })?;
            Ok(ArgumentValue::Integer(value))
        }
        ValueKind::KeyVal => {
            let raw = tokens_to_string(&tokens);
            if nullable && raw.trim().is_empty() {
                return Ok(ArgumentValue::KeyVal(String::new()));
            }
            validate_keyval(raw.as_str()).map_err(|msg| {
                let cursor = input.cursor();
                input.err_peek_or_point(&cursor, msg)
            })?;
            Ok(ArgumentValue::KeyVal(raw.trim().to_string()))
        }
        ValueKind::Column => {
            let raw = tokens_to_string(&tokens);
            let normalized = raw.trim().to_string();
            parse_column_template(normalized.as_str()).map_err(|msg| {
                let cursor = input.cursor();
                input.err_peek_or_point(&cursor, msg.to_string())
            })?;
            Ok(ArgumentValue::Column(normalized))
        }
        ValueKind::Delimiter => {
            let src = tokens_to_string(&tokens);
            if nullable && src.trim().is_empty() {
                return Ok(ArgumentValue::Delimiter(Delimiter::None));
            }
            let value = insignificant_whitespace()
                .ignore_then(delimiter(ctx))
                .then_ignore(insignificant_whitespace())
                .then_ignore(end())
                .parse(build_token_stream(src.as_str()))
                .into_result()
                .map_err(|_| {
                    let cursor = input.cursor();
                    input.err_peek_or_point(&cursor, "invalid delimiter argument")
                })?;
            Ok(ArgumentValue::Delimiter(value))
        }
        ValueKind::Star => {
            let cursor = input.cursor();
            Err(input.err_peek_or_point(
                &cursor,
                "invalid spec: star kind is not supported by delimited/paired forms",
            ))
        }
    }
}

fn argument_content_value(mode: ContentMode, node: SyntaxNode) -> ArgumentValue {
    match mode {
        ContentMode::Math => ArgumentValue::MathContent(node),
        ContentMode::Text => ArgumentValue::TextContent(node),
    }
}

fn parse_tokens_as_cs_name<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    tokens: &[Token],
) -> Result<String, Rich<'src, Token>> {
    if tokens
        .iter()
        .any(|token| matches!(token, Token::ControlSeq(_)))
    {
        let cursor = input.cursor();
        return Err(input.err_peek_or_point(&cursor, "escape sequence should not appear in CSName"));
    }

    Ok(tokens_to_string(tokens))
}

/// Build a parser for a single argument slot described by `spec`.
///
/// Returns a `TrackedArgumentSlot` that bundles the public `ArgumentSlot`,
/// its source span, and (for content arguments) a tracked content subtree
/// so that callers can expose `arg.N` / `arg.N.content` paths.
pub(super) fn argument_parser<'a>(
    ctx: &'a ParseContext,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    spec: &'static ArgSpec,
    strict: bool,
) -> ArgumentParser<'a> {
    custom(move |input| {
        let arg_start = input.cursor();

        match &spec.form {
            ArgForm::Standard => match spec.kind {
                ValueKind::Content { mode } => {
                    if spec.required {
                        if matches!(input.peek(), Some(Token::LBrace)) {
                            // Braced content: {abc}
                            let tokens = collect_braced_tokens(input, true)?;
                            let arg_span = input.span_from_cursor(&arg_start);
                            let content_offset = arg_span.start + 1; // skip opening brace
                            let content = parse_tokens_as_content(
                                input,
                                ctx,
                                mode,
                                tokens,
                                strict,
                                content_offset,
                            )?;
                            Ok(TrackedArgumentSlot {
                                slot: Some(Argument::from_value(
                                    ArgumentKind::Mandatory,
                                    argument_content_value(mode, content.node.clone()),
                                )),
                                span: Some(arg_span),
                                content: Some(content),
                            })
                        } else {
                            // Shorthand single item: \frac 1 2
                            let item: TrackedNode = match mode {
                                ContentMode::Math => input.parse(
                                    math_item_parser(
                                        ctx,
                                        math_content.clone(),
                                        text_content.clone(),
                                        strict,
                                    )
                                    .labelled("mandatory argument")
                                    .as_context(),
                                )?,
                                ContentMode::Text => input.parse(
                                    text_item_parser(
                                        ctx,
                                        math_content.clone(),
                                        text_content.clone(),
                                        strict,
                                    )
                                    .labelled("mandatory argument")
                                    .as_context(),
                                )?,
                            };
                            let arg_span = input.span_from_cursor(&arg_start);
                            let content = TrackedNode {
                                node: normalize_argument_value(mode, item.node.clone()),
                                span: item.span,
                                records: item.records,
                                diagnostics: item.diagnostics,
                            };
                            Ok(TrackedArgumentSlot {
                                slot: Some(Argument::from_value(
                                    ArgumentKind::Mandatory,
                                    argument_content_value(mode, content.node.clone()),
                                )),
                                span: Some(arg_span),
                                content: Some(content),
                            })
                        }
                    } else if !matches!(input.peek(), Some(Token::LBracket)) {
                        Ok(TrackedArgumentSlot::untracked(None))
                    } else {
                        let Some(tokens) = collect_optional_bracketed_tokens(input, false)? else {
                            return Ok(TrackedArgumentSlot::untracked(None));
                        };
                        let arg_span = input.span_from_cursor(&arg_start);
                        let content_offset = arg_span.start + 1; // skip opening bracket
                        let content = parse_tokens_as_content(
                            input,
                            ctx,
                            mode,
                            tokens,
                            strict,
                            content_offset,
                        )?;
                        Ok(TrackedArgumentSlot {
                            slot: Some(Argument::from_value(
                                ArgumentKind::Optional,
                                argument_content_value(mode, content.node.clone()),
                            )),
                            span: Some(arg_span),
                            content: Some(content),
                        })
                    }
                }
                ValueKind::Delimiter => {
                    if spec.required {
                        let parser = if spec.nullable {
                            maybe_braced_or_empty(delimiter(ctx), Delimiter::None).boxed()
                        } else {
                            maybe_braced(delimiter(ctx)).boxed()
                        }
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::Delimiter(value),
                            ))
                        });
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    } else if !matches!(input.peek(), Some(Token::LBracket)) {
                        Ok(TrackedArgumentSlot::untracked(None))
                    } else {
                        let parser = if spec.nullable {
                            optional_bracketed_or_empty(delimiter(ctx), Delimiter::None).boxed()
                        } else {
                            optional_bracketed(delimiter(ctx)).boxed()
                        }
                        .map(move |opt| {
                            opt.map(|value| {
                                Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::Delimiter(value),
                                )
                            })
                        });
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    }
                }
                ValueKind::Dimension => {
                    if spec.required {
                        let parser = if spec.nullable {
                            maybe_braced_or_empty(dimension(), String::new()).boxed()
                        } else {
                            maybe_braced(dimension()).boxed()
                        }
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::Dimension(value),
                            ))
                        });
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    } else if !matches!(input.peek(), Some(Token::LBracket)) {
                        Ok(TrackedArgumentSlot::untracked(None))
                    } else {
                        let parser = if spec.nullable {
                            optional_bracketed_or_empty(dimension(), String::new()).boxed()
                        } else {
                            optional_bracketed(dimension()).boxed()
                        }
                        .map(move |opt| {
                            opt.map(|value| {
                                Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::Dimension(value),
                                )
                            })
                        });
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    }
                }
                ValueKind::Integer => {
                    if spec.required {
                        let parser = if spec.nullable {
                            maybe_braced_or_empty(integer(), String::new()).boxed()
                        } else {
                            maybe_braced(integer()).boxed()
                        }
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::Integer(value),
                            ))
                        });
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    } else if !matches!(input.peek(), Some(Token::LBracket)) {
                        Ok(TrackedArgumentSlot::untracked(None))
                    } else {
                        let parser = if spec.nullable {
                            optional_bracketed_or_empty(integer(), String::new()).boxed()
                        } else {
                            optional_bracketed(integer()).boxed()
                        }
                        .map(move |opt| {
                            opt.map(|value| {
                                Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::Integer(value),
                                )
                            })
                        });
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    }
                }
                ValueKind::KeyVal => {
                    if spec.required {
                        let nullable = spec.nullable;
                        let parser = keyval_value(true, nullable)
                            .map(move |value| {
                                Some(Argument::from_value(
                                    ArgumentKind::Mandatory,
                                    ArgumentValue::KeyVal(value),
                                ))
                            })
                            .boxed();
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    } else if !matches!(input.peek(), Some(Token::LBracket)) {
                        Ok(TrackedArgumentSlot::untracked(None))
                    } else {
                        let nullable = spec.nullable;
                        let parser = keyval_value(false, nullable)
                            .map(move |value| {
                                Some(Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::KeyVal(value),
                                ))
                            })
                            .boxed();
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    }
                }
                ValueKind::Column => {
                    if spec.required {
                        let parser = column_spec_value(true)
                            .map(move |value| {
                                Some(Argument::from_value(
                                    ArgumentKind::Mandatory,
                                    ArgumentValue::Column(value),
                                ))
                            })
                            .boxed();
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    } else if !matches!(input.peek(), Some(Token::LBracket)) {
                        Ok(TrackedArgumentSlot::untracked(None))
                    } else {
                        let parser = column_spec_value(false)
                            .map(move |value| {
                                Some(Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::Column(value),
                                ))
                            })
                            .boxed();
                        let slot = input.parse(parser)?;
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(slot, arg_span))
                    }
                }
                ValueKind::CSName => {
                    if spec.required {
                        let value = if matches!(input.peek(), Some(Token::LBrace)) {
                            let tokens = collect_delimited_tokens(
                                input,
                                &DelimiterToken::Char('{'),
                                &DelimiterToken::Char('}'),
                            )?;
                            if spec.nullable && tokens.is_empty() {
                                String::new()
                            } else {
                                parse_tokens_as_cs_name(input, &tokens)?
                            }
                        } else {
                            let cursor = input.cursor();
                            let token = input.next().ok_or_else(|| {
                                input.err_peek_or_point(&cursor, "missing required CSName argument")
                            })?;
                            parse_tokens_as_cs_name(input, std::slice::from_ref(&token))?
                        };
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::CSName(value),
                            )),
                            arg_span,
                        ))
                    } else if !matches!(input.peek(), Some(Token::LBracket)) {
                        Ok(TrackedArgumentSlot::untracked(None))
                    } else {
                        let Some(tokens) = collect_optional_bracketed_tokens(input, false)? else {
                            return Ok(TrackedArgumentSlot::untracked(None));
                        };
                        let value = if spec.nullable
                            && tokens.iter().all(|t| matches!(t, Token::Whitespaces))
                        {
                            String::new()
                        } else {
                            parse_tokens_as_cs_name(input, &tokens)?
                        };
                        let arg_span = input.span_from_cursor(&arg_start);
                        Ok(TrackedArgumentSlot::with_span(
                            Some(Argument::from_value(
                                ArgumentKind::Optional,
                                ArgumentValue::CSName(value),
                            )),
                            arg_span,
                        ))
                    }
                }
                ValueKind::Star => {
                    let cursor = input.cursor();
                    Err(input
                        .err_peek_or_point(&cursor, "invalid spec: star kind requires star form"))
                }
            },
            ArgForm::Star => {
                let present = matches!(input.peek(), Some(Token::Star));
                if present {
                    input.next();
                }
                let arg_span = input.span_from_cursor(&arg_start);
                Ok(TrackedArgumentSlot::with_span(
                    Some(Argument {
                        kind: ArgumentKind::Star,
                        value: ArgumentValue::Boolean(present),
                    }),
                    arg_span,
                ))
            }
            ArgForm::Group => {
                if !matches!(input.peek(), Some(Token::LBrace)) {
                    if spec.required {
                        let cursor = input.cursor();
                        return Err(input
                            .err_peek_or_point(&cursor, "missing required braced group argument"));
                    }
                    return Ok(TrackedArgumentSlot::untracked(None));
                }

                let tokens = collect_delimited_tokens(
                    input,
                    &DelimiterToken::Char('{'),
                    &DelimiterToken::Char('}'),
                )?;
                let arg_span = input.span_from_cursor(&arg_start);

                // For content-typed group args, track the content subtree.
                if let ValueKind::Content { mode } = spec.kind {
                    let content_offset = arg_span.start + 1;
                    let content =
                        parse_tokens_as_content(input, ctx, mode, tokens, strict, content_offset)?;
                    return Ok(TrackedArgumentSlot {
                        slot: Some(Argument::from_value(
                            ArgumentKind::Group,
                            argument_content_value(mode, content.node.clone()),
                        )),
                        span: Some(arg_span),
                        content: Some(content),
                    });
                }

                let value =
                    parse_delimited_value(input, ctx, spec.kind, tokens, strict, spec.nullable)?;
                Ok(TrackedArgumentSlot::with_span(
                    Some(Argument::from_value(ArgumentKind::Group, value)),
                    arg_span,
                ))
            }
            ArgForm::Delimited { open, close } => {
                let has_open =
                    matches!(input.peek(), Some(token) if token_matches_delimiter(&token, open));
                if !has_open {
                    if spec.required {
                        let cursor = input.cursor();
                        return Err(
                            input.err_peek_or_point(&cursor, "missing required delimited argument")
                        );
                    }
                    return Ok(TrackedArgumentSlot::untracked(None));
                }

                let tokens = collect_delimited_tokens(input, open, close)?;
                let arg_span = input.span_from_cursor(&arg_start);

                // For content-typed delimited args, track the content subtree.
                if let ValueKind::Content { mode } = spec.kind {
                    let open_len = delimiter_token_source_len(open);
                    let close_len = delimiter_token_source_len(close);
                    let content_offset = arg_span.start + open_len;
                    let content_span_end = arg_span.end.saturating_sub(close_len);
                    let _ = content_span_end; // content span is computed inside parse_tokens_as_content
                    let content =
                        parse_tokens_as_content(input, ctx, mode, tokens, strict, content_offset)?;
                    return Ok(TrackedArgumentSlot {
                        slot: Some(Argument::from_value(
                            ArgumentKind::Delimited {
                                open: syntax_delimiter(open),
                                close: syntax_delimiter(close),
                            },
                            argument_content_value(mode, content.node.clone()),
                        )),
                        span: Some(arg_span),
                        content: Some(content),
                    });
                }

                let value =
                    parse_delimited_value(input, ctx, spec.kind, tokens, strict, spec.nullable)?;
                Ok(TrackedArgumentSlot::with_span(
                    Some(Argument::from_value(
                        ArgumentKind::Delimited {
                            open: syntax_delimiter(open),
                            close: syntax_delimiter(close),
                        },
                        value,
                    )),
                    arg_span,
                ))
            }
            ArgForm::Paired { pairs } => {
                let matched = input.peek().and_then(|next| {
                    pairs
                        .iter()
                        .find(|(open, _)| token_matches_delimiter(&next, open))
                });

                let Some((open, close)) = matched else {
                    if spec.required {
                        let cursor = input.cursor();
                        return Err(
                            input.err_peek_or_point(&cursor, "missing required paired argument")
                        );
                    }
                    return Ok(TrackedArgumentSlot::untracked(None));
                };

                let tokens = collect_delimited_tokens(input, open, close)?;
                let arg_span = input.span_from_cursor(&arg_start);

                // For content-typed paired args, track the content subtree.
                if let ValueKind::Content { mode } = spec.kind {
                    let open_len = delimiter_token_source_len(open);
                    let content_offset = arg_span.start + open_len;
                    let content =
                        parse_tokens_as_content(input, ctx, mode, tokens, strict, content_offset)?;
                    return Ok(TrackedArgumentSlot {
                        slot: Some(Argument::from_value(
                            ArgumentKind::Paired {
                                open: syntax_delimiter(open),
                                close: syntax_delimiter(close),
                            },
                            argument_content_value(mode, content.node.clone()),
                        )),
                        span: Some(arg_span),
                        content: Some(content),
                    });
                }

                let value =
                    parse_delimited_value(input, ctx, spec.kind, tokens, strict, spec.nullable)?;
                Ok(TrackedArgumentSlot::with_span(
                    Some(Argument::from_value(
                        ArgumentKind::Paired {
                            open: syntax_delimiter(open),
                            close: syntax_delimiter(close),
                        },
                        value,
                    )),
                    arg_span,
                ))
            }
        }
    })
    .boxed()
}

/// Compute the source-text length of a delimiter token.
fn delimiter_token_source_len(delimiter: &DelimiterToken) -> usize {
    match delimiter {
        DelimiterToken::Char(_) => 1,
        DelimiterToken::ControlSeq(name) => name.len() + 1, // backslash + name
    }
}

/// Collect tokens inside an optional `[…]` argument.
///
/// Returns `None` if the next token is not `[`. When `match_brackets` is
/// true, inner `[…]` pairs at brace depth 0 are tracked for nesting;
/// otherwise the first `]` at brace depth 0 closes the argument (allowing
/// unbalanced brackets inside braces).
pub(crate) fn collect_optional_bracketed_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    match_brackets: bool,
) -> Result<Option<Vec<Token>>, Rich<'src, Token>> {
    if !matches!(input.peek(), Some(Token::LBracket)) {
        return Ok(None);
    }

    let start = input.cursor();
    input.next();

    let mut tokens = Vec::new();
    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;

    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed bracket argument")),
        };

        match token {
            Token::LBracket => {
                if match_brackets && brace_depth == 0 {
                    bracket_depth += 1;
                }
                tokens.push(Token::LBracket);
            }
            Token::RBracket => {
                if brace_depth == 0 {
                    if match_brackets && bracket_depth > 0 {
                        bracket_depth -= 1;
                        tokens.push(Token::RBracket);
                        continue;
                    }
                    break;
                }
                tokens.push(Token::RBracket);
            }
            Token::LBrace => {
                brace_depth += 1;
                tokens.push(Token::LBrace);
            }
            Token::RBrace => {
                if brace_depth == 0 {
                    return Err(input.err_since(&start, "unbalanced brace in bracket argument"));
                }
                brace_depth -= 1;
                tokens.push(Token::RBrace);
            }
            other => tokens.push(other),
        }
    }

    Ok(Some(tokens))
}

/// Consume tokens inside a mandatory `{…}` group.
///
/// When `allow_nested` is false, encountering a nested `{` is an error.
pub(crate) fn collect_braced_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    allow_nested: bool,
) -> Result<Vec<Token>, Rich<'src, Token>> {
    let start = input.cursor();
    match input.next() {
        Some(Token::LBrace) => {}
        _ => return Err(input.err_since(&start, "expected '{'")),
    }

    let mut tokens = Vec::new();
    let mut depth = 0usize;

    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed brace argument")),
        };

        match token {
            Token::LBrace => {
                if !allow_nested {
                    return Err(input.err_since(&start, "nested braces not allowed"));
                }
                depth += 1;
                tokens.push(Token::LBrace);
            }
            Token::RBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(Token::RBrace);
            }
            other => tokens.push(other),
        }
    }

    Ok(tokens)
}

/// Serialize a token sequence back into a LaTeX string for re-parsing.
fn tokens_to_string(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::ControlSeq(name) => {
                out.push('\\');
                out.push_str(name);
            }
            Token::Char(c) => out.push(*c),
            Token::Star => out.push('*'),
            Token::Alignment => out.push('&'),
            Token::MathShift => out.push('$'),
            Token::Parameter => out.push('#'),
            Token::Superscript => out.push('^'),
            Token::Subscript => out.push('_'),
            Token::Prime(count) => {
                for _ in 0..*count {
                    out.push('\'');
                }
            }
            Token::ActiveChar => out.push('~'),
            Token::LBracket => out.push('['),
            Token::RBracket => out.push(']'),
            Token::LBrace => out.push('{'),
            Token::RBrace => out.push('}'),
            Token::Whitespaces => out.push(' '),
            Token::Comment => {}
        }
    }
    out
}

/// Validate that `raw` is a well-formed `key=value,…` sequence.
///
/// Checks balanced braces, non-empty keys/values, and allows backslash
/// escapes inside both keys and values.
fn validate_keyval(raw: &str) -> Result<(), &'static str> {
    let mut key = String::new();
    let mut value = String::new();
    let mut in_value = false;
    let mut brace_depth = 0usize;

    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                let target = if in_value { &mut value } else { &mut key };
                target.push('\\');
                if let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphabetic() {
                        while let Some(c) = chars.peek().copied() {
                            if c.is_ascii_alphabetic() {
                                target.push(c);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    } else {
                        target.push(next);
                        chars.next();
                    }
                }
            }
            '{' => {
                brace_depth += 1;
                if in_value {
                    value.push('{');
                } else {
                    key.push('{');
                }
            }
            '}' => {
                if brace_depth == 0 {
                    return Err("unbalanced brace in keyval");
                }
                brace_depth -= 1;
                if in_value {
                    value.push('}');
                } else {
                    key.push('}');
                }
            }
            '=' if brace_depth == 0 && !in_value => {
                if key.trim().is_empty() {
                    return Err("keyval missing key");
                }
                in_value = true;
            }
            ',' if brace_depth == 0 && in_value => {
                if value.trim().is_empty() {
                    return Err("keyval missing value");
                }
                key.clear();
                value.clear();
                in_value = false;
            }
            ',' if brace_depth == 0 && !in_value => {
                return Err("keyval missing value");
            }
            '=' if brace_depth == 0 && in_value => {
                value.push('=');
            }
            other => {
                if in_value {
                    value.push(other);
                } else {
                    key.push(other);
                }
            }
        }
    }

    if brace_depth != 0 {
        return Err("unbalanced brace in keyval");
    }

    if !in_value {
        return Err("keyval missing value");
    }

    if value.trim().is_empty() {
        return Err("keyval missing value");
    }

    Ok(())
}

fn normalize_keyval_string(raw: &str) -> String {
    raw.trim().to_string()
}

fn integer<'a>() -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    let sign = select! { Token::Char(c @ ('+' | '-')) => c }.or_not();
    let digit = select! { Token::Char(c) if c.is_ascii_digit() => c };

    sign.then(digit.repeated().at_least(1).collect::<Vec<char>>())
        .map(|(sign, digits)| {
            let mut out = String::with_capacity(digits.len() + 1);
            if let Some(s) = sign {
                out.push(s);
            }
            for d in digits {
                out.push(d);
            }
            out
        })
        .labelled("integer")
}

fn dimension<'a>() -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    let sign = select! { Token::Char(c @ ('+' | '-')) => c }.or_not();
    let digit = select! { Token::Char(c) if c.is_ascii_digit() => c };
    let sep = select! { Token::Char(c @ ('.' | ',')) => c };
    let ws = insignificant_whitespace();
    let alpha = select! { Token::Char(c) if c.is_ascii_alphabetic() => c };
    let unit = alpha.repeated().at_least(1).collect::<Vec<char>>();

    let int_digits = digit.repeated().collect::<Vec<char>>();
    let frac = sep.then(digit.repeated().collect::<Vec<char>>());

    sign.then(int_digits)
        .then(frac.or_not())
        .then_ignore(ws)
        .then(unit)
        .try_map(|(((sign, int_digits), frac), unit_chars), span| {
            let has_int = !int_digits.is_empty();
            let has_frac = frac.as_ref().is_some_and(|(_, ds)| !ds.is_empty());
            if !has_int && !has_frac {
                return Err(Rich::custom(span, "invalid dimension"));
            }
            let unit: String = unit_chars.into_iter().collect();
            if !is_valid_dimension_unit(&unit) {
                return Err(Rich::custom(span, "unsupported dimension unit"));
            }
            let mut value = String::new();
            if let Some(s) = sign {
                value.push(s);
            }
            for d in &int_digits {
                value.push(*d);
            }
            if let Some((_, frac_digits)) = frac {
                value.push('.');
                for d in &frac_digits {
                    value.push(*d);
                }
            }
            Ok(format!("{}{}", value, unit))
        })
        .labelled("dimension")
}

fn keyval_value<'a>(
    required: bool,
    nullable: bool,
) -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let start = input.cursor();
        let raw = if required {
            if !matches!(input.peek(), Some(Token::LBrace)) {
                return Err(input.err_since(&start, "expected keyval argument"));
            }
            let tokens = collect_braced_tokens(input, true)?;
            tokens_to_string(&tokens)
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            return Ok(String::new());
        };

        if nullable && raw.trim().is_empty() {
            return Ok(String::new());
        }

        validate_keyval(&raw).map_err(|msg| {
            let span = input.span_from_cursor(&start);
            let mut err = Rich::custom(span, msg);
            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, &str>>::in_context(
                &mut err,
                "argument value",
                span,
            );
            err
        })?;

        Ok(normalize_keyval_string(&raw))
    })
}

fn column_spec_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let raw = if required {
            let start = input.cursor();
            if !matches!(input.peek(), Some(Token::LBrace)) {
                return Err(input.err_since(&start, "expected column argument"));
            }
            let tokens = collect_braced_tokens(input, true)?;
            tokens_to_string(&tokens)
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            String::new()
        };
        let normalized = raw.trim().to_string();

        parse_column_template(&normalized).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg.to_string())
        })?;

        Ok(normalized)
    })
}

/// Unwrap a single-child group into its inner node.
///
/// When a content argument is parsed, the result may be a group node
/// wrapping a single child. This function strips the unnecessary wrapper
/// so argument values are as flat as possible.
pub(crate) fn normalize_argument_value(mode: ContentMode, node: SyntaxNode) -> SyntaxNode {
    match node {
        SyntaxNode::Group { children, .. } => fold_items(mode, children),
        other => other,
    }
}

/// Fold a list of items into a single node: return as-is for one item,
/// wrap in an implicit group for zero or multiple items.
pub(crate) fn fold_items(mode: ContentMode, items: Vec<SyntaxNode>) -> SyntaxNode {
    match items.len() {
        0 => SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: vec![],
        },
        1 => items.into_iter().next().unwrap(),
        _ => SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: items,
        },
    }
}
