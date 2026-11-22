//! Text-mode content loop, text atom coalescing, and text argument shorthand.

use texform_interface::syntax_node::{ContentMode, SyntaxNode};

use crate::lexer::Token;
use crate::parse::diagnostics;
use crate::parse::parser::{PResult, RecoveryHint, TokenParser};
use crate::parse::tree::ParsedNode;

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Parse a sequence of text-mode items until `}`, `\end`, or EOF.
    ///
    /// Preserves trailing whitespace before `\end`, or before `}` / EOF when at
    /// least one item has already been parsed.
    pub(super) fn parse_text_content(&mut self) -> PResult<Vec<ParsedNode>> {
        self.with_recursion(|p| {
            let mut items = Vec::new();

            loop {
                let leading_ws = p.cursor.peek() == Some(Token::Whitespaces);
                let next_non_ws = p.cursor.peek_non_ws().map(|(_, tok, _)| tok);
                let natural_end = matches!(next_non_ws, None | Some(Token::RBrace));
                let at_stop = matches!(next_non_ws, Some(Token::ControlSeq("end")));

                if leading_ws && (at_stop || (natural_end && !items.is_empty())) {
                    // Items such as `$x$`, `\^n`, or a single-token argument leave
                    // a following space unconsumed; keep it as text content before
                    // the enclosing group or environment closes.
                    let cp = p.checkpoint();
                    let item = match p.parse_text_item() {
                        Ok(item) => item,
                        Err(abort) => p.recover_item(cp, abort.hint, ContentMode::Text)?,
                    };
                    items.push(item);
                    break;
                }
                if natural_end || at_stop {
                    break;
                }

                let cp = p.checkpoint();
                let item = match p.parse_text_item() {
                    Ok(item) => item,
                    Err(abort) => p.recover_item(cp, abort.hint, ContentMode::Text)?,
                };
                items.push(item);
            }

            Ok(items)
        })
    }

    /// Parse a single text-mode item.
    pub(super) fn parse_text_item(&mut self) -> PResult<ParsedNode> {
        match self.cursor.peek() {
            Some(
                Token::Char(_)
                | Token::Whitespaces
                | Token::Star
                | Token::LBracket
                | Token::RBracket
                | Token::Prime(_),
            ) => Ok(self.parse_text_chunk()),
            Some(Token::ControlSeq(name)) => {
                let item = self.parse_text_control_seq(name)?;
                // Whitespace terminating an argument-free pure-alphabetic control
                // word is lexical, not text content.
                if matches!(
                    &item.node,
                    SyntaxNode::Command { name, args, .. } | SyntaxNode::Declarative { name, args }
                        if args.is_empty() && name.chars().all(|c| c.is_ascii_alphabetic())
                ) {
                    self.cursor.skip_ws();
                }
                Ok(item)
            }
            Some(Token::MathShift) => self.parse_inline_math_dollar(),
            Some(Token::LBrace) => self.parse_braced_group(ContentMode::Text),
            Some(Token::Superscript | Token::Subscript) => {
                let (_, span) = self.cursor.bump().expect("peek ensured script marker");
                let contexts = self.snapshot_contexts(span.end);
                Err(self.emit_and_abort(
                    diagnostics::text_script_error(span, contexts),
                    RecoveryHint::Skip,
                ))
            }
            Some(Token::ActiveChar) => {
                let (_, span) = self.cursor.bump().expect("peek ensured ActiveChar");
                Ok(ParsedNode::leaf(SyntaxNode::ActiveSpace, span))
            }
            other => {
                let span = self.cursor.peek_span();
                Err(self.emit_expected(span, &["text item"], other))
            }
        }
    }

    /// Coalesce adjacent text-like tokens into a single [`SyntaxNode::Text`],
    /// folding consecutive whitespace runs into a single ASCII space.
    fn parse_text_chunk(&mut self) -> ParsedNode {
        let start_pos = self.cursor.pos();
        let mut buf = String::new();
        let mut last_was_space = false;

        while let Some(tok) = self.cursor.peek() {
            match tok {
                Token::Char(c) => {
                    self.cursor.bump();
                    buf.push(c);
                    last_was_space = false;
                }
                Token::Whitespaces => {
                    self.cursor.bump();
                    if !last_was_space {
                        buf.push(' ');
                        last_was_space = true;
                    }
                }
                Token::Star => {
                    self.cursor.bump();
                    buf.push('*');
                    last_was_space = false;
                }
                Token::LBracket => {
                    self.cursor.bump();
                    buf.push('[');
                    last_was_space = false;
                }
                Token::RBracket => {
                    self.cursor.bump();
                    buf.push(']');
                    last_was_space = false;
                }
                Token::Prime(n) => {
                    self.cursor.bump();
                    for _ in 0..n {
                        buf.push('\'');
                    }
                    last_was_space = false;
                }
                _ => break,
            }
        }

        let span = self.cursor.span_of_range(start_pos..self.cursor.pos());
        ParsedNode::leaf(SyntaxNode::Text(buf), span)
    }

    /// Parse an unbraced mandatory text-mode argument.
    ///
    /// An undelimited TeX argument consumes a single token (e.g. `\mbox TeV`
    /// captures only `T`), so single character-like tokens are read one at a
    /// time instead of being coalesced into a running word.
    pub(super) fn parse_text_argument_item(&mut self) -> PResult<ParsedNode> {
        self.cursor.skip_ws();
        match self.cursor.peek() {
            Some(Token::Char(c)) => {
                let (_, span) = self.cursor.bump().expect("peek ensured Char");
                Ok(ParsedNode::leaf(SyntaxNode::Text(c.to_string()), span))
            }
            Some(Token::Star) => {
                let (_, span) = self.cursor.bump().expect("peek ensured Star");
                Ok(ParsedNode::leaf(SyntaxNode::Text("*".to_string()), span))
            }
            Some(Token::LBracket) => {
                let (_, span) = self.cursor.bump().expect("peek ensured LBracket");
                Ok(ParsedNode::leaf(SyntaxNode::Text("[".to_string()), span))
            }
            Some(Token::RBracket) => {
                let (_, span) = self.cursor.bump().expect("peek ensured RBracket");
                Ok(ParsedNode::leaf(SyntaxNode::Text("]".to_string()), span))
            }
            Some(Token::Prime(n)) => {
                let (_, span) = self.cursor.bump().expect("peek ensured Prime");
                Ok(ParsedNode::leaf(SyntaxNode::Text("'".repeat(n)), span))
            }
            Some(Token::ControlSeq("end")) => {
                let span = self.cursor.peek_span();
                Err(self.emit_expected(span, &["text item"], Some(Token::ControlSeq("end"))))
            }
            _ => self.parse_text_item(),
        }
    }
}
