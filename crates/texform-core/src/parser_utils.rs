//! Parser utilities shared across parser modules.
//!
//! This module hosts small reusable helpers and state machines that are
//! easier to test and maintain outside of the main combinator pipeline.

use chumsky::{
    input::{Cursor, InputRef},
    prelude::*,
};

use crate::lexer::Token;
use texform_interface::syntax_node::{ContentMode, GroupKind, SyntaxNode};

pub type ParserError<'a> = extra::Err<Rich<'a, Token>>;
pub type TokenInput<'a> = &'a [Token];
pub type ParserInput<'src, 'parse> = InputRef<'src, 'parse, TokenInput<'src>, ParserError<'src>>;

/// Ergonomic helpers for building spans and custom errors in imperative parsers.
pub trait ParserInputExt<'src, 'parse> {
    fn pos(&self) -> usize;

    fn span_from_cursor(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
    ) -> SimpleSpan<usize>;

    fn err_since(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token>;

    fn err_at_cursor(&self, msg: impl ToString) -> Rich<'src, Token>;

    /// Build an error for the next token without consuming it. Falls back to a
    /// point span at EOF.
    fn err_peek_or_point(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token>;
}

impl<'src, 'parse> ParserInputExt<'src, 'parse> for ParserInput<'src, 'parse> {
    #[inline]
    fn pos(&self) -> usize {
        *self.cursor().inner()
    }

    #[inline]
    fn span_from_cursor(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
    ) -> SimpleSpan<usize> {
        self.span_since(start)
    }

    #[inline]
    fn err_since(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token> {
        Rich::custom(self.span_from_cursor(start), msg)
    }

    #[inline]
    fn err_at_cursor(&self, msg: impl ToString) -> Rich<'src, Token> {
        let pos = self.pos();
        Rich::custom(SimpleSpan::new((), pos..pos), msg)
    }

    #[inline]
    fn err_peek_or_point(
        &mut self,
        start: &Cursor<'src, 'parse, TokenInput<'src>>,
        msg: impl ToString,
    ) -> Rich<'src, Token> {
        let start_pos = *start.inner();
        let end = if self.peek().is_some() {
            start_pos + 1
        } else {
            start_pos
        };
        Rich::custom(SimpleSpan::new((), start_pos..end), msg)
    }
}

/// Superscript state while scanning a script sequence.
///
/// We distinguish three cases so we can implement MathJax-compatible
/// error semantics:
/// - `Prime`: only prime markers contributed to the superscript so far
/// - `Explicit`: a normal superscript atom (no primes)
/// - `Mixed`: combination of primes and normal superscripts
#[derive(Clone)]
pub enum SupState {
    Prime(SyntaxNode),
    Explicit(SyntaxNode),
    Mixed(SyntaxNode),
}

/// Result of parsing scripted atom components.
///
/// This is the minimal data needed to build a `SyntaxNode::Scripted`:
/// we keep the base node and optional subscript/superscript nodes.
pub struct ScriptComponents {
    pub base: SyntaxNode,
    pub subscript: Option<SyntaxNode>,
    pub superscript: Option<SyntaxNode>,
}

/// Parse a scripted atom sequence starting from an optional base.
///
/// Responsibilities:
/// - Accept optional base atom, or synthesize an empty base when the
///   first token is a script marker (`^`, `_`, or prime)
/// - Scan an arbitrary-length sequence of script markers and their
///   arguments, folding primes and explicit superscripts into a single
///   superscript state
/// - Enforce MathJax-like error conditions:
///   - "Double exponent: use braces to clarify"
///   - "Double subscripts: use braces to clarify"
///   - "Prime causes double exponent: use braces to clarify"
///
/// On success this returns a `ScriptComponents` that the caller can wrap
/// into a `SyntaxNode::Scripted`.
pub fn parse_scripted_components<'src, 'parse, P>(
    input: &mut ParserInput<'src, 'parse>,
    atom_for_scripts: P,
) -> Result<ScriptComponents, Rich<'src, Token>>
where
    P: Parser<'src, TokenInput<'src>, SyntaxNode, ParserError<'src>> + Clone + 'src,
{
    let ws = select! { Token::Whitespaces => () }.repeated().ignored();

    // Optional base: either an atom or an empty implicit group when
    // the sequence starts with a script marker.
    let base_opt = input.parse(atom_for_scripts.clone().or_not())?;
    let base = match base_opt {
        Some(b) => b,
        None => match input.peek() {
            Some(Token::Superscript) | Some(Token::Subscript) | Some(Token::Prime(_)) => {
                SyntaxNode::Group {
                    mode: ContentMode::Math,
                    kind: GroupKind::Implicit,
                    children: vec![],
                }
            }
            _ => {
                let cursor = input.cursor();
                return Err(input.err_peek_or_point(&cursor, "expected atom or script marker"));
            }
        },
    };

    let mut sup_state: Option<SupState> = None;
    let mut subscript: Option<SyntaxNode> = None;

    loop {
        let _ = input.parse(ws.clone());

        let marker_start = input.cursor();

        let marker = match input.peek() {
            Some(Token::Superscript) => {
                input.next();
                let node = input.parse(atom_for_scripts.clone())?;
                Some(("sup", node))
            }
            Some(Token::Subscript) => {
                input.next();
                let node = input.parse(atom_for_scripts.clone())?;
                Some(("sub", node))
            }
            Some(Token::Prime(_)) => {
                let count = match input.next() {
                    Some(Token::Prime(n)) => n,
                    _ => unreachable!("peek ensured prime token"),
                };
                let node = if count == 1 {
                    SyntaxNode::Char('\'')
                } else {
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: (0..count).map(|_| SyntaxNode::Char('\'')).collect(),
                    }
                };
                Some(("prime", node))
            }
            _ => None,
        };

        let Some((kind, node)) = marker else {
            break;
        };

        match kind {
            "sub" => {
                if subscript.is_some() {
                    return Err(
                        input.err_since(&marker_start, "Double subscripts: use braces to clarify")
                    );
                }
                subscript = Some(node);
            }
            "sup" => {
                let current = sup_state.take();
                sup_state = match current {
                    None => Some(SupState::Explicit(node)),
                    Some(SupState::Prime(existing)) => Some(SupState::Mixed(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![existing, node],
                    })),
                    Some(SupState::Explicit(_)) | Some(SupState::Mixed(_)) => {
                        return Err(input
                            .err_since(&marker_start, "Double exponent: use braces to clarify"));
                    }
                };
            }
            "prime" => {
                let current = sup_state.take();
                sup_state = match current {
                    None => Some(SupState::Prime(node)),
                    Some(SupState::Prime(existing)) => Some(SupState::Mixed(SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: vec![existing, node],
                    })),
                    Some(SupState::Explicit(_)) | Some(SupState::Mixed(_)) => {
                        return Err(input.err_since(
                            &marker_start,
                            "Prime causes double exponent: use braces to clarify",
                        ));
                    }
                };
            }
            _ => unreachable!("unsupported script kind"),
        }
    }

    let superscript = sup_state.map(|s| match s {
        SupState::Prime(n) | SupState::Explicit(n) | SupState::Mixed(n) => n,
    });

    Ok(ScriptComponents {
        base,
        subscript,
        superscript,
    })
}
