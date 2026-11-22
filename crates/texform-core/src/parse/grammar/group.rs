//! Paired delimiter structures: explicit `{...}` groups, `\left...\right`
//! delimited groups, and `$...$` / `\(...\)` inline math groups.

use texform_interface::syntax_node::{ContentMode, Delimiter, GroupKind, SyntaxNode};

use crate::lexer::Token;
use crate::parse::diagnostics;
use crate::parse::parser::{MathStop, PResult, ParseAbort, RecoveryHint, TokenParser};
use crate::parse::tree::{self, ParsedNode};

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Parse an explicit `{...}` brace group in `mode`.
    ///
    /// Enforces `config.max_group_depth` (and delegates global recursion-depth
    /// counting to its inner `parse_math_content` / `parse_text_content` call
    /// so each brace level consumes one recursion step). When `max_group_depth`
    /// is exceeded, skips to the matching `}`, emits `MaxGroupDepthExceeded`,
    /// and returns an in-place `SyntaxNode::Error` even when `abort_on_error`
    /// is `true`.
    pub(super) fn parse_braced_group(&mut self, mode: ContentMode) -> PResult<ParsedNode> {
        let group_start_pos = self.cursor.pos();
        self.expect(Token::LBrace, "'{'")?;

        let Some(result) = self.with_group(|p| {
            let children = match mode {
                ContentMode::Math => p.parse_math_content()?,
                ContentMode::Text => p.parse_text_content()?,
            };

            if p.cursor.eat(Token::RBrace).is_none() {
                let err_span = p.cursor.peek_span();
                let found = p.cursor.peek();
                if found.is_none() {
                    if p.try_emit_eof_upgrade(err_span, None) {
                        return Err(ParseAbort {
                            hint: RecoveryHint::Skip,
                        });
                    }
                    return p.fail_err(diagnostics::unclosed_group(err_span), RecoveryHint::Skip);
                }
                if mode == ContentMode::Math && found == Some(Token::MathShift) {
                    return p.fail_err(
                        diagnostics::unexpected_math_shift(p.src, err_span),
                        RecoveryHint::Skip,
                    );
                }
                return Err(p.emit_expected(err_span, &["'}'"], found));
            }

            let span = p.cursor.span_of_range(group_start_pos..p.cursor.pos());
            let (nodes, span_kids) = tree::decompose_children(children);
            Ok(ParsedNode::with_kids(
                SyntaxNode::Group {
                    mode,
                    kind: GroupKind::Explicit,
                    children: nodes,
                },
                span,
                span_kids,
            ))
        }) else {
            self.cursor.skip_matching_rbrace();
            let span = self
                .cursor
                .span_of_range(group_start_pos..self.cursor.pos());
            let diag = diagnostics::max_group_depth_exceeded(self.config.max_group_depth, span);
            let message = diag.message.clone();
            self.emit_diag(diag);
            let snippet = self.slice_snippet(span);
            return Ok(ParsedNode::leaf(
                SyntaxNode::Error { message, snippet },
                span,
            ));
        };

        result
    }

    /// Try to consume a registered math delimiter (or `.` for `Delimiter::None`).
    pub(super) fn try_parse_delimiter(&mut self) -> Option<Delimiter> {
        let delim = match self.cursor.peek()? {
            Token::Char(c) => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                let record = self.ctx.lookup_delimiter(s, false, ContentMode::Math)?;
                if record.name == "." && record.unicode_value.is_empty() {
                    Delimiter::None
                } else {
                    Delimiter::Char(c)
                }
            }
            // Raw square brackets are tokenized separately for optional
            // arguments, but still act as plain delimiters after \left / \right.
            Token::LBracket => {
                self.ctx.lookup_delimiter("[", false, ContentMode::Math)?;
                Delimiter::Char('[')
            }
            Token::RBracket => {
                self.ctx.lookup_delimiter("]", false, ContentMode::Math)?;
                Delimiter::Char(']')
            }
            Token::ControlSeq(name) => {
                self.ctx.lookup_delimiter(name, true, ContentMode::Math)?;
                let canonical = self.ctx.lookup_delimiter_control(name)?;
                Delimiter::Control(canonical)
            }
            _ => return None,
        };
        self.cursor.bump();
        Some(delim)
    }

    /// Parse a `\left <delim> ... \right <delim>` math group.
    ///
    /// Whitespace between `\left`/`\right` and its delimiter is allowed. All
    /// delimiter and missing-`\right` errors propagate via
    /// [`RecoveryHint::Propagate`] so enclosing constructs handle recovery.
    pub(super) fn parse_delimited_group(&mut self) -> PResult<ParsedNode> {
        self.with_recursion(|p| {
            let group_start_pos = p.cursor.pos();
            let group_start_byte = p.cursor.byte_pos();
            p.cursor.bump(); // consume `\left`

            p.with_context("left-delimited group", group_start_byte, |p| {
                p.cursor.skip_ws();
                let left = match p.try_parse_delimiter() {
                    Some(delim) => delim,
                    None => {
                        let err_span = p.cursor.peek_span();
                        return p.fail_err(
                            diagnostics::left_right_delimiter("invalid \\left delimiter", err_span),
                            RecoveryHint::Propagate,
                        );
                    }
                };

                let children = p.parse_math_content()?;

                if p.cursor.eat(Token::ControlSeq("right")).is_none() {
                    let span = p.cursor.span_of_range(group_start_pos..p.cursor.pos());
                    return p.fail_err(
                        diagnostics::left_right_delimiter(
                            "missing \\right for \\left-delimited group",
                            span,
                        ),
                        RecoveryHint::Propagate,
                    );
                }

                p.cursor.skip_ws();
                let right = match p.try_parse_delimiter() {
                    Some(delim) => delim,
                    None => {
                        let err_span = p.cursor.peek_span();
                        return p.fail_err(
                            diagnostics::left_right_delimiter(
                                "invalid \\right delimiter",
                                err_span,
                            ),
                            RecoveryHint::Propagate,
                        );
                    }
                };

                let span = p.cursor.span_of_range(group_start_pos..p.cursor.pos());
                let (nodes, span_kids) = tree::decompose_children(children);
                Ok(ParsedNode::with_kids(
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Delimited { left, right },
                        children: nodes,
                    },
                    span,
                    span_kids,
                ))
            })
        })
    }

    /// Parse `$ ... $` inline math inside text mode.
    pub(super) fn parse_inline_math_dollar(&mut self) -> PResult<ParsedNode> {
        self.with_recursion(|p| {
            let start_pos = p.cursor.pos();
            let opener_span = p.expect(Token::MathShift, "'$'")?;

            let children = p.parse_math_content()?;

            if p.cursor.eat(Token::MathShift).is_none() {
                if p.cursor.is_at_end() {
                    return p.fail_err(
                        diagnostics::unclosed_inline_math(opener_span),
                        RecoveryHint::Skip,
                    );
                }
                let err_span = p.cursor.peek_span();
                let found = p.cursor.peek();
                return Err(p.emit_expected(err_span, &["'$'"], found));
            }

            let span = p.cursor.span_of_range(start_pos..p.cursor.pos());
            let (nodes, span_kids) = tree::decompose_children(children);
            Ok(ParsedNode::with_kids(
                SyntaxNode::Group {
                    mode: ContentMode::Math,
                    kind: GroupKind::InlineMath,
                    children: nodes,
                },
                span,
                span_kids,
            ))
        })
    }

    /// Parse `\( ... \)` inline math inside text mode.
    pub(super) fn parse_inline_math_paren(&mut self) -> PResult<ParsedNode> {
        self.with_recursion(|p| {
            let start_pos = p.cursor.pos();
            p.cursor.bump(); // consume `\(`

            let children = p.with_math_stop(MathStop::ControlParen, |p| p.parse_math_content())?;

            if p.cursor.eat(Token::ControlSeq(")")).is_none() {
                let err_span = p.cursor.peek_span();
                let found = p.cursor.peek();
                return Err(p.emit_unclosed_at_eof_or_expected(err_span, &["\\)"], found, None));
            }

            let span = p.cursor.span_of_range(start_pos..p.cursor.pos());
            let (nodes, span_kids) = tree::decompose_children(children);
            Ok(ParsedNode::with_kids(
                SyntaxNode::Group {
                    mode: ContentMode::Math,
                    kind: GroupKind::InlineMath,
                    children: nodes,
                },
                span,
                span_kids,
            ))
        })
    }
}
