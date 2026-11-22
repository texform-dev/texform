//! Environment parsing: `\begin{name}[args]...body...\end{name}`, live
//! `env_stack` management, row-break spacing policy, and mismatch recovery.

use std::borrow::Cow;

use texform_interface::syntax_node::{ContentMode, GroupKind, SyntaxNode};

use crate::lexer::Token;
use crate::parse::diagnostics;
use crate::parse::parser::{OpenEnv, PResult, ParseAbort, RecoveryHint, TokenParser};
use crate::parse::result::Span;
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
        let Some(env_name) = self.cursor.eat_env_name(self.src) else {
            let err_span = self.cursor.peek_span();
            let found = self.cursor.peek();
            return Err(self.emit_unclosed_at_eof_or_expected(
                err_span,
                &["environment name"],
                found,
                None,
            ));
        };
        let Some(close_span) = self.cursor.eat(Token::RBrace) else {
            let err_span = self.cursor.peek_span();
            let found = self.cursor.peek();
            return Err(self.emit_unclosed_at_eof_or_expected(err_span, &["'}'"], found, None));
        };

        Ok((env_name, Span::new(open_span.start, close_span.end)))
    }

    /// Parse a full `\begin{name}[args]...body...\end{name}` environment.
    pub(super) fn parse_environment(
        &mut self,
        current_mode: ContentMode,
        begin_span: Span,
    ) -> PResult<ParsedNode> {
        self.with_recursion(|p| {
            let env_start_pos = p.cursor.pos();
            p.cursor.bump(); // consume `\begin`
            p.cursor.skip_ws();

            let (name, name_span) = p.parse_env_name()?;
            let header_span = Span::new(begin_span.start, name_span.end);

            let (cmd_args, known, body_mode) = if let Some(meta) =
                p.ctx.lookup_env(name.as_ref(), current_mode)
            {
                let args_spec = meta.argspec.args;
                let body_mode = meta.body_mode;
                let cmd_args = if args_spec.is_empty() {
                    Vec::new()
                } else {
                    p.parse_argument_slots(args_spec, "environment argument", false)
                        .map_err(downgrade_inner_arg_abort)?
                };
                (cmd_args, true, body_mode)
            } else if p
                .ctx
                .lookup_env(name.as_ref(), other_mode(current_mode))
                .is_some()
            {
                return p.fail_err(
                    diagnostics::environment_mode_error(name.as_ref(), current_mode, header_span),
                    RecoveryHint::Skip,
                );
            } else if p.config.reject_unknown {
                return p.fail_err(
                    diagnostics::unknown_environment(name.as_ref(), header_span),
                    RecoveryHint::Propagate,
                );
            } else {
                (Vec::new(), false, current_mode)
            };

            let body_start_pos = p.cursor.pos();
            let body_start_byte = p.cursor.byte_pos();
            let row_break_skips = known && row_break_skips_spaces_in_environment(name.as_ref());
            let open_env = OpenEnv::new(name.clone(), begin_span, name_span, body_mode);

            let (body, eof_reported) = p.with_env(open_env, row_break_skips, |p| {
                p.with_context("environment body", body_start_byte, |p| {
                    let children = match body_mode {
                        ContentMode::Math => p.parse_math_content()?,
                        ContentMode::Text => p.parse_text_content()?,
                    };
                    let body_span = p.cursor.span_of_range(body_start_pos..p.cursor.pos());
                    let (nodes, span_kids) = tree::decompose_children(children);
                    let eof_reported = p.active_env_eof_reported();
                    Ok((
                        ParsedNode::with_kids(
                            SyntaxNode::Group {
                                mode: body_mode,
                                kind: GroupKind::Implicit,
                                children: nodes,
                            },
                            body_span,
                            span_kids,
                        ),
                        eof_reported,
                    ))
                })
            })?;

            // Match `\end{found}`. At this point `with_env` has popped the
            // current environment, so `p.active_envs()` holds strictly outer
            // open environments.
            p.parse_environment_closing(name.as_ref(), eof_reported)?;

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
    fn parse_environment_closing(&mut self, name: &str, eof_reported: bool) -> PResult<()> {
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
                        self.fail_err(
                            diagnostics::environment_name_mismatch(
                                name,
                                found_name.as_ref(),
                                end_span,
                            ),
                            RecoveryHint::Skip,
                        )
                    }
                } else {
                    // Incomplete `\end` header (e.g. `\end` or `\end{matrix`):
                    // span covers the entire partial header through EOF or
                    // the first unexpected token.
                    let start_pos = self.cursor.pos();
                    let end_head_span = self.cursor.peek_span();
                    self.cursor.bump();
                    let mut end_byte = end_head_span.end;
                    let after_head = self.cursor.pos();
                    self.cursor.skip_ws();
                    if let Some(brace_span) = self.cursor.eat(Token::LBrace) {
                        end_byte = brace_span.end;
                        while let Some((_, tok_span)) = self
                            .cursor
                            .eat_if(|t| matches!(t, Token::Char(_) | Token::Star))
                        {
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
                    self.fail_err(
                        diagnostics::unclosed_environment(name, partial_span, found),
                        RecoveryHint::Skip,
                    )
                }
            }
            None => {
                if eof_reported {
                    return Err(ParseAbort {
                        hint: RecoveryHint::Skip,
                    });
                }
                let span = self.cursor.peek_span();
                let diag_idx = self.diagnostics().len();
                self.emit_diag(diagnostics::unclosed_environment(name, span, None));
                self.mark_active_envs_eof_reported(name, diag_idx);
                Err(ParseAbort {
                    hint: RecoveryHint::Skip,
                })
            }
            Some(other) => {
                let span = self.cursor.peek_span();
                let found = Some(diagnostics::format_token_label(other));
                self.fail_err(
                    diagnostics::unclosed_environment(name, span, found),
                    RecoveryHint::Skip,
                )
            }
        }
    }
}
