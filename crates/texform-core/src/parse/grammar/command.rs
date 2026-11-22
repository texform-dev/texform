//! Control-sequence dispatch for math and text modes (prefix, declarative,
//! escaped symbols, bare delimiter controls, mode errors, and unknown commands).

use texform_interface::syntax_node::{ContentMode, SyntaxNode};

use crate::knowledge::{ArgSpec, CommandKind};
use crate::lexer::Token;
use crate::parse::diagnostics;
use crate::parse::parser::{PResult, ParseAbort, RecoveryHint, TokenParser};
use crate::parse::result::ParseDiagnosticKind;
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
/// contained a `\left` or `\begin` error. Preserves [`RecoveryHint::Propagate`]
/// only for global recursion-depth overflow.
#[inline]
pub(super) fn downgrade_inner_arg_abort(
    parser: &TokenParser<'_, '_, '_>,
    abort: ParseAbort,
) -> ParseAbort {
    if abort.hint == RecoveryHint::Propagate
        && parser
            .diagnostics()
            .last()
            .is_some_and(|d| d.kind == Some(ParseDiagnosticKind::MaxGroupDepthExceeded))
    {
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
    pub(super) fn parse_math_control_seq(&mut self, name: &'src str) -> PResult<ParsedNode> {
        match name {
            "left" => return self.parse_delimited_group(),
            "begin" => return self.parse_environment(ContentMode::Math),
            "right" | "end" => {
                let span = self.cursor.peek_span();
                return Err(self.emit_expected(
                    span,
                    &["math atom"],
                    Some(Token::ControlSeq(name)),
                ));
            }
            _ => {}
        }

        if let Some(meta) = self.ctx.lookup_command(name, ContentMode::Math) {
            match meta.kind {
                CommandKind::Declarative => {
                    let args_spec = meta.argspec.args;
                    return self.parse_declarative_command(name, args_spec, ContentMode::Math);
                }
                CommandKind::Prefix => {
                    let args_spec = meta.argspec.args;
                    return self.parse_prefix_command(name, args_spec, ContentMode::Math);
                }
                CommandKind::Infix => {
                    // An infix command in bare-atom position (e.g. `x^\over`) is
                    // not a valid prefix/declarative head; fall through only to
                    // escaped symbol / delimiter control before rejecting.
                    if let Some(ch) = escaped_symbol_char(name) {
                        let (_, span) = self.cursor.bump().expect("peek ensured ControlSeq");
                        return Ok(ParsedNode::leaf(SyntaxNode::Char(ch), span));
                    }
                    if self
                        .ctx
                        .lookup_delimiter(name, true, ContentMode::Math)
                        .is_some()
                    {
                        let (_, span) = self.cursor.bump().expect("peek ensured ControlSeq");
                        return Ok(ParsedNode::leaf(
                            SyntaxNode::Command {
                                name: name.to_owned(),
                                args: Vec::new(),
                                known: true,
                            },
                            span,
                        ));
                    }
                    let span = self.cursor.peek_span();
                    return Err(self.emit_expected(
                        span,
                        &["math atom"],
                        Some(Token::ControlSeq(name)),
                    ));
                }
            }
        }

        if let Some(ch) = escaped_symbol_char(name) {
            let (_, span) = self.cursor.bump().expect("peek ensured ControlSeq");
            return Ok(ParsedNode::leaf(SyntaxNode::Char(ch), span));
        }

        if self
            .ctx
            .lookup_delimiter(name, true, ContentMode::Math)
            .is_some()
        {
            let (_, span) = self.cursor.bump().expect("peek ensured ControlSeq");
            return Ok(ParsedNode::leaf(
                SyntaxNode::Command {
                    name: name.to_owned(),
                    args: Vec::new(),
                    known: true,
                },
                span,
            ));
        }

        if self.ctx.lookup_command(name, ContentMode::Text).is_some() {
            let span = self.cursor.peek_span();
            let contexts = self.snapshot_contexts(span.end);
            return Err(self.emit_and_abort(
                diagnostics::command_mode_error(name, ContentMode::Math, span, contexts),
                RecoveryHint::CommandModeError,
            ));
        }

        self.parse_unknown_command(name)
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
    pub(super) fn parse_text_control_seq(&mut self, name: &'src str) -> PResult<ParsedNode> {
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
                    return self.parse_environment(ContentMode::Text);
                }
            }
            "end" => {
                let span = self.cursor.peek_span();
                return Err(self.emit_expected(
                    span,
                    &["text item"],
                    Some(Token::ControlSeq(name)),
                ));
            }
            _ => {}
        }

        if let Some(ch) = escaped_symbol_char(name) {
            let (_, span) = self.cursor.bump().expect("peek ensured ControlSeq");
            return Ok(ParsedNode::leaf(SyntaxNode::Char(ch), span));
        }

        if let Some(meta) = self.ctx.lookup_command(name, ContentMode::Text) {
            match meta.kind {
                CommandKind::Declarative => {
                    let args_spec = meta.argspec.args;
                    return self.parse_declarative_command(name, args_spec, ContentMode::Text);
                }
                CommandKind::Prefix => {
                    let args_spec = meta.argspec.args;
                    return self.parse_prefix_command(name, args_spec, ContentMode::Text);
                }
                CommandKind::Infix => {
                    let span = self.cursor.peek_span();
                    return Err(self.emit_expected(
                        span,
                        &["text item"],
                        Some(Token::ControlSeq(name)),
                    ));
                }
            }
        }

        if self.ctx.lookup_command(name, ContentMode::Math).is_some() {
            let span = self.cursor.peek_span();
            let contexts = self.snapshot_contexts(span.end);
            return Err(self.emit_and_abort(
                diagnostics::command_mode_error(name, ContentMode::Text, span, contexts),
                RecoveryHint::CommandModeError,
            ));
        }

        self.parse_unknown_command(name)
    }

    /// Parse a registered prefix command and its argument slots.
    fn parse_prefix_command(
        &mut self,
        name: &'src str,
        args_spec: &'static [ArgSpec],
        mode: ContentMode,
    ) -> PResult<ParsedNode> {
        let cmd_start_pos = self.cursor.pos();
        let (_, head_span) = self.cursor.bump().expect("peek ensured ControlSeq");

        if args_spec.is_empty() {
            return Ok(ParsedNode::leaf(
                SyntaxNode::Command {
                    name: name.to_owned(),
                    args: Vec::new(),
                    known: true,
                },
                head_span,
            ));
        }

        // `\\` is the only command whose optional `[...]` argument spacing
        // depends on the enclosing environment (`array`/`darray`/`eqnarray`).
        let allow_row_break_opt_space = name == "\\" && self.row_break_skips_spaces();
        let cmd_args = match self.parse_argument_slots(
            args_spec,
            mode,
            "command argument",
            allow_row_break_opt_space,
        ) {
            Ok(args) => args,
            Err(abort) => return Err(downgrade_inner_arg_abort(self, abort)),
        };

        let span = self.cursor.span_of_range(cmd_start_pos..self.cursor.pos());
        let (args, span_kids) = tree::decompose_args(cmd_args);
        Ok(ParsedNode::with_kids(
            SyntaxNode::Command {
                name: name.to_owned(),
                args,
                known: true,
            },
            span,
            span_kids,
        ))
    }

    /// Parse a registered declarative command and its argument slots.
    fn parse_declarative_command(
        &mut self,
        name: &'src str,
        args_spec: &'static [ArgSpec],
        mode: ContentMode,
    ) -> PResult<ParsedNode> {
        let cmd_start_pos = self.cursor.pos();
        let (_, head_span) = self.cursor.bump().expect("peek ensured ControlSeq");

        if args_spec.is_empty() {
            return Ok(ParsedNode::leaf(
                SyntaxNode::Declarative {
                    name: name.to_owned(),
                    args: Vec::new(),
                },
                head_span,
            ));
        }

        let cmd_args =
            match self.parse_argument_slots(args_spec, mode, "declarative command argument", false)
            {
                Ok(args) => args,
                Err(abort) => return Err(downgrade_inner_arg_abort(self, abort)),
            };

        let span = self.cursor.span_of_range(cmd_start_pos..self.cursor.pos());
        let (args, span_kids) = tree::decompose_args(cmd_args);
        Ok(ParsedNode::with_kids(
            SyntaxNode::Declarative {
                name: name.to_owned(),
                args,
            },
            span,
            span_kids,
        ))
    }

    /// Handle an unrecognized control sequence according to `config.reject_unknown`.
    fn parse_unknown_command(&mut self, name: &'src str) -> PResult<ParsedNode> {
        if self.config.reject_unknown {
            let span = self.cursor.peek_span();
            let contexts = self.snapshot_contexts(span.end);
            return Err(self.emit_and_abort(
                diagnostics::unknown_command(name, span, contexts),
                RecoveryHint::UnknownCommandHead,
            ));
        }
        let (_, span) = self.cursor.bump().expect("peek ensured ControlSeq");
        Ok(ParsedNode::leaf(
            SyntaxNode::Command {
                name: name.to_owned(),
                args: Vec::new(),
                known: false,
            },
            span,
        ))
    }
}
