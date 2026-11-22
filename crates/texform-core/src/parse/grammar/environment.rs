//! Environment parsing: `\begin{name}[args]...body...\end{name}`, live
//! `env_stack` management, row-break spacing policy, and mismatch recovery.

use std::borrow::Cow;

use texform_interface::syntax_node::{ContentMode, GroupKind, SyntaxNode};

use crate::lexer::Token;
use crate::parse::diagnostics;
use crate::parse::parser::{OpenEnv, PResult, ParseAbort, RecoveryHint, TokenParser};
use crate::parse::result::{ParseDiagnosticKind, Span};
use crate::parse::tree::{self, ParsedNode};

use super::command::downgrade_inner_arg_abort;
use super::other_mode;

/// Whether `\\` inside environment `name` skips spaces before its `[...]`
/// argument.
///
/// Matches LaTeX's distinction between `\@arraycr` (`array`, `darray`,
/// `eqnarray`, `eqnarray*`) and amsmath's `\math@cr`.
#[inline]
fn row_break_skips_spaces_in_environment(name: &str) -> bool {
    matches!(name, "array" | "darray" | "eqnarray" | "eqnarray*")
}

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Parse `{name}` for an environment header (`Char` and `Star` tokens only).
    ///
    /// Borrows `name` directly from `self.src` when tokens are contiguous;
    /// allocates only when a comment splits the name in the source.
    fn parse_env_name(&mut self) -> PResult<(Cow<'src, str>, Span)> {
        let open_span = self.expect(Token::LBrace, "'{'")?;
        let name_tok_start = self.cursor.pos();
        let mut first_byte = 0usize;
        let mut prev_end = 0usize;
        let mut contiguous = true;

        while let Some(tok) = self.cursor.peek() {
            match tok {
                Token::Char(_) | Token::Star => {
                    let (_, span) = self.cursor.bump().expect("peek ensured Char or Star");
                    if self.cursor.pos() == name_tok_start + 1 {
                        first_byte = span.start;
                    } else if span.start != prev_end {
                        contiguous = false;
                    }
                    prev_end = span.end;
                }
                _ => break,
            }
        }

        let name_tok_end = self.cursor.pos();
        if name_tok_end == name_tok_start {
            let err_span = self.cursor.peek_span();
            let found = self.cursor.peek();
            return Err(self.emit_unclosed_at_eof_or_expected(
                err_span,
                &["environment name"],
                found,
                None,
            ));
        }

        let close_span = if self.cursor.peek() == Some(Token::RBrace) {
            self.cursor.bump().expect("peek ensured RBrace").1
        } else {
            let err_span = self.cursor.peek_span();
            let found = self.cursor.peek();
            return Err(self.emit_unclosed_at_eof_or_expected(err_span, &["'}'"], found, None));
        };

        let env_name = if contiguous {
            Cow::Borrowed(&self.src[first_byte..prev_end])
        } else {
            let mut owned = String::with_capacity(name_tok_end - name_tok_start);
            for tok in &self.cursor.tokens_slice()[name_tok_start..name_tok_end] {
                match tok {
                    Token::Char(c) => owned.push(*c),
                    Token::Star => owned.push('*'),
                    _ => unreachable!("env name loop only consumes Char and Star"),
                }
            }
            Cow::Owned(owned)
        };

        Ok((env_name, Span::new(open_span.start, close_span.end)))
    }

    /// Parse a full `\begin{name}[args]...body...\end{name}` environment.
    pub(super) fn parse_environment(&mut self, current_mode: ContentMode) -> PResult<ParsedNode> {
        self.with_recursion(|p| {
            let env_start_pos = p.cursor.pos();
            let (_, begin_span) = p.cursor.bump().expect("peek ensured \\begin");
            p.cursor.skip_ws();

            let (name, name_span) = p.parse_env_name()?;
            let header_span = Span::new(begin_span.start, name_span.end);

            let (cmd_args, known, body_mode) =
                if let Some(meta) = p.ctx.lookup_env(name.as_ref(), current_mode) {
                    let args_spec = meta.argspec.args;
                    let body_mode = meta.body_mode;
                    let cmd_args = if args_spec.is_empty() {
                        Vec::new()
                    } else {
                        match p.parse_argument_slots(
                            args_spec,
                            current_mode,
                            "environment argument",
                            false,
                        ) {
                            Ok(args) => args,
                            Err(abort) => return Err(downgrade_inner_arg_abort(p, abort)),
                        }
                    };
                    (cmd_args, true, body_mode)
                } else if p
                    .ctx
                    .lookup_env(name.as_ref(), other_mode(current_mode))
                    .is_some()
                {
                    // Strict parsing pins `EnvironmentModeError` to the full `\begin{name}`
                    // header (matching `parser_diagnostics::strict_expected_found_errors_*`),
                    // while lenient recovery keeps the nested `{name}` span (matching the
                    // facade contract in `parse_states::recovered_environment_mode_*`).
                    let err_span = if p.config.abort_on_error {
                        header_span
                    } else {
                        name_span
                    };
                    let contexts = p.snapshot_contexts(err_span.end);
                    return Err(p.emit_and_abort(
                        diagnostics::environment_mode_error(
                            name.as_ref(),
                            current_mode,
                            err_span,
                            contexts,
                        ),
                        RecoveryHint::Skip,
                    ));
                } else if p.config.reject_unknown {
                    let contexts = p.snapshot_contexts(header_span.end);
                    return Err(p.emit_and_abort(
                        diagnostics::unknown_environment(name.as_ref(), header_span, contexts),
                        RecoveryHint::Propagate,
                    ));
                } else {
                    (Vec::new(), false, current_mode)
                };

            let body_start_pos = p.cursor.pos();
            let body_start_byte = p.cursor.byte_pos();
            let row_break_skips = known && row_break_skips_spaces_in_environment(name.as_ref());
            let open_env = OpenEnv {
                name: name.clone(),
                begin_span,
                name_span,
                body_mode,
            };

            let body = p.with_env(open_env, row_break_skips, |p| {
                p.with_context("environment body", body_start_byte, |p| {
                    let children = match body_mode {
                        ContentMode::Math => p.parse_math_content()?,
                        ContentMode::Text => p.parse_text_content()?,
                    };
                    let body_span = p.cursor.span_of_range(body_start_pos..p.cursor.pos());
                    let (nodes, span_kids) = tree::decompose_children(children);
                    Ok(ParsedNode::with_kids(
                        SyntaxNode::Group {
                            mode: body_mode,
                            kind: GroupKind::Implicit,
                            children: nodes,
                        },
                        body_span,
                        span_kids,
                    ))
                })
            })?;

            // Match `\end{found}`. At this point `with_env` has popped the
            // current environment, so `p.active_envs()` holds strictly outer
            // open environments.
            p.parse_environment_closing(name.as_ref())?;

            let span = p.cursor.span_of_range(env_start_pos..p.cursor.pos());
            let (args, mut span_kids) = tree::decompose_args(cmd_args);
            let (body_node, body_tree) = body.into_span_tree();
            // Environment span subtree order is [args..., body].
            span_kids.push(body_tree);

            Ok(ParsedNode::with_kids(
                SyntaxNode::Environment {
                    name: name.into_owned(),
                    args,
                    known,
                    body: Box::new(body_node),
                },
                span,
                span_kids,
            ))
        })
    }

    /// Match the closing `\end{name}` marker or emit a structured mismatch /
    /// unclosed diagnostic. Split out of [`Self::parse_environment`] so deeply
    /// nested environments keep compact stack frames while parsing their body.
    #[inline(never)]
    fn parse_environment_closing(&mut self, name: &str) -> PResult<()> {
        match self.cursor.peek() {
            Some(Token::ControlSeq("end")) => {
                if let Some((end_pos, found_name, end_span)) =
                    self.cursor.peek_env_marker(self.src, "end")
                {
                    if found_name.as_ref() == name {
                        self.cursor.set_pos(end_pos);
                        Ok(())
                    } else {
                        let belongs_to_outer = self
                            .active_envs()
                            .iter()
                            .any(|env| env.name.as_ref() == found_name.as_ref());
                        if !belongs_to_outer {
                            // Likely typo in closing tag: consume `\end{found}`
                            // so it is not left dangling.
                            self.cursor.set_pos(end_pos);
                        }
                        let contexts = self.snapshot_contexts(end_span.end);
                        Err(self.emit_and_abort(
                            diagnostics::environment_name_mismatch(
                                name,
                                found_name.as_ref(),
                                end_span,
                                contexts,
                            ),
                            RecoveryHint::Skip,
                        ))
                    }
                } else {
                    // Incomplete `\end` header (e.g. `\end` or `\end{matrix`):
                    // span covers the entire partial header through EOF or
                    // the first unexpected token.
                    let start_pos = self.cursor.pos();
                    let (_, end_head_span) = self.cursor.bump().expect("peek ensured \\end");
                    let mut end_byte = end_head_span.end;
                    let after_head = self.cursor.pos();
                    self.cursor.skip_ws();
                    if self.cursor.peek() == Some(Token::LBrace) {
                        let (_, brace_span) = self.cursor.bump().expect("peek ensured LBrace");
                        end_byte = brace_span.end;
                        while let Some(Token::Char(_) | Token::Star) = self.cursor.peek() {
                            let (_, tok_span) = self.cursor.bump().unwrap();
                            end_byte = tok_span.end;
                        }
                    } else {
                        self.cursor.set_pos(after_head);
                    }
                    self.cursor.set_pos(start_pos);
                    let partial_span = Span::new(end_head_span.start, end_byte);
                    let found = self
                        .src
                        .get(partial_span.start..partial_span.end)
                        .map(str::to_owned)
                        .or_else(|| Some("\\end".to_string()));
                    let contexts = self.snapshot_contexts(partial_span.end);
                    Err(self.emit_and_abort(
                        diagnostics::unclosed_environment(name, partial_span, found, contexts),
                        RecoveryHint::Skip,
                    ))
                }
            }
            None => {
                let span = self.cursor.peek_span();
                let expected_end = format!("\\end{{{name}}}");
                let already_reported = self.diagnostics().iter().rev().any(|diag| {
                    diag.kind == Some(ParseDiagnosticKind::UnclosedEnvironment)
                        && diag.span == span
                        && diag.expected.first().map(String::as_str) == Some(expected_end.as_str())
                });
                if already_reported {
                    return Err(ParseAbort {
                        hint: RecoveryHint::Skip,
                    });
                }
                let contexts = self.snapshot_contexts(span.end);
                Err(self.emit_and_abort(
                    diagnostics::unclosed_environment(name, span, None, contexts),
                    RecoveryHint::Skip,
                ))
            }
            Some(other) => {
                let span = self.cursor.peek_span();
                let found = Some(diagnostics::format_token_label(other));
                let contexts = self.snapshot_contexts(span.end);
                Err(self.emit_and_abort(
                    diagnostics::unclosed_environment(name, span, found, contexts),
                    RecoveryHint::Skip,
                ))
            }
        }
    }
}
