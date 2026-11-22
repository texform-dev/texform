//! Argument-slot parsing for commands and environments.
//!
//! Dispatches directly on `(ArgForm, ValueKind)`, collects balanced token
//! ranges without copying `Vec<Token>`, strips protective braces in place, and
//! parses content arguments via zero-copy sub-windows on the same
//! [`TokenParser`].

use std::borrow::Cow;
use std::ops::Range;

use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter, SyntaxNode,
};

use crate::argument_delimiters::{ContentBoundary, enclosing_braces};
use crate::knowledge::{ArgForm, ArgSpec, DelimiterToken, ValueKind};
use crate::lexer::Token;
use crate::parse::cursor::{BalancedScanError, DelimitedRange};
use crate::parse::diagnostics;
use crate::parse::parser::{PResult, ParseAbort, RecoveryHint, TokenParser};
use crate::parse::result::Span;
use crate::parse::tree::{self, ParsedNode, TrackedArgSlot};

use super::scalar;

/// Wrap a parsed content node in the [`ArgumentValue`] matching `kind`.
#[inline]
fn content_value_for_kind(kind: ValueKind, node: SyntaxNode) -> ArgumentValue {
    match kind {
        ValueKind::Content {
            mode: ContentMode::Math,
        } => ArgumentValue::MathContent(node),
        ValueKind::Content {
            mode: ContentMode::Text,
        } => ArgumentValue::TextContent(node),
        ValueKind::OperatorName => ArgumentValue::OperatorNameContent(node),
        _ => unreachable!("non-content value kind cannot carry a content node"),
    }
}

/// Strip one enclosing brace layer from `range` when it exists purely to
/// protect inner delimiters from terminating the surrounding argument.
fn strip_content_protection(
    tokens: &[Token<'_>],
    spans: &[Span],
    range: &mut Range<usize>,
    fallback_eoi: &mut usize,
    open: Option<Token<'_>>,
    close: Token<'_>,
) {
    let slice = &tokens[range.clone()];
    let Some(rel_inner) = enclosing_braces(slice) else {
        return;
    };
    if ContentBoundary::new(open, close).needs_protection(&slice[rel_inner.clone()]) {
        let abs_start = range.start + rel_inner.start;
        let abs_end = range.start + rel_inner.end;
        if let Some(open_brace_idx) = abs_start.checked_sub(1) {
            *fallback_eoi = spans[open_brace_idx].end;
        }
        *range = abs_start..abs_end;
    }
}

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Parse all argument slots declared by `args` in order.
    ///
    /// Enforces each slot's `no_leading_space` flag and handles `\\`'s
    /// environment-dependent optional bracket spacing
    /// (`allow_row_break_opt_space`).
    pub(super) fn parse_argument_slots(
        &mut self,
        args: &'static [ArgSpec],
        context_label: &'static str,
        allow_row_break_opt_space: bool,
    ) -> PResult<Vec<TrackedArgSlot>> {
        if args.is_empty() {
            return Ok(Vec::new());
        }

        self.with_recursion(|p| {
            let mut slots = Vec::with_capacity(args.len());

            for spec in args {
                if !spec.no_leading_space {
                    p.cursor.skip_ws();
                } else if allow_row_break_opt_space
                    && !spec.required
                    && matches!(spec.form, ArgForm::Standard)
                {
                    let before_ws = p.cursor.pos();
                    p.cursor.skip_ws();
                    if p.cursor.peek() != Some(Token::LBracket) {
                        p.cursor.set_pos(before_ws);
                    }
                }

                let mut slot = p.parse_single_argument(spec, context_label)?;
                if let Some(argument) = slot.slot.as_mut() {
                    argument.no_leading_space = spec.no_leading_space;
                }
                slots.push(slot);
            }

            Ok(slots)
        })
    }

    /// Parse one argument slot described by `spec`.
    pub(super) fn parse_single_argument(
        &mut self,
        spec: &'static ArgSpec,
        context_label: &'static str,
    ) -> PResult<TrackedArgSlot> {
        let arg_start_byte = self.cursor.byte_pos();

        match &spec.form {
            ArgForm::Standard => self.parse_standard_argument(spec, context_label, arg_start_byte),
            ArgForm::Star => {
                let present = self.cursor.eat(Token::Star).is_some();
                Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                    ArgumentKind::Star,
                    ArgumentValue::Boolean(present),
                ))))
            }
            ArgForm::Group => self.parse_group_argument(spec, context_label, arg_start_byte),
            ArgForm::Until { close } => {
                self.parse_until_argument(spec, close, context_label, arg_start_byte)
            }
            ArgForm::Delimited { open, close } => {
                self.parse_delimited_argument(spec, open, close, context_label, arg_start_byte)
            }
            ArgForm::Paired { pairs } => {
                self.parse_paired_argument(spec, pairs, context_label, arg_start_byte)
            }
        }
    }

    fn parse_standard_argument(
        &mut self,
        spec: &'static ArgSpec,
        context_label: &'static str,
        arg_start_byte: usize,
    ) -> PResult<TrackedArgSlot> {
        match spec.kind {
            ValueKind::Content { .. } | ValueKind::OperatorName => {
                self.parse_standard_content_argument(spec, context_label, arg_start_byte)
            }
            _ => self.parse_standard_scalar_argument(spec, context_label, arg_start_byte),
        }
    }

    /// Hot-path standard `Content` / `OperatorName` argument parser, split from
    /// scalar kinds to keep recursive call frames compact.
    fn parse_standard_content_argument(
        &mut self,
        spec: &'static ArgSpec,
        context_label: &'static str,
        arg_start_byte: usize,
    ) -> PResult<TrackedArgSlot> {
        let mode = spec
            .kind
            .content_mode()
            .expect("content-like value kind should have a content mode");
        if spec.required {
            if self.cursor.peek() == Some(Token::LBrace) {
                let range =
                    self.with_context(context_label, arg_start_byte, |p| p.scan_braced_arg(true))?;
                let surface = range.full_span();
                let parsed = self.parse_content_subwindow(
                    mode,
                    range.inner,
                    range.open_span.end,
                    surface,
                    context_label,
                )?;
                let (node, span_tree) = parsed.into_span_tree();
                Ok(TrackedArgSlot::with_content(
                    Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        content_value_for_kind(spec.kind, node),
                    )),
                    span_tree,
                ))
            } else {
                // Shorthand unbraced single-item mandatory argument (e.g. `\frac 1 2`).
                // Charge a recursion step here so unbraced nesting (`\frac 1 \frac 1 ...`)
                // shares the same recursion budget as braced nesting (`\frac{1}{...}`).
                let parsed = self.with_recursion(|p| {
                    p.with_context(context_label, arg_start_byte, |p| match mode {
                        ContentMode::Math => p.parse_math_atom_argument(),
                        ContentMode::Text => p.parse_text_argument_item(),
                    })
                })?;
                let (node, span_tree) = parsed.into_span_tree();
                Ok(TrackedArgSlot::with_content(
                    Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        content_value_for_kind(spec.kind, node),
                    )),
                    span_tree,
                ))
            }
        } else if self.cursor.peek() != Some(Token::LBracket) {
            Ok(TrackedArgSlot::untracked(None))
        } else {
            let range = self.with_context(context_label, arg_start_byte, |p| {
                p.scan_bracketed_arg(false)
            })?;
            let surface = range.full_span();
            let mut inner = range.inner;
            let mut fallback_eoi = range.open_span.end;
            strip_content_protection(
                self.cursor.tokens_slice(),
                self.cursor.spans_slice(),
                &mut inner,
                &mut fallback_eoi,
                None,
                Token::RBracket,
            );
            let parsed =
                self.parse_content_subwindow(mode, inner, fallback_eoi, surface, context_label)?;
            let (node, span_tree) = parsed.into_span_tree();
            Ok(TrackedArgSlot::with_content(
                Some(Argument::from_value(
                    ArgumentKind::Optional,
                    content_value_for_kind(spec.kind, node),
                )),
                span_tree,
            ))
        }
    }

    #[inline(never)]
    fn parse_standard_scalar_argument(
        &mut self,
        spec: &'static ArgSpec,
        context_label: &'static str,
        arg_start_byte: usize,
    ) -> PResult<TrackedArgSlot> {
        match spec.kind {
            ValueKind::Content { .. } | ValueKind::OperatorName => {
                unreachable!("content kinds handled by parse_standard_content_argument")
            }
            ValueKind::Delimiter => self.parse_inline_or_bracketed_scalar(
                spec,
                context_label,
                arg_start_byte,
                Delimiter::None,
                "delimiter",
                |p| {
                    scalar::parse_delimiter_cursor(p.ctx, &mut p.cursor).ok_or_else(|| {
                        scalar::ScalarError {
                            message: "invalid delimiter argument",
                            span: p.cursor.peek_span(),
                        }
                    })
                },
                ArgumentValue::Delimiter,
            ),
            ValueKind::Dimension => self.parse_inline_or_bracketed_scalar(
                spec,
                context_label,
                arg_start_byte,
                String::new(),
                "dimension",
                |p| scalar::parse_dimension_cursor(&mut p.cursor),
                ArgumentValue::Dimension,
            ),
            ValueKind::Integer => self.parse_inline_or_bracketed_scalar(
                spec,
                context_label,
                arg_start_byte,
                String::new(),
                "integer",
                |p| scalar::parse_integer_cursor(&mut p.cursor),
                ArgumentValue::Integer,
            ),
            ValueKind::KeyVal => self.parse_braced_or_bracketed_scalar(
                spec,
                context_label,
                arg_start_byte,
                "expected keyval argument",
                |slice| scalar::parse_keyval_slice(slice, spec.nullable),
                ArgumentValue::KeyVal,
            ),
            ValueKind::Column => self.parse_braced_or_bracketed_scalar(
                spec,
                context_label,
                arg_start_byte,
                "expected column argument",
                scalar::parse_column_slice,
                ArgumentValue::Column,
            ),
            ValueKind::CSName => {
                if spec.required {
                    if self.cursor.peek() == Some(Token::LBrace) {
                        let range = self.with_context(context_label, arg_start_byte, |p| {
                            p.scan_delimited_arg(
                                &DelimiterToken::Char('{'),
                                &DelimiterToken::Char('}'),
                            )
                        })?;
                        self.parse_delimited_range_scalar(
                            range,
                            ArgumentKind::Mandatory,
                            context_label,
                            |slice| scalar::parse_cs_name_slice(slice, spec.nullable, false),
                            ArgumentValue::CSName,
                        )
                    } else {
                        let value = self.with_context(context_label, arg_start_byte, |p| {
                            let Some((tok, span)) = p.cursor.bump() else {
                                let span = p.cursor.peek_span();
                                return p.fail_err(
                                    diagnostics::argument_validation(
                                        "missing required CSName argument",
                                        span,
                                    ),
                                    RecoveryHint::Skip,
                                );
                            };
                            scalar::parse_cs_name_slice(std::slice::from_ref(&tok), false, false)
                                .map_err(|msg| {
                                    p.fail(
                                        diagnostics::argument_validation(msg, span),
                                        RecoveryHint::Skip,
                                    )
                                })
                        })?;
                        Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                            ArgumentKind::Mandatory,
                            ArgumentValue::CSName(value),
                        ))))
                    }
                } else if self.cursor.peek() != Some(Token::LBracket) {
                    Ok(TrackedArgSlot::untracked(None))
                } else {
                    let range = self.with_context(context_label, arg_start_byte, |p| {
                        p.scan_bracketed_arg(false)
                    })?;
                    self.parse_delimited_range_scalar(
                        range,
                        ArgumentKind::Optional,
                        context_label,
                        |slice| scalar::parse_cs_name_slice(slice, spec.nullable, true),
                        ArgumentValue::CSName,
                    )
                }
            }
            ValueKind::Star => unreachable!("ValueKind::Star is only valid with ArgForm::Star"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_inline_or_bracketed_scalar<T>(
        &mut self,
        spec: &'static ArgSpec,
        context_label: &'static str,
        arg_start_byte: usize,
        empty_value: T,
        expected_label: &'static str,
        parse_inner: impl Fn(&mut Self) -> Result<T, scalar::ScalarError>,
        wrap: fn(T) -> ArgumentValue,
    ) -> PResult<TrackedArgSlot> {
        if spec.required {
            let value = self.with_context(context_label, arg_start_byte, |p| {
                p.parse_maybe_braced_scalar(spec.nullable, empty_value, parse_inner, expected_label)
            })?;
            Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                ArgumentKind::Mandatory,
                wrap(value),
            ))))
        } else {
            let value = self.try_parse_optional_bracketed_scalar(spec.nullable, empty_value, |p| {
                parse_inner(p).ok()
            });
            Ok(TrackedArgSlot::untracked(value.map(|v| {
                Argument::from_value(ArgumentKind::Optional, wrap(v))
            })))
        }
    }

    fn parse_braced_or_bracketed_scalar<T, E: Into<String>>(
        &mut self,
        spec: &'static ArgSpec,
        context_label: &'static str,
        arg_start_byte: usize,
        missing_required_msg: &'static str,
        parse_slice: impl FnOnce(&[Token<'_>]) -> Result<T, E>,
        wrap: fn(T) -> ArgumentValue,
    ) -> PResult<TrackedArgSlot> {
        let (range, arg_kind) = if spec.required {
            let range = self.with_context(context_label, arg_start_byte, |p| {
                if p.cursor.peek() != Some(Token::LBrace) {
                    let span = p.cursor.peek_span();
                    return p.fail_err(
                        diagnostics::argument_validation(missing_required_msg, span),
                        RecoveryHint::Skip,
                    );
                }
                p.scan_braced_arg(true)
            })?;
            (range, ArgumentKind::Mandatory)
        } else if self.cursor.peek() != Some(Token::LBracket) {
            return Ok(TrackedArgSlot::untracked(None));
        } else {
            let range = self.with_context(context_label, arg_start_byte, |p| {
                p.scan_bracketed_arg(false)
            })?;
            (range, ArgumentKind::Optional)
        };
        self.parse_delimited_range_scalar(range, arg_kind, context_label, parse_slice, wrap)
    }

    fn parse_delimited_range_scalar<T, E: Into<String>>(
        &mut self,
        range: DelimitedRange,
        arg_kind: ArgumentKind,
        context_label: &'static str,
        parse_slice: impl FnOnce(&[Token<'_>]) -> Result<T, E>,
        wrap: fn(T) -> ArgumentValue,
    ) -> PResult<TrackedArgSlot> {
        let surface = range.full_span();
        let value = self.with_context_span(context_label, surface, |p| {
            let slice = &p.cursor.tokens_slice()[range.inner];
            parse_slice(slice).map_err(|msg| {
                p.fail(
                    diagnostics::argument_validation(msg, surface),
                    RecoveryHint::Skip,
                )
            })
        })?;
        Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
            arg_kind,
            wrap(value),
        ))))
    }

    fn parse_group_argument(
        &mut self,
        spec: &'static ArgSpec,
        context_label: &'static str,
        arg_start_byte: usize,
    ) -> PResult<TrackedArgSlot> {
        if self.cursor.peek() != Some(Token::LBrace) {
            if spec.required {
                return self.with_context(context_label, arg_start_byte, |p| {
                    let span = p.cursor.peek_span();
                    p.fail_err(
                        diagnostics::argument_validation(
                            "missing required braced group argument",
                            span,
                        ),
                        RecoveryHint::Skip,
                    )
                });
            }
            return Ok(TrackedArgSlot::untracked(None));
        }

        let range = self.with_context(context_label, arg_start_byte, |p| {
            p.scan_delimited_arg(&DelimiterToken::Char('{'), &DelimiterToken::Char('}'))
        })?;
        let surface = range.full_span();

        if let Some(mode) = spec.kind.content_mode() {
            let parsed = self.parse_content_subwindow(
                mode,
                range.inner,
                range.open_span.end,
                surface,
                context_label,
            )?;
            let (node, span_tree) = parsed.into_span_tree();
            return Ok(TrackedArgSlot::with_content(
                Some(Argument::from_value(
                    ArgumentKind::Group,
                    content_value_for_kind(spec.kind, node),
                )),
                span_tree,
            ));
        }

        let value = self.parse_delimited_scalar_value(
            spec.kind,
            range.inner,
            range.open_span.end,
            spec.nullable,
            surface,
            context_label,
        )?;
        Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
            ArgumentKind::Group,
            value,
        ))))
    }

    fn parse_until_argument(
        &mut self,
        spec: &'static ArgSpec,
        close: &'static DelimiterToken,
        context_label: &'static str,
        arg_start_byte: usize,
    ) -> PResult<TrackedArgSlot> {
        let (mut inner, surface, mut fallback_eoi) =
            self.with_context(context_label, arg_start_byte, |p| p.scan_until_arg(close))?;

        // TeX strips one brace layer when a delimited parameter is a single group.
        let tokens = self.cursor.tokens_slice();
        let slice = &tokens[inner.clone()];
        if slice.first() == Some(&Token::LBrace)
            && slice.last() == Some(&Token::RBrace)
            && let Some(rel) = enclosing_braces(slice)
        {
            let abs_start = inner.start + rel.start;
            let abs_end = inner.start + rel.end;
            if let Some(open_brace_idx) = abs_start.checked_sub(1) {
                fallback_eoi = self.cursor.spans_slice()[open_brace_idx].end;
            }
            inner = abs_start..abs_end;
        }

        let arg_kind = ArgumentKind::Until {
            close: scalar::syntax_delimiter(close),
        };
        if let Some(mode) = spec.kind.content_mode() {
            let parsed =
                self.parse_content_subwindow(mode, inner, fallback_eoi, surface, context_label)?;
            let (node, span_tree) = parsed.into_span_tree();
            return Ok(TrackedArgSlot::with_content(
                Some(Argument::from_value(
                    arg_kind,
                    content_value_for_kind(spec.kind, node),
                )),
                span_tree,
            ));
        }

        let value = self.parse_delimited_scalar_value(
            spec.kind,
            inner,
            fallback_eoi,
            spec.nullable,
            surface,
            context_label,
        )?;
        Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
            arg_kind, value,
        ))))
    }

    fn parse_delimited_argument(
        &mut self,
        spec: &'static ArgSpec,
        open: &'static DelimiterToken,
        close: &'static DelimiterToken,
        context_label: &'static str,
        arg_start_byte: usize,
    ) -> PResult<TrackedArgSlot> {
        let has_open = self
            .cursor
            .peek()
            .is_some_and(|tok| scalar::token_matches_delimiter(tok, open));
        if !has_open {
            if spec.required {
                return self.with_context(context_label, arg_start_byte, |p| {
                    let span = p.cursor.peek_span();
                    p.fail_err(
                        diagnostics::argument_validation(
                            "missing required delimited argument",
                            span,
                        ),
                        RecoveryHint::Skip,
                    )
                });
            }
            return Ok(TrackedArgSlot::untracked(None));
        }

        let range = self.with_context(context_label, arg_start_byte, |p| {
            p.scan_delimited_arg(open, close)
        })?;
        let surface = range.full_span();
        let arg_kind = ArgumentKind::Delimited {
            open: scalar::syntax_delimiter(open),
            close: scalar::syntax_delimiter(close),
        };

        if let Some(mode) = spec.kind.content_mode() {
            let mut inner = range.inner;
            let mut fallback_eoi = range.open_span.end;
            strip_content_protection(
                self.cursor.tokens_slice(),
                self.cursor.spans_slice(),
                &mut inner,
                &mut fallback_eoi,
                Some(scalar::delimiter_token(open)),
                scalar::delimiter_token(close),
            );
            let parsed =
                self.parse_content_subwindow(mode, inner, fallback_eoi, surface, context_label)?;
            let (node, span_tree) = parsed.into_span_tree();
            return Ok(TrackedArgSlot::with_content(
                Some(Argument::from_value(
                    arg_kind,
                    content_value_for_kind(spec.kind, node),
                )),
                span_tree,
            ));
        }

        let value = self.parse_delimited_scalar_value(
            spec.kind,
            range.inner,
            range.open_span.end,
            spec.nullable,
            surface,
            context_label,
        )?;
        Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
            arg_kind, value,
        ))))
    }

    fn parse_paired_argument(
        &mut self,
        spec: &'static ArgSpec,
        pairs: &'static [(DelimiterToken, DelimiterToken)],
        context_label: &'static str,
        arg_start_byte: usize,
    ) -> PResult<TrackedArgSlot> {
        let matched = self.cursor.peek().and_then(|next| {
            pairs
                .iter()
                .find(|(open, _)| scalar::token_matches_delimiter(next, open))
        });

        let Some((open, close)) = matched else {
            if spec.required {
                return self.with_context(context_label, arg_start_byte, |p| {
                    let span = p.cursor.peek_span();
                    p.fail_err(
                        diagnostics::argument_validation("missing required paired argument", span),
                        RecoveryHint::Skip,
                    )
                });
            }
            return Ok(TrackedArgSlot::untracked(None));
        };

        let range = self.with_context(context_label, arg_start_byte, |p| {
            p.scan_delimited_arg(open, close)
        })?;
        let surface = range.full_span();
        let arg_kind = ArgumentKind::Paired {
            open: scalar::syntax_delimiter(open),
            close: scalar::syntax_delimiter(close),
        };

        if let Some(mode) = spec.kind.content_mode() {
            let mut inner = range.inner;
            let mut fallback_eoi = range.open_span.end;
            strip_content_protection(
                self.cursor.tokens_slice(),
                self.cursor.spans_slice(),
                &mut inner,
                &mut fallback_eoi,
                Some(scalar::delimiter_token(open)),
                scalar::delimiter_token(close),
            );
            let parsed =
                self.parse_content_subwindow(mode, inner, fallback_eoi, surface, context_label)?;
            let (node, span_tree) = parsed.into_span_tree();
            return Ok(TrackedArgSlot::with_content(
                Some(Argument::from_value(
                    arg_kind,
                    content_value_for_kind(spec.kind, node),
                )),
                span_tree,
            ));
        }

        let value = self.parse_delimited_scalar_value(
            spec.kind,
            range.inner,
            range.open_span.end,
            spec.nullable,
            surface,
            context_label,
        )?;
        Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
            arg_kind, value,
        ))))
    }

    // ------------------------------------------------------------------------
    // Zero-copy delimiter scanners with CommentTruncatedArgument classification
    // ------------------------------------------------------------------------

    fn scan_braced_arg(&mut self, allow_nested: bool) -> PResult<DelimitedRange> {
        self.cursor.scan_braced(allow_nested).map_err(|err| {
            let (span, msg, check_comment) = match err {
                BalancedScanError::MissingOpen => (self.cursor.peek_span(), "expected '{'", None),
                BalancedScanError::Unclosed { open_span } => (
                    Span::new(
                        open_span.start,
                        self.cursor.prev_end_byte().max(open_span.end),
                    ),
                    "unclosed brace argument",
                    Some(open_span.start),
                ),
                BalancedScanError::NestedBrace { open_span } => (
                    Span::new(open_span.start, self.cursor.prev_end_byte()),
                    "nested braces not allowed",
                    None,
                ),
                BalancedScanError::UnbalancedBrace { open_span } => (
                    Span::new(open_span.start, self.cursor.prev_end_byte()),
                    "unbalanced brace in argument",
                    None,
                ),
            };
            self.emit_unclosed_arg_or_validation(span, msg, check_comment)
        })
    }

    fn scan_bracketed_arg(&mut self, match_brackets: bool) -> PResult<DelimitedRange> {
        self.cursor.scan_bracketed(match_brackets).map_err(|err| {
            let (span, msg, check_comment) = match err {
                BalancedScanError::MissingOpen => (self.cursor.peek_span(), "expected '['", None),
                BalancedScanError::Unclosed { open_span } => (
                    Span::new(
                        open_span.start,
                        self.cursor.prev_end_byte().max(open_span.end),
                    ),
                    "unclosed bracket argument",
                    Some(open_span.start),
                ),
                BalancedScanError::UnbalancedBrace { open_span } => (
                    Span::new(open_span.start, self.cursor.prev_end_byte()),
                    "unbalanced brace in bracket argument",
                    None,
                ),
                BalancedScanError::NestedBrace { open_span } => (
                    Span::new(open_span.start, self.cursor.prev_end_byte()),
                    "nested braces not allowed",
                    None,
                ),
            };
            self.emit_unclosed_arg_or_validation(span, msg, check_comment)
        })
    }

    fn scan_delimited_arg(
        &mut self,
        open: &DelimiterToken,
        close: &DelimiterToken,
    ) -> PResult<DelimitedRange> {
        let open_tok = scalar::delimiter_token(open);
        let close_tok = scalar::delimiter_token(close);

        let Some(open_span) = self.cursor.eat(open_tok) else {
            let span = self.cursor.peek_span();
            let msg = if self.cursor.is_at_end() {
                "expected delimited argument"
            } else {
                "missing opening delimiter"
            };
            return self.fail_err(
                diagnostics::argument_validation(msg, span),
                RecoveryHint::Skip,
            );
        };

        let inner_start = self.cursor.pos();
        let mut boundary = ContentBoundary::new(Some(open_tok), close_tok);

        while let Some((token, close_span)) = self.cursor.bump() {
            if boundary.ends_at(&token) {
                return Ok(DelimitedRange {
                    open_span,
                    inner: inner_start..self.cursor.pos() - 1,
                    close_span,
                });
            }
        }

        let span = Span::new(
            open_span.start,
            self.cursor.prev_end_byte().max(open_span.end),
        );
        Err(self.emit_unclosed_arg_or_validation(
            span,
            "unclosed delimited argument",
            Some(open_span.start),
        ))
    }

    fn scan_until_arg(&mut self, close: &DelimiterToken) -> PResult<(Range<usize>, Span, usize)> {
        let inner_start = self.cursor.pos();
        let start_byte = self.cursor.byte_pos();
        let close_tok = scalar::delimiter_token(close);
        let mut braces = 0usize;

        loop {
            let Some(token) = self.cursor.peek() else {
                let span = Span::new(start_byte, self.cursor.eoi_pos());
                return self.fail_err(
                    diagnostics::argument_validation("missing terminator for argument", span),
                    RecoveryHint::Skip,
                );
            };

            if braces == 0
                && let Some(close_span) = self.cursor.eat(close_tok)
            {
                return Ok((
                    inner_start..self.cursor.pos() - 1,
                    Span::new(start_byte, close_span.end),
                    start_byte,
                ));
            }

            match token {
                Token::LBrace => braces += 1,
                Token::RBrace if braces == 0 => {
                    let span = Span::new(start_byte, self.cursor.byte_pos());
                    return self.fail_err(
                        diagnostics::argument_validation(
                            "missing terminator before closing group",
                            span,
                        ),
                        RecoveryHint::Skip,
                    );
                }
                Token::RBrace => braces -= 1,
                _ => {}
            }
            self.cursor.bump();
        }
    }

    /// Emit `CommentTruncatedArgument` when an unclosed argument scan hit EOF
    /// with an unescaped `%` in the remaining source; otherwise emit
    /// `ArgumentValidation`.
    fn emit_unclosed_arg_or_validation(
        &mut self,
        span: Span,
        message: &'static str,
        comment_check_start: Option<usize>,
    ) -> ParseAbort {
        if let Some(start) = comment_check_start
            && self
                .src
                .get(start..)
                .is_some_and(diagnostics::has_unescaped_percent)
        {
            return self.fail(
                diagnostics::comment_truncated_argument(span),
                RecoveryHint::Skip,
            );
        }
        self.fail(
            diagnostics::argument_validation(message, span),
            RecoveryHint::Skip,
        )
    }

    // ------------------------------------------------------------------------
    // Scalar helpers for Standard & Delimited forms
    // ------------------------------------------------------------------------

    /// Parse an inline or `{...}`-wrapped scalar argument (`Delimiter`,
    /// `Dimension`, `Integer`), optionally accepting an empty `{}` when
    /// `nullable` is `true`.
    fn parse_maybe_braced_scalar<T>(
        &mut self,
        nullable: bool,
        empty_value: T,
        parse_inner: impl Fn(&mut Self) -> Result<T, scalar::ScalarError>,
        expected_label: &'static str,
    ) -> PResult<T> {
        if let Some(open_span) = self.cursor.eat(Token::LBrace) {
            self.cursor.skip_ws();
            if nullable && self.cursor.eat(Token::RBrace).is_some() {
                return Ok(empty_value);
            }
            let value = parse_inner(self).map_err(|err| {
                self.fail(
                    diagnostics::argument_validation(err.message, err.span),
                    RecoveryHint::Skip,
                )
            })?;
            self.cursor.skip_ws();
            if self.cursor.eat(Token::RBrace).is_some() {
                Ok(value)
            } else {
                let err_span = self.cursor.peek_span();
                let found = self.cursor.peek();
                if found.is_none() && self.try_emit_eof_upgrade(err_span, Some(open_span.start)) {
                    return Err(ParseAbort {
                        hint: RecoveryHint::Skip,
                    });
                }
                self.fail_err(
                    diagnostics::argument_validation_expected(
                        format!("expected '}}' closing {expected_label} argument"),
                        err_span,
                        &["'}'"],
                        found,
                    ),
                    RecoveryHint::Skip,
                )
            }
        } else {
            parse_inner(self).map_err(|err| {
                self.fail(
                    diagnostics::argument_validation(err.message, err.span),
                    RecoveryHint::Skip,
                )
            })
        }
    }

    /// Parse an optional `[...]` scalar argument with backtracking: if `[` is
    /// followed by content that does not parse as a valid scalar (e.g. `\\[a,b]`
    /// in a matrix), rewind cleanly and treat the optional slot as absent
    /// (`None`).
    fn try_parse_optional_bracketed_scalar<T>(
        &mut self,
        nullable: bool,
        empty_value: T,
        parse_inner: impl Fn(&mut Self) -> Option<T>,
    ) -> Option<T> {
        let cp = self.checkpoint();
        self.cursor.eat(Token::LBracket)?;
        self.cursor.skip_ws();

        let value = if let Some(val) = parse_inner(self) {
            val
        } else if nullable {
            empty_value
        } else {
            self.rewind(cp);
            return None;
        };

        self.cursor.skip_ws();
        if self.cursor.eat(Token::RBracket).is_some() {
            Some(value)
        } else {
            self.rewind(cp);
            None
        }
    }

    /// Parse a scalar [`ArgumentValue`] from an already-collected token range
    /// (used by `Group`, `Until`, `Delimited`, and `Paired`).
    fn parse_delimited_scalar_value(
        &mut self,
        kind: ValueKind,
        inner: Range<usize>,
        fallback_eoi: usize,
        nullable: bool,
        surface_span: Span,
        context_label: &'static str,
    ) -> PResult<ArgumentValue> {
        self.with_context_span(context_label, surface_span, |p| {
            let tokens = &p.cursor.tokens_slice()[inner.clone()];
            let spans = &p.cursor.spans_slice()[inner];
            let parsed: Result<ArgumentValue, Cow<'static, str>> = match kind {
                ValueKind::Content { .. } | ValueKind::OperatorName => {
                    unreachable!("content arguments are parsed by parse_content_subwindow")
                }
                ValueKind::CSName => scalar::parse_cs_name_slice(tokens, nullable, true)
                    .map(ArgumentValue::CSName)
                    .map_err(Cow::Borrowed),
                ValueKind::Dimension => {
                    scalar::parse_dimension_slice(tokens, spans, fallback_eoi, nullable)
                        .map(ArgumentValue::Dimension)
                        .map_err(Cow::Borrowed)
                }
                ValueKind::Integer => {
                    scalar::parse_integer_slice(tokens, spans, fallback_eoi, nullable)
                        .map(ArgumentValue::Integer)
                        .map_err(Cow::Borrowed)
                }
                ValueKind::KeyVal => scalar::parse_keyval_slice(tokens, nullable)
                    .map(ArgumentValue::KeyVal)
                    .map_err(Cow::Borrowed),
                ValueKind::Column => scalar::parse_column_slice(tokens)
                    .map(ArgumentValue::Column)
                    .map_err(Cow::Owned),
                ValueKind::Delimiter => {
                    scalar::parse_delimiter_slice(p.ctx, tokens, spans, fallback_eoi, nullable)
                        .map(ArgumentValue::Delimiter)
                        .map_err(Cow::Borrowed)
                }
                ValueKind::Star => {
                    unreachable!("ValueKind::Star is only valid with ArgForm::Star")
                }
            };
            parsed.map_err(|msg| {
                p.fail(
                    diagnostics::argument_validation(msg, surface_span),
                    RecoveryHint::Skip,
                )
            })
        })
    }

    // ------------------------------------------------------------------------
    // Zero-copy content argument sub-window parsing
    // ------------------------------------------------------------------------

    fn parse_content_subwindow(
        &mut self,
        mode: ContentMode,
        inner: Range<usize>,
        fallback_eoi: usize,
        surface_span: Span,
        context_label: &'static str,
    ) -> PResult<ParsedNode> {
        let sub_tokens = &self.cursor.tokens_slice()[inner.clone()];
        let full_inner_span = self.cursor.span_of_range(inner.clone());

        // Whitespace-only text content (e.g. `\text{ }`) collapses to a single
        // space node spanning the inner whitespace tokens.
        if mode == ContentMode::Text
            && !sub_tokens.is_empty()
            && sub_tokens.iter().all(|tok| *tok == Token::Whitespaces)
        {
            return Ok(ParsedNode::leaf(
                SyntaxNode::Text(" ".to_string()),
                full_inner_span,
            ));
        }

        let sub = self.cursor.sub_window(inner, fallback_eoi);

        self.with_context_span(context_label, surface_span, |p| {
            p.with_sub_cursor(sub, |p| {
                let items = match mode {
                    ContentMode::Math => p.parse_math_content()?,
                    ContentMode::Text => p.parse_text_content()?,
                };

                if !p.cursor.is_at_end() {
                    let found = p.cursor.peek();
                    let err_span = p.cursor.peek_span();
                    let diag = if mode == ContentMode::Math && found == Some(Token::MathShift) {
                        diagnostics::unexpected_math_shift(p.src, err_span)
                    } else {
                        diagnostics::raw_expected_found(err_span, &["end of input"], found)
                    };
                    let err_message = diag.message.clone();
                    p.emit_diag(diag);

                    if p.config.abort_on_error {
                        return Err(ParseAbort {
                            hint: RecoveryHint::Skip,
                        });
                    }

                    // Lenient recovery inside a content argument: preserve the
                    // enclosing command shell by folding the sub-window into an
                    // Error node when no inner items succeeded, or attaching a
                    // trailing Error node.
                    if items.is_empty() {
                        let span = if full_inner_span.start < full_inner_span.end {
                            full_inner_span
                        } else {
                            err_span
                        };
                        let snippet = p.slice_snippet(span);
                        return Ok(ParsedNode::leaf(
                            SyntaxNode::Error {
                                message: err_message,
                                snippet,
                            },
                            span,
                        ));
                    }
                    let tail_span = p.cursor.span_of_range(p.cursor.pos()..p.cursor.len());
                    let tail_snippet = p.slice_snippet(tail_span);
                    let mut all_items = items;
                    all_items.push(ParsedNode::leaf(
                        SyntaxNode::Error {
                            message: err_message,
                            snippet: tail_snippet,
                        },
                        tail_span,
                    ));
                    return Ok(tree::fold_argument(mode, all_items, full_inner_span));
                }

                // Cover all tokens consumed by the sub-window (including math
                // padding whitespace) or a point span at `fallback_eoi` when
                // empty.
                let content_span = p.cursor.span_of_range(0..p.cursor.pos());
                Ok(tree::fold_argument(mode, items, content_span))
            })
        })
    }
}
