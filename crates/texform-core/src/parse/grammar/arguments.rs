//! Argument-slot parsing for commands and environments.
//!
//! Dispatches directly on `(ArgForm, ValueKind)`, collects balanced token
//! ranges without copying `Vec<Token>`, strips protective braces in place, and
//! parses content arguments via zero-copy sub-windows on the same
//! [`TokenParser`].

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
use crate::parse::result::{ParseDiagnosticKind, Span};
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
        _parent_mode: ContentMode,
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
                let present = if self.cursor.peek() == Some(Token::Star) {
                    self.cursor.bump();
                    true
                } else {
                    false
                };
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
                    p.with_context(context_label, arg_start_byte, |p| {
                        let before_pos = p.cursor.pos();
                        let diag_start = p.diagnostics().len();
                        let res = match mode {
                            ContentMode::Math => p.parse_math_atom_argument(),
                            ContentMode::Text => p.parse_text_argument_item(),
                        };
                        match res {
                            Ok(node) => Ok(node),
                            Err(abort) => {
                                // When failure occurs at the argument entry
                                // before consuming any token, report
                                // `ArgumentValidation` at that span.
                                if p.cursor.pos() == before_pos
                                    && p.diagnostics().len() > diag_start
                                {
                                    p.promote_entry_arg_validation(diag_start);
                                }
                                Err(abort)
                            }
                        }
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
            ValueKind::Delimiter => {
                if spec.required {
                    let value = self.with_context(context_label, arg_start_byte, |p| {
                        p.parse_maybe_braced_scalar(
                            spec.nullable,
                            Delimiter::None,
                            |p| {
                                scalar::parse_delimiter_cursor(p.ctx, &mut p.cursor).ok_or_else(
                                    || scalar::ScalarError {
                                        message: "invalid delimiter argument",
                                        span: p.cursor.peek_span(),
                                    },
                                )
                            },
                            "delimiter",
                        )
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        ArgumentValue::Delimiter(value),
                    ))))
                } else {
                    let value = self.try_parse_optional_bracketed_scalar(
                        spec.nullable,
                        Delimiter::None,
                        |p| scalar::parse_delimiter_cursor(p.ctx, &mut p.cursor),
                    );
                    Ok(TrackedArgSlot::untracked(value.map(|v| {
                        Argument::from_value(ArgumentKind::Optional, ArgumentValue::Delimiter(v))
                    })))
                }
            }
            ValueKind::Dimension => {
                if spec.required {
                    let value = self.with_context(context_label, arg_start_byte, |p| {
                        p.parse_maybe_braced_scalar(
                            spec.nullable,
                            String::new(),
                            |p| scalar::parse_dimension_cursor(&mut p.cursor),
                            "dimension",
                        )
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        ArgumentValue::Dimension(value),
                    ))))
                } else {
                    let value = self.try_parse_optional_bracketed_scalar(
                        spec.nullable,
                        String::new(),
                        |p| scalar::parse_dimension_cursor(&mut p.cursor).ok(),
                    );
                    Ok(TrackedArgSlot::untracked(value.map(|v| {
                        Argument::from_value(ArgumentKind::Optional, ArgumentValue::Dimension(v))
                    })))
                }
            }
            ValueKind::Integer => {
                if spec.required {
                    let value = self.with_context(context_label, arg_start_byte, |p| {
                        p.parse_maybe_braced_scalar(
                            spec.nullable,
                            String::new(),
                            |p| scalar::parse_integer_cursor(&mut p.cursor),
                            "integer",
                        )
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        ArgumentValue::Integer(value),
                    ))))
                } else {
                    let value = self.try_parse_optional_bracketed_scalar(
                        spec.nullable,
                        String::new(),
                        |p| scalar::parse_integer_cursor(&mut p.cursor).ok(),
                    );
                    Ok(TrackedArgSlot::untracked(value.map(|v| {
                        Argument::from_value(ArgumentKind::Optional, ArgumentValue::Integer(v))
                    })))
                }
            }
            ValueKind::KeyVal => {
                if spec.required {
                    let range = self.with_context(context_label, arg_start_byte, |p| {
                        if p.cursor.peek() != Some(Token::LBrace) {
                            let span = p.cursor.peek_span();
                            let contexts = p.snapshot_contexts(span.end);
                            return Err(p.emit_and_abort(
                                diagnostics::argument_validation(
                                    "expected keyval argument",
                                    span,
                                    contexts,
                                ),
                                RecoveryHint::Skip,
                            ));
                        }
                        p.scan_braced_arg(true)
                    })?;
                    let surface = range.full_span();
                    let value = self.with_context_span(context_label, surface, |p| {
                        let slice = &p.cursor.tokens_slice()[range.inner];
                        scalar::parse_keyval_slice(slice, spec.nullable).map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        ArgumentValue::KeyVal(value),
                    ))))
                } else if self.cursor.peek() != Some(Token::LBracket) {
                    Ok(TrackedArgSlot::untracked(None))
                } else {
                    let range = self.with_context(context_label, arg_start_byte, |p| {
                        p.scan_bracketed_arg(false)
                    })?;
                    let surface = range.full_span();
                    let value = self.with_context_span(context_label, surface, |p| {
                        let slice = &p.cursor.tokens_slice()[range.inner];
                        scalar::parse_keyval_slice(slice, spec.nullable).map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Optional,
                        ArgumentValue::KeyVal(value),
                    ))))
                }
            }
            ValueKind::Column => {
                if spec.required {
                    let range = self.with_context(context_label, arg_start_byte, |p| {
                        if p.cursor.peek() != Some(Token::LBrace) {
                            let span = p.cursor.peek_span();
                            let contexts = p.snapshot_contexts(span.end);
                            return Err(p.emit_and_abort(
                                diagnostics::argument_validation(
                                    "expected column argument",
                                    span,
                                    contexts,
                                ),
                                RecoveryHint::Skip,
                            ));
                        }
                        p.scan_braced_arg(true)
                    })?;
                    let surface = range.full_span();
                    let value = self.with_context_span(context_label, surface, |p| {
                        let slice = &p.cursor.tokens_slice()[range.inner];
                        scalar::parse_column_slice(slice).map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        ArgumentValue::Column(value),
                    ))))
                } else if self.cursor.peek() != Some(Token::LBracket) {
                    Ok(TrackedArgSlot::untracked(None))
                } else {
                    let range = self.with_context(context_label, arg_start_byte, |p| {
                        p.scan_bracketed_arg(false)
                    })?;
                    let surface = range.full_span();
                    let value = self.with_context_span(context_label, surface, |p| {
                        let slice = &p.cursor.tokens_slice()[range.inner];
                        scalar::parse_column_slice(slice).map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Optional,
                        ArgumentValue::Column(value),
                    ))))
                }
            }
            ValueKind::CSName => {
                if spec.required {
                    if self.cursor.peek() == Some(Token::LBrace) {
                        let range = self.with_context(context_label, arg_start_byte, |p| {
                            p.scan_delimited_arg(
                                &DelimiterToken::Char('{'),
                                &DelimiterToken::Char('}'),
                            )
                        })?;
                        let surface = range.full_span();
                        let value = self.with_context_span(context_label, surface, |p| {
                            let slice = &p.cursor.tokens_slice()[range.inner];
                            scalar::parse_cs_name_slice(slice, spec.nullable, false).map_err(
                                |msg| {
                                    let contexts = p.snapshot_contexts(surface.end);
                                    p.emit_and_abort(
                                        diagnostics::argument_validation(msg, surface, contexts),
                                        RecoveryHint::Skip,
                                    )
                                },
                            )
                        })?;
                        Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                            ArgumentKind::Mandatory,
                            ArgumentValue::CSName(value),
                        ))))
                    } else {
                        let value = self.with_context(context_label, arg_start_byte, |p| {
                            let Some((tok, span)) = p.cursor.bump() else {
                                let span = p.cursor.peek_span();
                                let contexts = p.snapshot_contexts(span.end);
                                return Err(p.emit_and_abort(
                                    diagnostics::argument_validation(
                                        "missing required CSName argument",
                                        span,
                                        contexts,
                                    ),
                                    RecoveryHint::Skip,
                                ));
                            };
                            scalar::parse_cs_name_slice(std::slice::from_ref(&tok), false, false)
                                .map_err(|msg| {
                                    let contexts = p.snapshot_contexts(span.end);
                                    p.emit_and_abort(
                                        diagnostics::argument_validation(msg, span, contexts),
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
                    let surface = range.full_span();
                    let value = self.with_context_span(context_label, surface, |p| {
                        let slice = &p.cursor.tokens_slice()[range.inner];
                        scalar::parse_cs_name_slice(slice, spec.nullable, true).map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                    })?;
                    Ok(TrackedArgSlot::untracked(Some(Argument::from_value(
                        ArgumentKind::Optional,
                        ArgumentValue::CSName(value),
                    ))))
                }
            }
            ValueKind::Star => self.with_context(context_label, arg_start_byte, |p| {
                let span = p.cursor.peek_span();
                let contexts = p.snapshot_contexts(span.end);
                Err(p.emit_and_abort(
                    diagnostics::argument_validation(
                        "invalid spec: star kind requires star form",
                        span,
                        contexts,
                    ),
                    RecoveryHint::Skip,
                ))
            }),
        }
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
                    let contexts = p.snapshot_contexts(span.end);
                    Err(p.emit_and_abort(
                        diagnostics::argument_validation(
                            "missing required braced group argument",
                            span,
                            contexts,
                        ),
                        RecoveryHint::Skip,
                    ))
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
                    let contexts = p.snapshot_contexts(span.end);
                    Err(p.emit_and_abort(
                        diagnostics::argument_validation(
                            "missing required delimited argument",
                            span,
                            contexts,
                        ),
                        RecoveryHint::Skip,
                    ))
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
                    let contexts = p.snapshot_contexts(span.end);
                    Err(p.emit_and_abort(
                        diagnostics::argument_validation(
                            "missing required paired argument",
                            span,
                            contexts,
                        ),
                        RecoveryHint::Skip,
                    ))
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

        match self.cursor.peek() {
            None => {
                let span = self.cursor.peek_span();
                let contexts = self.snapshot_contexts(span.end);
                return Err(self.emit_and_abort(
                    diagnostics::argument_validation("expected delimited argument", span, contexts),
                    RecoveryHint::Skip,
                ));
            }
            Some(tok) if tok != open_tok => {
                let span = self.cursor.peek_span();
                let contexts = self.snapshot_contexts(span.end);
                return Err(self.emit_and_abort(
                    diagnostics::argument_validation("missing opening delimiter", span, contexts),
                    RecoveryHint::Skip,
                ));
            }
            Some(_) => {}
        }

        let (_, open_span) = self.cursor.bump().expect("peek ensured open delimiter");
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
                let contexts = self.snapshot_contexts(span.end);
                return Err(self.emit_and_abort(
                    diagnostics::argument_validation(
                        "missing terminator for argument",
                        span,
                        contexts,
                    ),
                    RecoveryHint::Skip,
                ));
            };

            if braces == 0 && token == close_tok {
                let inner_end = self.cursor.pos();
                let (_, close_span) = self.cursor.bump().expect("peek ensured close delimiter");
                return Ok((
                    inner_start..inner_end,
                    Span::new(start_byte, close_span.end),
                    start_byte,
                ));
            }

            match token {
                Token::LBrace => braces += 1,
                Token::RBrace if braces == 0 => {
                    let span = Span::new(start_byte, self.cursor.byte_pos());
                    let contexts = self.snapshot_contexts(span.end);
                    return Err(self.emit_and_abort(
                        diagnostics::argument_validation(
                            "missing terminator before closing group",
                            span,
                            contexts,
                        ),
                        RecoveryHint::Skip,
                    ));
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
            let contexts = self.snapshot_contexts(span.end);
            return self.emit_and_abort(
                diagnostics::comment_truncated_argument(span, contexts),
                RecoveryHint::Skip,
            );
        }
        let contexts = self.snapshot_contexts(span.end);
        self.emit_and_abort(
            diagnostics::argument_validation(message, span, contexts),
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
        if self.cursor.peek() == Some(Token::LBrace) {
            let (_, open_span) = self.cursor.bump().expect("peek ensured LBrace");
            self.cursor.skip_ws();
            if nullable && self.cursor.peek() == Some(Token::RBrace) {
                self.cursor.bump();
                return Ok(empty_value);
            }
            let value = match parse_inner(self) {
                Ok(v) => v,
                Err(err) => {
                    let contexts = self.snapshot_contexts(err.span.end);
                    return Err(self.emit_and_abort(
                        diagnostics::argument_validation(err.message, err.span, contexts),
                        RecoveryHint::Skip,
                    ));
                }
            };
            self.cursor.skip_ws();
            if self.cursor.peek() == Some(Token::RBrace) {
                self.cursor.bump();
                Ok(value)
            } else {
                let err_span = self.cursor.peek_span();
                let found = self.cursor.peek();
                if found.is_none() && self.try_emit_eof_upgrade(err_span, Some(open_span.start)) {
                    return Err(ParseAbort {
                        hint: RecoveryHint::Skip,
                    });
                }
                let contexts = self.snapshot_contexts(err_span.end);
                Err(self.emit_and_abort(
                    diagnostics::argument_validation_expected(
                        format!("expected '}}' closing {expected_label} argument"),
                        err_span,
                        &["'}'"],
                        found,
                        contexts,
                    ),
                    RecoveryHint::Skip,
                ))
            }
        } else {
            parse_inner(self).map_err(|err| {
                let contexts = self.snapshot_contexts(err.span.end);
                self.emit_and_abort(
                    diagnostics::argument_validation(err.message, err.span, contexts),
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
        if self.cursor.peek() != Some(Token::LBracket) {
            return None;
        }
        let cp = self.checkpoint();
        self.cursor.bump(); // consume '['
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
        if self.cursor.peek() == Some(Token::RBracket) {
            self.cursor.bump();
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
            match kind {
                ValueKind::Content { .. } | ValueKind::OperatorName => {
                    unreachable!("content arguments are parsed by parse_content_subwindow")
                }
                ValueKind::CSName => scalar::parse_cs_name_slice(tokens, nullable, true)
                    .map(ArgumentValue::CSName)
                    .map_err(|msg| {
                        let contexts = p.snapshot_contexts(surface_span.end);
                        p.emit_and_abort(
                            diagnostics::argument_validation(msg, surface_span, contexts),
                            RecoveryHint::Skip,
                        )
                    }),
                ValueKind::Dimension => {
                    scalar::parse_dimension_slice(tokens, spans, fallback_eoi, nullable)
                        .map(ArgumentValue::Dimension)
                        .map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface_span.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface_span, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                }
                ValueKind::Integer => {
                    scalar::parse_integer_slice(tokens, spans, fallback_eoi, nullable)
                        .map(ArgumentValue::Integer)
                        .map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface_span.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface_span, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                }
                ValueKind::KeyVal => scalar::parse_keyval_slice(tokens, nullable)
                    .map(ArgumentValue::KeyVal)
                    .map_err(|msg| {
                        let contexts = p.snapshot_contexts(surface_span.end);
                        p.emit_and_abort(
                            diagnostics::argument_validation(msg, surface_span, contexts),
                            RecoveryHint::Skip,
                        )
                    }),
                ValueKind::Column => scalar::parse_column_slice(tokens)
                    .map(ArgumentValue::Column)
                    .map_err(|msg| {
                        let contexts = p.snapshot_contexts(surface_span.end);
                        p.emit_and_abort(
                            diagnostics::argument_validation(msg, surface_span, contexts),
                            RecoveryHint::Skip,
                        )
                    }),
                ValueKind::Delimiter => {
                    scalar::parse_delimiter_slice(p.ctx, tokens, spans, fallback_eoi, nullable)
                        .map(ArgumentValue::Delimiter)
                        .map_err(|msg| {
                            let contexts = p.snapshot_contexts(surface_span.end);
                            p.emit_and_abort(
                                diagnostics::argument_validation(msg, surface_span, contexts),
                                RecoveryHint::Skip,
                            )
                        })
                }
                ValueKind::Star => {
                    let contexts = p.snapshot_contexts(surface_span.end);
                    Err(p.emit_and_abort(
                        diagnostics::argument_validation(
                            "invalid spec: star kind is not supported by delimited/paired forms",
                            surface_span,
                            contexts,
                        ),
                        RecoveryHint::Skip,
                    ))
                }
            }
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
        let sub_eoi = sub.eoi_pos();

        self.with_context_span(context_label, surface_span, |p| {
            let diag_start = p.diagnostics().len();

            p.with_sub_cursor(sub, |p| {
                let items = match mode {
                    ContentMode::Math => p.parse_math_content()?,
                    ContentMode::Text => p.parse_text_content()?,
                };

                if !p.cursor.is_at_end() {
                    let found = p.cursor.peek();
                    let err_span = p.cursor.peek_span();
                    let diag = if mode == ContentMode::Math && found == Some(Token::MathShift) {
                        let contexts = p.snapshot_contexts(err_span.end);
                        diagnostics::unexpected_math_shift(p.src, err_span, contexts)
                    } else {
                        let contexts = p.snapshot_contexts(err_span.end);
                        diagnostics::raw_expected_found(
                            err_span,
                            &["something else", "end of input"],
                            found,
                            contexts,
                        )
                    };
                    let err_message = diag.message.clone();
                    p.emit(diag);

                    if p.config.abort_on_error || p.config.reject_unknown {
                        return Err(ParseAbort {
                            hint: RecoveryHint::Skip,
                        });
                    }

                    // Lenient recovery inside a content argument: preserve the
                    // enclosing command shell by folding the sub-window into an
                    // Error node when no inner items succeeded, or attaching a
                    // trailing Error node.
                    p.suppress_trailing_subwindow_noise(diag_start, sub_eoi);
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

                // If lenient text recovery saw an unclosed trailing `$` right at
                // the sub-window tail *after* an earlier semantic error (such as
                // `\text{\frac{a}{b}$}` or `\text{\underline{a^2}$}`), drop that
                // trailing noise diagnostic.
                p.suppress_trailing_subwindow_noise(diag_start, sub_eoi);

                // Cover all tokens consumed by the sub-window (including math
                // padding whitespace) or a point span at `fallback_eoi` when
                // empty.
                let content_span = p.cursor.span_of_range(0..p.cursor.pos());
                Ok(tree::fold_argument(mode, items, content_span))
            })
        })
    }

    /// Upgrade an entry-position failure inside an unbraced mandatory argument
    /// to [`ParseDiagnosticKind::ArgumentValidation`] for cases such as
    /// `a+\frac{1}`, `a+\frac 1 }`, and `a+\frac 1 \bad`.
    fn promote_entry_arg_validation(&mut self, diag_start: usize) {
        if let Some(diag) = self.diagnostics_mut().get_mut(diag_start)
            && matches!(
                diag.kind,
                Some(ParseDiagnosticKind::RawExpectedFound | ParseDiagnosticKind::UnknownCommand)
            )
        {
            diag.kind = Some(ParseDiagnosticKind::ArgumentValidation);
        }
    }

    /// Drop trailing sub-window tail noise (such as a lone `$` right at the end
    /// of `\text{\frac{a}{b}$}` or `\text{\underline{a^2}$}`) when an earlier
    /// semantic diagnostic already explained the failure inside the same
    /// sub-window.
    fn suppress_trailing_subwindow_noise(&mut self, diag_start: usize, sub_eoi: usize) {
        let diags = self.diagnostics_mut();
        if diags.len() <= diag_start + 1 {
            return;
        }
        let first_direct_start = diags[diag_start..]
            .iter()
            .filter(|d| {
                !matches!(
                    d.kind,
                    Some(
                        ParseDiagnosticKind::RawExpectedFound
                            | ParseDiagnosticKind::UnclosedInlineMath
                    ) | None
                )
            })
            .map(|d| d.span.start)
            .min();
        let Some(first_direct_start) = first_direct_start else {
            return;
        };
        let mut idx = diag_start;
        while idx < diags.len() {
            let is_trailing_noise = matches!(
                diags[idx].kind,
                Some(
                    ParseDiagnosticKind::RawExpectedFound | ParseDiagnosticKind::UnclosedInlineMath
                )
            ) && diags[idx].span.end == sub_eoi
                && diags[idx].span.start > first_direct_start;
            if is_trailing_noise {
                diags.remove(idx);
            } else {
                idx += 1;
            }
        }
    }
}
