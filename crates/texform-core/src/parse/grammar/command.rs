//! Control-sequence dispatch for math and text modes (prefix, declarative,
//! escaped symbols, bare delimiter controls, mode errors, and unknown commands).

use texform_interface::syntax_node::{ContentMode, SyntaxNode};

use crate::knowledge::{ArgSpec, CommandKind};
use crate::lexer::Token;
use crate::parse::diagnostics;
use crate::parse::parser::{PResult, ParseAbort, RecoveryHint, TokenParser};
use crate::parse::result::Span;
use crate::parse::tree::{self, ParsedNode};

/// Map the seven TeX escaped symbol control sequences to their literal characters.
#[inline]
fn escaped_symbol_char(name: &str) -> Option<char> {
    match name {
        "%" => Some('%'),
        "$" => Some('$'),
        "&" => Some('&'),
        "#" => Some('#'),
        "_" => Some('_'),
        "{" => Some('{'),
        "}" => Some('}'),
        _ => None,
    }
}

/// Downgrade item-head recovery hints that originated inside a command or
/// environment argument slot so outer recovery does not truncate the enclosing
/// command head or prevent recovering an enclosing command whose argument
/// contained a `\left` or `\begin` error. Preserves [`RecoveryHint::Fatal`]
/// for unrecoverable global parser failures (recursion-depth overflow).
#[inline]
pub(super) fn downgrade_inner_arg_abort(abort: ParseAbort) -> ParseAbort {
    if abort.hint == RecoveryHint::Fatal {
        abort
    } else {
        ParseAbort {
            hint: RecoveryHint::Skip,
        }
    }
}

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Dispatch a control sequence in math mode in precedence order:
    ///
    /// 1. `\left` -> delimited group
    /// 2. `\begin` -> environment
    /// 3. `\right` / `\end` -> unexpected closer error
    /// 4. Single `ctx.lookup_command(name, Math)` -> `Declarative` / `Prefix`
    /// 5. Escaped symbol (`\%`, `\$`, `\&`, `\#`, `\_`, `\{`, `\}`)
    /// 6. Bare delimiter control (`ctx.lookup_delimiter(name, true, Math)`)
    /// 7. `ctx.lookup_command(name, Text)` -> `CommandModeError`
    /// 8. Unknown command (`UnknownCommand` when `reject_unknown`, else `known: false`)
    pub(super) fn parse_math_control_seq(
        &mut self,
        name: &'src str,
        head_span: Span,
    ) -> PResult<ParsedNode> {
        match name {
            "left" => return self.parse_delimited_group(),
            "begin" => return self.parse_environment(ContentMode::Math, head_span),
            "right" | "end" => {
                return Err(self.emit_expected(
                    head_span,
                    &["math atom"],
                    Some(Token::ControlSeq(name)),
                ));
            }
            _ => {}
        }

        let mut is_infix = false;
        if let Some(meta) = self.ctx.lookup_command(name, ContentMode::Math) {
            match meta.kind {
                CommandKind::Declarative => {
                    return self.parse_registered_command(name, head_span, meta.argspec.args, true);
                }
                CommandKind::Prefix => {
                    return self.parse_registered_command(
                        name,
                        head_span,
                        meta.argspec.args,
                        false,
                    );
                }
                CommandKind::Infix => {
                    // An infix command in bare-atom position (e.g. `x^\over`) is
                    // not a valid prefix/declarative head; fall through only to
                    // escaped symbol / delimiter control before rejecting.
                    is_infix = true;
                }
            }
        }

        if let Some(ch) = escaped_symbol_char(name) {
            self.cursor.bump();
            return Ok(ParsedNode::leaf(SyntaxNode::Char(ch), head_span));
        }

        if self
            .ctx
            .lookup_delimiter(name, true, ContentMode::Math)
            .is_some()
        {
            self.cursor.bump();
            return Ok(ParsedNode::leaf(
                SyntaxNode::Command {
                    name: name.to_owned(),
                    args: Vec::new(),
                    known: true,
                },
                head_span,
            ));
        }

        if is_infix {
            return Err(self.emit_expected(
                head_span,
                &["math atom"],
                Some(Token::ControlSeq(name)),
            ));
        }

        if self.ctx.lookup_command(name, ContentMode::Text).is_some() {
            return self.fail_err(
                diagnostics::command_mode_error(name, ContentMode::Math, head_span),
                RecoveryHint::CommandModeError,
            );
        }

        self.parse_unknown_command(name, head_span)
    }

    /// Dispatch a control sequence in text mode in precedence order:
    ///
    /// 1. `\(` -> inline math
    /// 2. `\begin` -> environment
    /// 3. `\end` -> unexpected closer error
    /// 4. Escaped symbol BEFORE KB lookup
    /// 5. Single `ctx.lookup_command(name, Text)` -> `Declarative` / `Prefix`
    /// 6. `ctx.lookup_command(name, Math)` -> `CommandModeError`
    /// 7. Unknown command (`UnknownCommand` when `reject_unknown`, else `known: false`)
    pub(super) fn parse_text_control_seq(
        &mut self,
        name: &'src str,
        head_span: Span,
    ) -> PResult<ParsedNode> {
        match name {
            "(" => {
                // If `\(` has no matching `\)`, fall back to command parsing
                // when `\(` is either registered in text mode or unknown
                // commands are allowed.
                let cp = self.checkpoint();
                match self.parse_inline_math_paren() {
                    Ok(node) => return Ok(node),
                    Err(abort) => {
                        if self.ctx.lookup_command("(", ContentMode::Text).is_some()
                            || !self.config.reject_unknown
                        {
                            self.rewind(cp);
                        } else {
                            return Err(abort);
                        }
                    }
                }
            }
            "begin" => {
                let starts_env = matches!(self.cursor.peek_at_non_ws_after(1), Some(Token::LBrace));
                if starts_env
                    || self
                        .ctx
                        .lookup_command("begin", ContentMode::Text)
                        .is_none()
                {
                    return self.parse_environment(ContentMode::Text, head_span);
                }
            }
            "end" => {
                return Err(self.emit_expected(
                    head_span,
                    &["text item"],
                    Some(Token::ControlSeq(name)),
                ));
            }
            _ => {}
        }

        if let Some(ch) = escaped_symbol_char(name) {
            self.cursor.bump();
            return Ok(ParsedNode::leaf(SyntaxNode::Char(ch), head_span));
        }

        if let Some(meta) = self.ctx.lookup_command(name, ContentMode::Text) {
            match meta.kind {
                CommandKind::Declarative => {
                    return self.parse_registered_command(name, head_span, meta.argspec.args, true);
                }
                CommandKind::Prefix => {
                    return self.parse_registered_command(
                        name,
                        head_span,
                        meta.argspec.args,
                        false,
                    );
                }
                CommandKind::Infix => {
                    return Err(self.emit_expected(
                        head_span,
                        &["text item"],
                        Some(Token::ControlSeq(name)),
                    ));
                }
            }
        }

        if self.ctx.lookup_command(name, ContentMode::Math).is_some() {
            return self.fail_err(
                diagnostics::command_mode_error(name, ContentMode::Text, head_span),
                RecoveryHint::CommandModeError,
            );
        }

        self.parse_unknown_command(name, head_span)
    }

    /// Parse a registered prefix or declarative command and its argument slots.
    fn parse_registered_command(
        &mut self,
        name: &'src str,
        head_span: Span,
        args_spec: &'static [ArgSpec],
        is_declarative: bool,
    ) -> PResult<ParsedNode> {
        let cmd_start_pos = self.cursor.pos();
        self.cursor.bump();

        let build_node = |args| {
            if is_declarative {
                SyntaxNode::Declarative {
                    name: name.to_owned(),
                    args,
                }
            } else {
                SyntaxNode::Command {
                    name: name.to_owned(),
                    args,
                    known: true,
                }
            }
        };

        if args_spec.is_empty() {
            return Ok(ParsedNode::leaf(build_node(Vec::new()), head_span));
        }

        // `\\` is the only command whose optional `[...]` argument spacing
        // depends on the enclosing environment (`array`/`darray`/`eqnarray`).
        let allow_row_break_opt_space =
            !is_declarative && name == "\\" && self.row_break_skips_spaces();
        let context_label = if is_declarative {
            "declarative command argument"
        } else {
            "command argument"
        };
        let cmd_args = self
            .parse_argument_slots(args_spec, context_label, allow_row_break_opt_space)
            .map_err(downgrade_inner_arg_abort)?;

        let span = self.cursor.span_of_range(cmd_start_pos..self.cursor.pos());
        let (args, span_kids) = tree::decompose_args(cmd_args);
        Ok(ParsedNode::with_kids(build_node(args), span, span_kids))
    }

    /// Handle an unrecognized control sequence according to `config.reject_unknown`.
    fn parse_unknown_command(&mut self, name: &'src str, head_span: Span) -> PResult<ParsedNode> {
        if self.config.reject_unknown {
            return self.fail_err(
                diagnostics::unknown_command(name, head_span),
                RecoveryHint::UnknownCommandHead,
            );
        }
        self.cursor.bump();
        Ok(ParsedNode::leaf(
            SyntaxNode::Command {
                name: name.to_owned(),
                args: Vec::new(),
                known: false,
            },
            head_span,
        ))
    }
}
