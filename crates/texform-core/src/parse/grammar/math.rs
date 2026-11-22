//! Math-mode content loop, infix tails, scripts/primes, and math atom dispatch.

use texform_interface::syntax_node::{ContentMode, GroupKind, SyntaxNode};

use crate::knowledge::CommandKind;
use crate::lexer::Token;
use crate::parse::cursor::token_as_literal_char;
use crate::parse::diagnostics;
use crate::parse::parser::{MathStop, PResult, RecoveryHint, TokenParser, is_math_hard_stop};
use crate::parse::result::Span;
use crate::parse::tree::{self, ParsedNode};

/// Tracks how a math item's superscript was built to detect double-exponent errors.
///
/// Primes (`f'`) and an explicit caret (`^`) may be combined once (`f'^2`),
/// whereas a second explicit caret or a prime run after an explicit caret or
/// intervening subscript (`f'_a'`) is a double-exponent error.
enum SupState {
    Prime(ParsedNode),
    Explicit(ParsedNode),
    Mixed(ParsedNode),
}

/// Tokens that can never begin a script operand. `]` and `&` are valid math
/// atoms and therefore are not operand boundaries.
#[inline]
fn is_script_operand_boundary(token: Option<Token<'_>>) -> bool {
    is_math_hard_stop(token, MathStop::Default)
        || matches!(
            token,
            Some(Token::Superscript | Token::Subscript | Token::Prime(_))
        )
}

/// Returns `true` when `node` ends an alignment cell (`&`, `\\`, `\cr`,
/// `\newline`). Cell separators never bind following scripts.
#[inline]
fn is_cell_separator(node: &SyntaxNode) -> bool {
    match node {
        SyntaxNode::AlignmentTab => true,
        SyntaxNode::Command { name, .. } => matches!(name.as_str(), "\\" | "cr" | "newline"),
        _ => false,
    }
}

/// Empty implicit group used as a synthetic script base at byte offset `pos`.
#[inline]
fn empty_script_base(pos: usize) -> ParsedNode {
    ParsedNode::leaf(SyntaxNode::empty_group(ContentMode::Math), Span::point(pos))
}

/// Assemble a [`SyntaxNode::Scripted`] node with `[base, subscript?, superscript?]`
/// child span ordering.
fn fold_scripted(
    base: ParsedNode,
    subscript: Option<ParsedNode>,
    superscript: Option<ParsedNode>,
    span: Span,
) -> ParsedNode {
    let mut span_kids = Vec::with_capacity(
        1 + usize::from(subscript.is_some()) + usize::from(superscript.is_some()),
    );
    let (base_node, base_tree) = base.into_span_tree();
    span_kids.push(base_tree);

    let sub_node = subscript.map(|sub| {
        let (node, tree) = sub.into_span_tree();
        span_kids.push(tree);
        Box::new(node)
    });
    let sup_node = superscript.map(|sup| {
        let (node, tree) = sup.into_span_tree();
        span_kids.push(tree);
        Box::new(node)
    });

    ParsedNode::with_kids(
        SyntaxNode::Scripted {
            base: Box::new(base_node),
            subscript: sub_node,
            superscript: sup_node,
        },
        span,
        span_kids,
    )
}

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Returns `true` if `name` is registered as an infix command in either mode.
    #[inline]
    pub(super) fn is_infix_command(&self, name: &str) -> bool {
        self.ctx
            .lookup_command(name, ContentMode::Math)
            .is_some_and(|meta| meta.kind == CommandKind::Infix)
            || self
                .ctx
                .lookup_command(name, ContentMode::Text)
                .is_some_and(|meta| meta.kind == CommandKind::Infix)
    }

    /// Single source of truth for math content loop stop boundaries.
    #[inline]
    pub(super) fn is_math_stop(&self, token: Option<Token<'_>>) -> bool {
        if is_math_hard_stop(token, self.math_stop()) {
            return true;
        }
        matches!(token, Some(Token::ControlSeq(name)) if self.is_infix_command(name))
    }

    /// Parse math content items plus an optional infix tail.
    ///
    /// Consumes leading and trailing insignificant whitespace so the enclosing
    /// span covers the full padded math sequence.
    pub(super) fn parse_math_content(&mut self) -> PResult<Vec<ParsedNode>> {
        self.with_recursion(|p| {
            p.cursor.skip_ws();
            let content_start = p.cursor.byte_pos();
            let mut leading = Vec::new();

            loop {
                p.cursor.skip_ws();
                if p.is_math_stop(p.cursor.peek()) {
                    break;
                }
                let cp = p.checkpoint();
                match p.parse_math_item() {
                    Ok(item) => leading.push(item),
                    Err(abort) => {
                        let recovered = p.recover_item(cp, abort.hint, ContentMode::Math)?;
                        leading.push(recovered);
                    }
                }
            }

            if let Some(Token::ControlSeq(name)) = p.cursor.peek()
                && let Some(meta) = p.ctx.lookup_command(name, ContentMode::Math)
                && meta.kind == CommandKind::Infix
            {
                let args_spec = meta.argspec.args;
                let infix_node = p.parse_infix_tail(leading, content_start, name, args_spec)?;
                p.cursor.skip_ws();
                return Ok(vec![infix_node]);
            }

            p.cursor.skip_ws();
            Ok(leading)
        })
    }

    /// Parse the infix command head, its arguments, and its right-hand item list.
    fn parse_infix_tail(
        &mut self,
        leading: Vec<ParsedNode>,
        content_start: usize,
        name: &'src str,
        args_spec: &'static [crate::knowledge::ArgSpec],
    ) -> PResult<ParsedNode> {
        self.cursor.bump(); // consume infix control sequence

        let cmd_args = if args_spec.is_empty() {
            Vec::new()
        } else {
            self.parse_argument_slots(args_spec, "infix command argument", false)
                .map_err(super::command::downgrade_inner_arg_abort)?
        };

        let mut right_items = Vec::new();
        loop {
            let before_ws = self.cursor.pos();
            self.cursor.skip_ws();

            if is_math_hard_stop(self.cursor.peek(), self.math_stop()) {
                self.cursor.set_pos(before_ws);
                break;
            }

            if let Some(Token::ControlSeq(second_name)) = self.cursor.peek()
                && self.is_infix_command(second_name)
            {
                let err_span = self.cursor.peek_span();
                return self.fail_err(
                    diagnostics::ambiguous_infix(second_name, err_span),
                    RecoveryHint::Skip,
                );
            }

            let cp = self.checkpoint();
            match self.parse_math_item() {
                Ok(item) => right_items.push(item),
                Err(abort) if abort.hint == RecoveryHint::MissingScriptContent => {
                    let recovered = self.recover_item(cp, abort.hint, ContentMode::Math)?;
                    right_items.push(recovered);
                }
                Err(abort) => return Err(abort),
            }
        }

        let content_end = self.cursor.prev_end_byte();
        let left_span = tree::items_span(&leading, content_start);
        let left = tree::fold_items(ContentMode::Math, leading, left_span);
        let right_span = tree::items_span(&right_items, content_end);
        let right = tree::fold_items(ContentMode::Math, right_items, right_span);

        let (args, args_kids) = tree::decompose_args(cmd_args);
        let (left_node, left_tree) = left.into_span_tree();
        let (right_node, right_tree) = right.into_span_tree();

        let infix_span = Span::new(left_tree.span.start, right_tree.span.end);
        // Infix span subtree order is [left, args..., right].
        let mut span_kids = Vec::with_capacity(args_kids.len() + 2);
        span_kids.push(left_tree);
        span_kids.extend(args_kids);
        span_kids.push(right_tree);

        Ok(ParsedNode::with_kids(
            SyntaxNode::Infix {
                name: name.to_owned(),
                args,
                left: Box::new(left_node),
                right: Box::new(right_node),
            },
            infix_span,
            span_kids,
        ))
    }

    /// Parse a single math item (base atom plus any attached `_`, `^`, and prime
    /// runs).
    pub(super) fn parse_math_item(&mut self) -> PResult<ParsedNode> {
        self.cursor.skip_ws();
        let item_start_pos = self.cursor.pos();
        let item_start_byte = self.cursor.byte_pos();

        let base = if matches!(
            self.cursor.peek(),
            Some(Token::Superscript | Token::Subscript | Token::Prime(_))
        ) {
            empty_script_base(item_start_byte)
        } else {
            self.parse_math_atom()?
        };

        // Alignment tabs and row breaks end a cell; following scripts start the
        // next cell with an empty base instead of attaching to the separator.
        if is_cell_separator(&base.node) {
            return Ok(base);
        }

        // Fast path (>80% of math atoms): no script or prime follows.
        let has_script = matches!(
            self.cursor.peek_non_ws(),
            Some((
                _,
                Token::Superscript | Token::Subscript | Token::Prime(_),
                _
            ))
        );
        if !has_script {
            return Ok(base);
        }

        self.parse_math_item_scripts(base, item_start_pos)
    }

    /// Parse trailing `_`, `^`, and prime runs attached to `base`.
    ///
    /// Kept out-of-line from [`Self::parse_math_item`] so recursive descent
    /// through nested atoms does not carry script-loop locals on the stack.
    #[inline(never)]
    fn parse_math_item_scripts(
        &mut self,
        base: ParsedNode,
        item_start_pos: usize,
    ) -> PResult<ParsedNode> {
        let mut sup_state: Option<SupState> = None;
        let mut subscript: Option<ParsedNode> = None;

        while let Some((
            marker_pos,
            marker_tok @ (Token::Superscript | Token::Subscript | Token::Prime(_)),
            _,
        )) = self.cursor.peek_non_ws()
        {
            self.cursor.set_pos(marker_pos);

            match marker_tok {
                Token::Superscript => {
                    self.cursor.bump();
                    let tracked = self.parse_script_operand(true)?;
                    let span = self.cursor.span_of_range(marker_pos..self.cursor.pos());
                    let tracked = ParsedNode { span, ..tracked };
                    match sup_state.take() {
                        None => sup_state = Some(SupState::Explicit(tracked)),
                        Some(SupState::Prime(existing)) => {
                            let merged_span = Span::new(existing.span.start, tracked.span.end);
                            let SyntaxNode::Prime { count } = existing.node else {
                                unreachable!("prime script state must contain a prime run");
                            };
                            let prime_known = self
                                .ctx
                                .lookup_command("prime", ContentMode::Math)
                                .is_some();
                            let mut children: Vec<_> = (0..count)
                                .map(|_| SyntaxNode::Command {
                                    name: "prime".to_owned(),
                                    args: Vec::new(),
                                    known: prime_known,
                                })
                                .collect();
                            children.push(tracked.node);
                            let merged = ParsedNode::leaf(
                                SyntaxNode::Group {
                                    mode: ContentMode::Math,
                                    kind: GroupKind::Implicit,
                                    children,
                                },
                                merged_span,
                            );
                            sup_state = Some(SupState::Mixed(merged));
                        }
                        Some(SupState::Explicit(_)) | Some(SupState::Mixed(_)) => {
                            let err_span = self.cursor.span_of_range(marker_pos..self.cursor.pos());
                            return self.fail_err(
                                diagnostics::custom_error(
                                    "Double exponent: use braces to clarify",
                                    err_span,
                                ),
                                RecoveryHint::Skip,
                            );
                        }
                    }
                }
                Token::Subscript => {
                    self.cursor.bump();
                    let tracked = self.parse_script_operand(false)?;
                    let span = self.cursor.span_of_range(marker_pos..self.cursor.pos());
                    let tracked = ParsedNode { span, ..tracked };
                    if subscript.is_some() {
                        return self.fail_err(
                            diagnostics::custom_error(
                                "Double subscripts: use braces to clarify",
                                span,
                            ),
                            RecoveryHint::Skip,
                        );
                    }
                    subscript = Some(tracked);
                }
                Token::Prime(first_count) => {
                    self.cursor.bump();
                    let mut count = first_count;
                    while let Some((next_pos, Token::Prime(next_count), _)) =
                        self.cursor.peek_non_ws()
                    {
                        self.cursor.set_pos(next_pos);
                        self.cursor.bump();
                        count += next_count;
                    }
                    let prime_span = self.cursor.span_of_range(marker_pos..self.cursor.pos());
                    let tracked = ParsedNode::leaf(SyntaxNode::Prime { count }, prime_span);
                    match sup_state.take() {
                        None => sup_state = Some(SupState::Prime(tracked)),
                        Some(SupState::Prime(_))
                        | Some(SupState::Explicit(_))
                        | Some(SupState::Mixed(_)) => {
                            return self.fail_err(
                                diagnostics::custom_error(
                                    "Prime causes double exponent: use braces to clarify",
                                    prime_span,
                                ),
                                RecoveryHint::Skip,
                            );
                        }
                    }
                }
                _ => unreachable!("peek_non_ws matched script or prime marker"),
            }
        }

        let superscript = sup_state.map(|state| match state {
            SupState::Prime(node) | SupState::Explicit(node) | SupState::Mixed(node) => node,
        });

        if subscript.is_none() && superscript.is_none() {
            return Ok(base);
        }

        let span = self.cursor.span_of_range(item_start_pos..self.cursor.pos());
        Ok(fold_scripted(base, subscript, superscript, span))
    }

    /// Parse the operand of a `^` or `_` marker, emitting `MissingScriptContent`
    /// before entering the atom parser when positioned at an operand boundary.
    fn parse_script_operand(&mut self, is_superscript: bool) -> PResult<ParsedNode> {
        self.cursor.skip_ws();
        let next = self.cursor.peek();
        if is_script_operand_boundary(next) {
            let err_span = self.cursor.peek_span();
            return self.fail_err(
                diagnostics::missing_script_content(is_superscript, err_span, next),
                RecoveryHint::MissingScriptContent,
            );
        }
        self.parse_math_atom()
    }

    /// Parse a single math atom without consuming trailing scripts.
    ///
    /// Hot-path ordinary math characters are dispatched first in a single
    /// `match` with no lookahead backtracking or hash lookups.
    pub(super) fn parse_math_atom(&mut self) -> PResult<ParsedNode> {
        if let Some((ch, span)) = self.cursor.eat_map(token_as_literal_char) {
            return Ok(ParsedNode::leaf(SyntaxNode::Char(ch), span));
        }
        if let Some(span) = self.cursor.eat(Token::Alignment) {
            return Ok(ParsedNode::leaf(SyntaxNode::AlignmentTab, span));
        }
        if let Some(span) = self.cursor.eat(Token::ActiveChar) {
            return Ok(ParsedNode::leaf(SyntaxNode::ActiveSpace, span));
        }
        match self.cursor.peek() {
            Some(Token::LBrace) => self.parse_braced_group(ContentMode::Math),
            Some(Token::ControlSeq(name)) => {
                let head_span = self.cursor.peek_span();
                self.parse_math_control_seq(name, head_span)
            }
            Some(Token::MathShift) => {
                let span = self.cursor.peek_span();
                self.fail_err(
                    diagnostics::unexpected_math_shift(self.src, span),
                    RecoveryHint::Skip,
                )
            }
            other => {
                let span = self.cursor.peek_span();
                Err(self.emit_expected(span, &["math atom"], other))
            }
        }
    }

    /// Parse an unbraced mandatory math argument (`\sqrt x^2`, `\frac 1 2`).
    ///
    /// Leaves trailing `_`, `^`, and prime tokens for the enclosing scripted
    /// parser. A leading `^`/`_` is parsed as an empty-base scripted atom, and
    /// a leading quote consumes only that single `Prime` token wrapped as an
    /// empty-base superscript.
    pub(super) fn parse_math_atom_argument(&mut self) -> PResult<ParsedNode> {
        self.cursor.skip_ws();
        if let Some((count, span)) = self.cursor.eat_map(|tok| match tok {
            Token::Prime(count) => Some(count),
            _ => None,
        }) {
            let prime = ParsedNode::leaf(SyntaxNode::Prime { count }, span);
            let base = empty_script_base(span.start);
            return Ok(fold_scripted(base, None, Some(prime), span));
        }
        match self.cursor.peek() {
            Some(Token::ControlSeq("right" | "end")) => {
                let span = self.cursor.peek_span();
                let found = self.cursor.peek();
                Err(self.emit_expected(span, &["math atom"], found))
            }
            Some(Token::ControlSeq(name)) if self.is_infix_command(name) => {
                let span = self.cursor.peek_span();
                let found = self.cursor.peek();
                Err(self.emit_expected(span, &["math atom"], found))
            }
            Some(Token::Superscript | Token::Subscript) => self.parse_math_item(),
            _ => self.parse_math_atom(),
        }
    }
}
