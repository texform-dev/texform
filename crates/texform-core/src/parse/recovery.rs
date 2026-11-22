//! Two-layer Sync-Set error recovery for failed content items.
//!
//! Operates purely on [`TokenParser`] state and [`Cursor`] primitives without
//! depending on `grammar/` or re-lexing the source.

use texform_interface::syntax_node::{ContentMode, SyntaxNode};

use crate::lexer::Token;
use crate::parse::parser::{
    Checkpoint, MathStop, PResult, ParseAbort, RecoveryHint, TokenParser, is_math_hard_stop,
};
use crate::parse::tree::ParsedNode;

/// Returns `true` when `token` belongs to the Sync-Set for `mode` and
/// `math_stop`.
#[inline]
pub(crate) fn is_sync_token(token: Token<'_>, mode: ContentMode, math_stop: MathStop) -> bool {
    match mode {
        ContentMode::Math => is_math_hard_stop(Some(token), math_stop),
        ContentMode::Text => matches!(
            token,
            Token::RBrace | Token::MathShift | Token::ControlSeq("end")
        ),
    }
}

impl TokenParser<'_, '_, '_> {
    /// Recover from a failed content item that started at `cp` and aborted with
    /// `hint`, wrapping the skipped source slice in a [`SyntaxNode::Error`]
    /// node.
    ///
    /// When `config.abort_on_error` is `true`, returns `Err(ParseAbort { hint })`
    /// immediately without consuming any recovery tokens.
    pub(crate) fn recover_item(
        &mut self,
        cp: Checkpoint,
        hint: RecoveryHint,
        mode: ContentMode,
    ) -> PResult<ParsedNode> {
        if self.config.abort_on_error {
            return Err(ParseAbort { hint });
        }

        let message = self
            .diagnostics()
            .get(cp.diag_len)
            .map(|diag| diag.message.clone())
            .unwrap_or_else(|| "parse error".to_string());

        match hint {
            RecoveryHint::Propagate | RecoveryHint::Fatal => Err(ParseAbort { hint }),

            RecoveryHint::MissingScriptContent => {
                // Keep the already-consumed base and script marker in place and
                // replace them with an Error node without crossing following items.
                let end_pos = self.cursor.pos();
                if end_pos <= cp.pos {
                    return Err(ParseAbort { hint });
                }
                let span = self.cursor.span_of_range(cp.pos..end_pos);
                let snippet = self.slice_snippet(span);
                Ok(ParsedNode::leaf(
                    SyntaxNode::Error { message, snippet },
                    span,
                ))
            }

            RecoveryHint::UnknownCommandHead => {
                // Unknown commands have no argspec, so consume only the command
                // token itself and leave any following `{...}` group intact.
                self.cursor.set_pos(cp.pos);
                self.cursor.skip_ws();
                let start_pos = self.cursor.pos();
                if self.cursor.bump().is_none() {
                    return Err(ParseAbort { hint });
                }
                let span = self.cursor.span_of_range(start_pos..self.cursor.pos());
                let snippet = self.slice_snippet(span);
                Ok(ParsedNode::leaf(
                    SyntaxNode::Error { message, snippet },
                    span,
                ))
            }

            RecoveryHint::CommandModeError => {
                // Consume the offending command token plus any immediately
                // following balanced `[...]` / `{...}` groups so valid arguments
                // are not scattered into secondary errors.
                self.cursor.set_pos(cp.pos);
                self.cursor.skip_ws();
                let start_pos = self.cursor.pos();
                if self.cursor.bump().is_none() {
                    return Err(ParseAbort { hint });
                }

                loop {
                    let before_ws = self.cursor.pos();
                    self.cursor.skip_ws();
                    match self.cursor.peek() {
                        Some(Token::LBracket) => {
                            if self.cursor.scan_bracketed(true).is_err() {
                                break;
                            }
                        }
                        Some(Token::LBrace) => {
                            if self.cursor.scan_braced(true).is_err() {
                                break;
                            }
                        }
                        _ => {
                            self.cursor.set_pos(before_ws);
                            break;
                        }
                    }
                }

                let span = self.cursor.span_of_range(start_pos..self.cursor.pos());
                let snippet = self.slice_snippet(span);
                Ok(ParsedNode::leaf(
                    SyntaxNode::Error { message, snippet },
                    span,
                ))
            }

            RecoveryHint::Skip => self.recover_skip(cp, hint, mode, message),
        }
    }

    fn recover_skip(
        &mut self,
        cp: Checkpoint,
        hint: RecoveryHint,
        mode: ContentMode,
        message: String,
    ) -> PResult<ParsedNode> {
        self.cursor.set_pos(cp.pos);
        if mode == ContentMode::Math {
            self.cursor.skip_ws();
        }
        let start_pos = self.cursor.pos();
        let math_stop = self.math_stop();
        let mut consumed = false;
        let mut opened_env = false;
        let mut nested_envs = 0usize;

        loop {
            let before_ws = self.cursor.pos();
            self.cursor.skip_ws();

            if let Some((end_pos, _, _)) = self.cursor.peek_env_marker(self.src, "begin") {
                self.cursor.set_pos(end_pos);
                if opened_env {
                    nested_envs += 1;
                } else {
                    opened_env = true;
                }
                consumed = true;
                continue;
            }

            if let Some((end_pos, end_name, _)) = self.cursor.peek_env_marker(self.src, "end") {
                let belongs_to_outer_env = self
                    .active_envs()
                    .iter()
                    .any(|env| env.name.as_ref() == end_name.as_ref());
                if belongs_to_outer_env {
                    // Leave outer environments' closing tags for their owners.
                    self.cursor.set_pos(before_ws);
                    break;
                }
                if nested_envs > 0 {
                    self.cursor.set_pos(end_pos);
                    nested_envs -= 1;
                    consumed = true;
                    continue;
                }
                if opened_env {
                    // Close (or absorb the mismatched `\end{...}` of) the failed
                    // environment that started at `cp`.
                    self.cursor.set_pos(end_pos);
                    consumed = true;
                    break;
                }
                if consumed {
                    self.cursor.set_pos(before_ws);
                    break;
                }
                // Stray unmatched `\end{...}` at the very start of recovery:
                // consume the marker once so the content loop makes progress.
                self.cursor.set_pos(end_pos);
                consumed = true;
                break;
            }

            match self.cursor.peek() {
                None => {
                    self.cursor.set_pos(before_ws);
                    break;
                }
                Some(Token::ControlSeq("right")) if mode == ContentMode::Math => {
                    self.cursor.set_pos(before_ws);
                    break;
                }
                Some(tok) if consumed && is_sync_token(tok, mode, math_stop) => {
                    self.cursor.set_pos(before_ws);
                    break;
                }
                _ => {
                    self.cursor.set_pos(before_ws);
                }
            }

            if self.cursor.bump().is_none() {
                break;
            }
            consumed = true;
        }

        if !consumed {
            return Err(ParseAbort { hint });
        }

        let span = self.cursor.span_of_range(start_pos..self.cursor.pos());
        let snippet = self.slice_snippet(span);
        Ok(ParsedNode::leaf(
            SyntaxNode::Error { message, snippet },
            span,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::lexer::lex_source;
    use crate::parse::config::ParseConfig;
    use crate::parse::context::ParseContext;
    use crate::parse::cursor::Cursor;
    use crate::parse::diagnostics;
    use crate::parse::parser::OpenEnv;
    use crate::parse::result::Span;

    fn parser_for<'tok, 'src, 'ctx>(
        stream: &'tok crate::lexer::TokenStream<'src>,
        src: &'src str,
        ctx: &'ctx ParseContext,
        config: &'ctx ParseConfig,
    ) -> TokenParser<'tok, 'src, 'ctx> {
        TokenParser::new(Cursor::from_stream(stream, src.len()), src, ctx, config)
    }

    #[test]
    fn abort_on_error_rethrows_immediately() {
        let src = r"\mycmd{x}";
        let stream = lex_source(src).unwrap();
        let ctx = ParseContext::empty();
        let config = ParseConfig::STRICT;
        let mut p = parser_for(&stream, src, &ctx, &config);

        let cp = p.checkpoint();
        p.emit_diag(diagnostics::unknown_command("mycmd", Span::new(0, 6)));
        let err = p
            .recover_item(cp, RecoveryHint::UnknownCommandHead, ContentMode::Math)
            .unwrap_err();
        assert_eq!(err.hint, RecoveryHint::UnknownCommandHead);
        assert_eq!(p.cursor.pos(), 0);
    }

    #[test]
    fn propagate_hint_is_not_recovered() {
        let src = r"\left\foo x \right)";
        let stream = lex_source(src).unwrap();
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut p = parser_for(&stream, src, &ctx, &config);

        let cp = p.checkpoint();
        p.emit_diag(diagnostics::left_right_delimiter(
            "invalid \\left delimiter",
            Span::new(5, 9),
        ));
        let err = p
            .recover_item(cp, RecoveryHint::Propagate, ContentMode::Math)
            .unwrap_err();
        assert_eq!(err.hint, RecoveryHint::Propagate);

        let fatal_err = p
            .recover_item(cp, RecoveryHint::Fatal, ContentMode::Math)
            .unwrap_err();
        assert_eq!(fatal_err.hint, RecoveryHint::Fatal);
    }

    #[test]
    fn unknown_command_head_consumes_only_command_token() {
        let src = r"\mycmd{x}+y";
        let stream = lex_source(src).unwrap();
        let ctx = ParseContext::empty();
        let config = ParseConfig {
            reject_unknown: true,
            abort_on_error: false,
            ..Default::default()
        };
        let mut p = parser_for(&stream, src, &ctx, &config);

        let cp = p.checkpoint();
        p.emit_diag(diagnostics::unknown_command("mycmd", Span::new(0, 6)));
        let node = p
            .recover_item(cp, RecoveryHint::UnknownCommandHead, ContentMode::Math)
            .unwrap();
        assert_eq!(
            node.node,
            SyntaxNode::Error {
                message: r"Unknown command: \mycmd".to_string(),
                snippet: r"\mycmd".to_string(),
            }
        );
        assert_eq!(node.span, Span::new(0, 6));
        assert_eq!(p.cursor.peek(), Some(Token::LBrace));
    }

    #[test]
    fn command_mode_error_consumes_trailing_bracket_and_brace_groups() {
        let src = r"\frac[opt]{a}{b} + z";
        let stream = lex_source(src).unwrap();
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut p = parser_for(&stream, src, &ctx, &config);

        let cp = p.checkpoint();
        p.emit_diag(diagnostics::command_mode_error(
            "frac",
            ContentMode::Text,
            Span::new(0, 5),
        ));
        let node = p
            .recover_item(cp, RecoveryHint::CommandModeError, ContentMode::Text)
            .unwrap();
        assert_eq!(
            node.node,
            SyntaxNode::Error {
                message: r"Command \frac is not allowed in text mode".to_string(),
                snippet: r"\frac[opt]{a}{b}".to_string(),
            }
        );
        assert_eq!(node.span, Span::new(0, 16));
        // Trailing space before `+ z` is preserved for the next sibling.
        assert_eq!(p.cursor.peek(), Some(Token::Whitespaces));
    }

    #[test]
    fn missing_script_content_builds_in_place_error_node() {
        let src = "{x^}";
        let stream = lex_source(src).unwrap();
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut p = parser_for(&stream, src, &ctx, &config);

        p.cursor.bump(); // '{'
        let cp = p.checkpoint();
        p.cursor.bump(); // 'x'
        p.cursor.bump(); // '^'
        p.emit_diag(diagnostics::missing_script_content(
            true,
            Span::new(3, 4),
            Some(Token::RBrace),
        ));

        let node = p
            .recover_item(cp, RecoveryHint::MissingScriptContent, ContentMode::Math)
            .unwrap();
        assert_eq!(
            node.node,
            SyntaxNode::Error {
                message: "Missing superscript content".to_string(),
                snippet: "x^".to_string(),
            }
        );
        assert_eq!(node.span, Span::new(1, 3));
        assert_eq!(p.cursor.peek(), Some(Token::RBrace));
    }

    #[test]
    fn skip_stops_before_outer_active_env_and_consumes_mismatched_env_end() {
        // Case 1: outer `matrix` is active; inner `\begin{align} x` fails and
        // must stop before `\end{matrix}`.
        let src_outer = r"\begin{align} x \end{matrix}";
        let stream_outer = lex_source(src_outer).unwrap();
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut p = parser_for(&stream_outer, src_outer, &ctx, &config);

        let outer_env = OpenEnv::new(
            Cow::Borrowed("matrix"),
            Span::new(0, 0),
            Span::new(0, 0),
            ContentMode::Math,
        );
        let node = p.with_env(outer_env, false, |p| {
            let cp = p.checkpoint();
            p.emit_diag(diagnostics::environment_name_mismatch(
                "align",
                "matrix",
                Span::new(16, 28),
            ));
            p.recover_item(cp, RecoveryHint::Skip, ContentMode::Math)
                .unwrap()
        });
        assert_eq!(
            node.node,
            SyntaxNode::Error {
                message: r"Environment name mismatch: expected \end{align}, found \end{matrix}"
                    .to_string(),
                snippet: r"\begin{align} x".to_string(),
            }
        );
        assert_eq!(p.cursor.peek(), Some(Token::Whitespaces));
        p.cursor.skip_ws();
        assert_eq!(p.cursor.peek(), Some(Token::ControlSeq("end")));

        // Case 2: no outer active env; `\begin{matrix} x \end{align} + z`
        // consumes the mismatched `\end{align}` and stops before `+ z`.
        let src_mismatch = r"\begin{matrix} x \end{align} + z";
        let stream_mismatch = lex_source(src_mismatch).unwrap();
        let mut p2 = parser_for(&stream_mismatch, src_mismatch, &ctx, &config);
        let cp = p2.checkpoint();
        p2.emit_diag(diagnostics::environment_name_mismatch(
            "matrix",
            "align",
            Span::new(17, 28),
        ));
        let node2 = p2
            .recover_item(cp, RecoveryHint::Skip, ContentMode::Math)
            .unwrap();
        assert_eq!(
            node2.node,
            SyntaxNode::Error {
                message: r"Environment name mismatch: expected \end{matrix}, found \end{align}"
                    .to_string(),
                snippet: r"\begin{matrix} x \end{align}".to_string(),
            }
        );
        p2.cursor.skip_ws();
        assert_eq!(p2.cursor.peek(), Some(Token::Char('+')));
    }

    #[test]
    fn skip_respects_control_paren_math_stop() {
        let src = r"a b \) + c";
        let stream = lex_source(src).unwrap();
        let ctx = ParseContext::empty();
        let config = ParseConfig::LENIENT;
        let mut p = parser_for(&stream, src, &ctx, &config);

        let node = p.with_math_stop(MathStop::ControlParen, |p| {
            let cp = p.checkpoint();
            p.emit_diag(diagnostics::unknown_command("bad", Span::new(0, 1)));
            p.recover_item(cp, RecoveryHint::Skip, ContentMode::Math)
                .unwrap()
        });
        assert_eq!(
            node.node,
            SyntaxNode::Error {
                message: r"Unknown command: \bad".to_string(),
                snippet: "a b".to_string(),
            }
        );
        p.cursor.skip_ws();
        assert_eq!(p.cursor.peek(), Some(Token::ControlSeq(")")));
    }
}
