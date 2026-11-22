//! Zero-copy SoA token/span cursor and low-level delimiter scanning primitives.

use std::borrow::Cow;
use std::ops::Range;

use crate::lexer::{Token, TokenStream};
use crate::parse::result::Span;

/// Map ordinary character-like tokens (`Char`, `Star`, `LBracket`, `RBracket`)
/// to their literal character value.
#[inline]
pub(crate) fn token_as_literal_char(token: Token<'_>) -> Option<char> {
    match token {
        Token::Char(c) => Some(c),
        Token::Star => Some('*'),
        Token::LBracket => Some('['),
        Token::RBracket => Some(']'),
        _ => None,
    }
}

/// Result of scanning a balanced `{...}` or `[...]` delimiter pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DelimitedRange {
    /// Byte span of the opening delimiter token.
    pub open_span: Span,
    /// Token index range of the tokens strictly inside the delimiters.
    pub inner: Range<usize>,
    /// Byte span of the closing delimiter token.
    pub close_span: Span,
}

impl DelimitedRange {
    /// Byte span covering from the opening delimiter through the closing delimiter.
    #[inline]
    pub(crate) fn full_span(&self) -> Span {
        Span::new(self.open_span.start, self.close_span.end)
    }
}

/// Failure reasons returned by balanced brace/bracket scanners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BalancedScanError {
    /// The cursor did not point at the expected opening delimiter.
    MissingOpen,
    /// Reached end of window before finding the matching closing delimiter.
    Unclosed { open_span: Span },
    /// Encountered a nested `{` while `allow_nested` was `false`.
    NestedBrace { open_span: Span },
    /// Encountered an unmatched `}` inside a `[...]` bracket scan.
    UnbalancedBrace { open_span: Span },
}

/// Read-only cursor over parallel token and span slices (SoA layout).
///
/// The slice lifetime `'tok` is kept separate from the source lifetime `'src` so
/// a [`TokenStream`] owned on the stack can be borrowed by sub-windows without
/// coupling its lifetime to the input `&str`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor<'tok, 'src> {
    tokens: &'tok [Token<'src>],
    spans: &'tok [Span],
    pos: usize,
    /// Logical end-of-input byte offset of this window, used for empty input
    /// and trailing EOF spans.
    eoi_pos: usize,
}

impl<'tok, 'src> Cursor<'tok, 'src> {
    /// Construct a cursor positioned at the beginning of `(tokens, spans)`.
    #[inline]
    pub(crate) fn new(tokens: &'tok [Token<'src>], spans: &'tok [Span], eoi_pos: usize) -> Self {
        debug_assert_eq!(tokens.len(), spans.len());
        Self {
            tokens,
            spans,
            pos: 0,
            eoi_pos,
        }
    }

    /// Construct a cursor covering the whole [`TokenStream`].
    #[inline]
    pub(crate) fn from_stream(stream: &'tok TokenStream<'src>, eoi_pos: usize) -> Self {
        Self::new(&stream.tokens, &stream.spans, eoi_pos)
    }

    /// Returns `true` when all tokens in this window have been consumed.
    #[inline]
    pub(crate) fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Total number of tokens in this window.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Current token index within this window.
    #[inline]
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Reposition the cursor to an earlier or scanned token index.
    #[inline]
    pub(crate) fn set_pos(&mut self, pos: usize) {
        debug_assert!(pos <= self.tokens.len());
        self.pos = pos;
    }

    /// Logical end-of-input byte offset for this window.
    #[inline]
    pub(crate) fn eoi_pos(&self) -> usize {
        self.eoi_pos
    }

    /// Byte offset of the next token, or `eoi_pos` when at end of window.
    #[inline]
    pub(crate) fn byte_pos(&self) -> usize {
        self.spans
            .get(self.pos)
            .map_or(self.eoi_pos, |span| span.start)
    }

    /// Exclusive end byte offset of the last consumed token, or the window start
    /// when no token has been consumed yet.
    #[inline]
    pub(crate) fn prev_end_byte(&self) -> usize {
        if self.pos > 0 {
            self.spans[self.pos - 1].end
        } else {
            self.spans.first().map_or(self.eoi_pos, |span| span.start)
        }
    }

    /// Peek at the next token without advancing.
    #[inline]
    pub(crate) fn peek(&self) -> Option<Token<'src>> {
        self.tokens.get(self.pos).copied()
    }

    /// Peek at the token `offset` positions ahead of `pos` without advancing.
    #[cfg(test)]
    #[inline]
    pub(crate) fn peek_at(&self, offset: usize) -> Option<Token<'src>> {
        self.pos
            .checked_add(offset)
            .and_then(|idx| self.tokens.get(idx).copied())
    }

    /// Byte span of the next token, or a zero-width point span at `eoi_pos` on EOF.
    #[inline]
    pub(crate) fn peek_span(&self) -> Span {
        self.spans
            .get(self.pos)
            .copied()
            .unwrap_or_else(|| Span::point(self.eoi_pos))
    }

    /// Peek at the first non-whitespace token starting `offset` tokens ahead of `pos`.
    #[inline]
    pub(crate) fn peek_at_non_ws_after(&self, offset: usize) -> Option<Token<'src>> {
        let mut idx = self.pos.checked_add(offset)?;
        while idx < self.tokens.len() {
            let token = self.tokens[idx];
            if token != Token::Whitespaces {
                return Some(token);
            }
            idx += 1;
        }
        None
    }

    /// Peek at the first non-whitespace token at or after `pos`.
    ///
    /// Returns `(token_index, token, span)` where `token_index` is an absolute
    /// index within this cursor window.
    #[inline]
    pub(crate) fn peek_non_ws(&self) -> Option<(usize, Token<'src>, Span)> {
        let mut idx = self.pos;
        while idx < self.tokens.len() {
            let token = self.tokens[idx];
            if token != Token::Whitespaces {
                return Some((idx, token, self.spans[idx]));
            }
            idx += 1;
        }
        None
    }

    /// Consume and return the next token together with its byte span.
    #[inline]
    pub(crate) fn bump(&mut self) -> Option<(Token<'src>, Span)> {
        let token = *self.tokens.get(self.pos)?;
        let span = self.spans[self.pos];
        self.pos += 1;
        Some((token, span))
    }

    /// Consume the next token if it equals `expected`, returning its byte span.
    #[inline]
    pub(crate) fn eat(&mut self, expected: Token<'_>) -> Option<Span> {
        if self.peek() == Some(expected) {
            let span = self.spans[self.pos];
            self.pos += 1;
            Some(span)
        } else {
            None
        }
    }

    /// Consume the next token if `predicate` returns `true`, returning the
    /// token together with its byte span.
    #[inline]
    pub(crate) fn eat_if(
        &mut self,
        predicate: impl FnOnce(Token<'src>) -> bool,
    ) -> Option<(Token<'src>, Span)> {
        let token = *self.tokens.get(self.pos)?;
        if !predicate(token) {
            return None;
        }
        let span = self.spans[self.pos];
        self.pos += 1;
        Some((token, span))
    }

    /// Consume the next token if `f` maps it to `Some(value)`, returning
    /// `(value, span)`.
    #[inline]
    pub(crate) fn eat_map<T>(
        &mut self,
        f: impl FnOnce(Token<'src>) -> Option<T>,
    ) -> Option<(T, Span)> {
        let token = *self.tokens.get(self.pos)?;
        let value = f(token)?;
        let span = self.spans[self.pos];
        self.pos += 1;
        Some((value, span))
    }

    /// Advance past any consecutive [`Token::Whitespaces`] tokens.
    ///
    /// Returns `true` if at least one whitespace token was skipped.
    #[inline]
    pub(crate) fn skip_ws(&mut self) -> bool {
        let start = self.pos;
        while self.pos < self.tokens.len() && self.tokens[self.pos] == Token::Whitespaces {
            self.pos += 1;
        }
        self.pos > start
    }

    /// Borrow the full token slice of this window.
    #[inline]
    pub(crate) fn tokens_slice(&self) -> &'tok [Token<'src>] {
        self.tokens
    }

    /// Borrow the full span slice of this window.
    #[inline]
    pub(crate) fn spans_slice(&self) -> &'tok [Span] {
        self.spans
    }

    /// Compute the byte span covering `range` of tokens in this window.
    ///
    /// Empty ranges produce a zero-width point span at the start of `range.start`
    /// (or `eoi_pos` when `range.start == self.len()`).
    #[inline]
    pub(crate) fn span_of_range(&self, range: Range<usize>) -> Span {
        debug_assert!(range.start <= range.end && range.end <= self.spans.len());
        if range.start < range.end {
            Span::new(self.spans[range.start].start, self.spans[range.end - 1].end)
        } else {
            let point = self
                .spans
                .get(range.start)
                .map_or(self.eoi_pos, |span| span.start);
            Span::point(point)
        }
    }

    /// Create a sub-cursor borrowing `token_range` directly without copying.
    ///
    /// The sub-window's `eoi_pos` is the end byte of its last token, falling
    /// back to `fallback_eoi` when `token_range` is empty.
    #[inline]
    pub(crate) fn sub_window(
        &self,
        token_range: Range<usize>,
        fallback_eoi: usize,
    ) -> Cursor<'tok, 'src> {
        debug_assert!(token_range.start <= token_range.end && token_range.end <= self.tokens.len());
        let tokens = &self.tokens[token_range.clone()];
        let spans = &self.spans[token_range];
        let eoi_pos = spans.last().map_or(fallback_eoi, |span| span.end);
        Cursor {
            tokens,
            spans,
            pos: 0,
            eoi_pos,
        }
    }

    /// Skip tokens until the matching `}` for an already-consumed `{` (or EOF).
    pub(crate) fn skip_matching_rbrace(&mut self) {
        let mut depth = 1usize;
        while depth > 0 {
            match self.bump() {
                Some((Token::LBrace, _)) => depth = depth.saturating_add(1),
                Some((Token::RBrace, _)) => depth -= 1,
                Some(_) => {}
                None => return,
            }
        }
    }

    /// Scan a balanced `{...}` group starting at `pos`.
    ///
    /// On success, advances `pos` just past the closing `}` and returns the
    /// inner token range and delimiter spans. When `allow_nested` is `false`,
    /// an inner `{` returns [`BalancedScanError::NestedBrace`].
    pub(crate) fn scan_braced(
        &mut self,
        allow_nested: bool,
    ) -> Result<DelimitedRange, BalancedScanError> {
        let Some(open_span) = self.eat(Token::LBrace) else {
            return Err(BalancedScanError::MissingOpen);
        };
        let inner_start = self.pos;
        let mut depth = 0usize;

        while let Some((token, span)) = self.bump() {
            match token {
                Token::LBrace => {
                    if !allow_nested {
                        return Err(BalancedScanError::NestedBrace { open_span });
                    }
                    depth += 1;
                }
                Token::RBrace => {
                    if depth == 0 {
                        return Ok(DelimitedRange {
                            open_span,
                            inner: inner_start..self.pos - 1,
                            close_span: span,
                        });
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }

        Err(BalancedScanError::Unclosed { open_span })
    }

    /// Scan a balanced `[...]` optional-argument group starting at `pos`.
    ///
    /// Tracks `{...}` brace depth so brackets inside braces do not terminate
    /// the scan early. When `match_brackets` is `true`, nested `[...]` pairs at
    /// brace depth 0 are balanced as well.
    pub(crate) fn scan_bracketed(
        &mut self,
        match_brackets: bool,
    ) -> Result<DelimitedRange, BalancedScanError> {
        let Some(open_span) = self.eat(Token::LBracket) else {
            return Err(BalancedScanError::MissingOpen);
        };
        let inner_start = self.pos;
        let mut brace_depth = 0usize;
        let mut bracket_depth = 0usize;

        while let Some((token, span)) = self.bump() {
            match token {
                Token::LBracket => {
                    if match_brackets && brace_depth == 0 {
                        bracket_depth += 1;
                    }
                }
                Token::RBracket => {
                    if brace_depth == 0 {
                        if match_brackets && bracket_depth > 0 {
                            bracket_depth -= 1;
                        } else {
                            return Ok(DelimitedRange {
                                open_span,
                                inner: inner_start..self.pos - 1,
                                close_span: span,
                            });
                        }
                    }
                }
                Token::LBrace => brace_depth += 1,
                Token::RBrace => {
                    if brace_depth == 0 {
                        return Err(BalancedScanError::UnbalancedBrace { open_span });
                    }
                    brace_depth -= 1;
                }
                _ => {}
            }
        }

        Err(BalancedScanError::Unclosed { open_span })
    }

    /// Scan one or more consecutive `Char` / `Star` tokens starting at
    /// `start_idx` as an environment name.
    ///
    /// Borrows `name` directly from `src` when the tokens are contiguous;
    /// allocates only when a comment split the name in the source.
    fn scan_env_name_at(
        &self,
        src: &'src str,
        start_idx: usize,
    ) -> Option<(usize, Cow<'src, str>)> {
        let mut idx = start_idx;
        let mut first_byte = 0usize;
        let mut prev_end = 0usize;
        let mut contiguous = true;

        while let Some(token) = self.tokens.get(idx) {
            match token {
                Token::Char(_) | Token::Star => {
                    let span = self.spans[idx];
                    if idx == start_idx {
                        first_byte = span.start;
                    } else if span.start != prev_end {
                        contiguous = false;
                    }
                    prev_end = span.end;
                    idx += 1;
                }
                _ => break,
            }
        }

        if idx == start_idx {
            return None;
        }

        let env_name = if contiguous {
            Cow::Borrowed(&src[first_byte..prev_end])
        } else {
            let mut owned = String::with_capacity(idx - start_idx);
            for tok in &self.tokens[start_idx..idx] {
                match tok {
                    Token::Char(c) => owned.push(*c),
                    Token::Star => owned.push('*'),
                    _ => unreachable!("env name loop only accepts Char and Star"),
                }
            }
            Cow::Owned(owned)
        };

        Some((idx, env_name))
    }

    /// Consume one or more consecutive `Char` / `Star` tokens at the current
    /// position as an environment name, returning `None` without advancing if
    /// no valid name token is present.
    pub(crate) fn eat_env_name(&mut self, src: &'src str) -> Option<Cow<'src, str>> {
        let (end_idx, name) = self.scan_env_name_at(src, self.pos)?;
        self.pos = end_idx;
        Some(name)
    }

    /// Inspect a `\begin{name}` or `\end{name}` marker at the current position
    /// without advancing the cursor.
    ///
    /// Returns `(end_pos, env_name, marker_span)` where `end_pos` is the token
    /// index immediately after the closing `}`. Borrows `env_name` directly
    /// from `src` when the name tokens are contiguous; allocates only if a
    /// comment split the name in the source.
    pub(crate) fn peek_env_marker(
        &self,
        src: &'src str,
        head: &str,
    ) -> Option<(usize, Cow<'src, str>, Span)> {
        let mut idx = self.pos;
        let Token::ControlSeq(name) = *self.tokens.get(idx)? else {
            return None;
        };
        if name != head {
            return None;
        }
        let head_start = self.spans[idx].start;
        idx += 1;

        while self.tokens.get(idx) == Some(&Token::Whitespaces) {
            idx += 1;
        }
        if self.tokens.get(idx) != Some(&Token::LBrace) {
            return None;
        }
        idx += 1;

        let (end_name_idx, env_name) = self.scan_env_name_at(src, idx)?;
        if self.tokens.get(end_name_idx) != Some(&Token::RBrace) {
            return None;
        }
        let close_end = self.spans[end_name_idx].end;
        Some((end_name_idx + 1, env_name, Span::new(head_start, close_end)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex_source;

    #[test]
    fn cursor_peek_skip_ws_and_byte_positions() {
        let src = "  a + b ";
        let stream = lex_source(src).unwrap();
        let mut cursor = Cursor::from_stream(&stream, src.len());

        assert!(!cursor.is_at_end());
        assert_eq!(cursor.len(), 7);
        assert_eq!(cursor.pos(), 0);
        assert_eq!(cursor.byte_pos(), 0);
        assert_eq!(cursor.prev_end_byte(), 0);
        assert_eq!(cursor.peek(), Some(Token::Whitespaces));
        assert_eq!(cursor.peek_at(1), Some(Token::Char('a')));
        assert_eq!(
            cursor.peek_non_ws(),
            Some((1, Token::Char('a'), Span::new(2, 3)))
        );

        assert!(cursor.skip_ws());
        assert!(!cursor.skip_ws());
        assert_eq!(cursor.pos(), 1);
        assert_eq!(cursor.byte_pos(), 2);
        assert_eq!(cursor.prev_end_byte(), 2);

        let (tok, span) = cursor.bump().unwrap();
        assert_eq!(tok, Token::Char('a'));
        assert_eq!(span, Span::new(2, 3));
        assert_eq!(cursor.prev_end_byte(), 3);
        assert_eq!(cursor.span_of_range(1..4), Span::new(2, 5));
        assert_eq!(cursor.span_of_range(1..1), Span::point(2));

        cursor.set_pos(cursor.len());
        assert!(cursor.is_at_end());
        assert_eq!(cursor.byte_pos(), src.len());
        assert_eq!(cursor.peek_span(), Span::point(src.len()));
        assert_eq!(
            cursor.span_of_range(cursor.len()..cursor.len()),
            Span::point(src.len())
        );
        assert_eq!(cursor.peek_non_ws(), None);
    }

    #[test]
    fn sub_window_preserves_original_spans_and_fallback_eoi() {
        let src = r"\frac{a+b}{}";
        let stream = lex_source(src).unwrap();
        let mut cursor = Cursor::from_stream(&stream, src.len());
        cursor.bump(); // \frac

        let first = cursor.scan_braced(true).unwrap();
        assert_eq!(first.full_span(), Span::new(5, 10));
        let mut sub = cursor.sub_window(first.inner.clone(), first.open_span.end);
        assert_eq!(sub.len(), 3);
        assert_eq!(sub.eoi_pos(), 9);
        assert_eq!(sub.bump(), Some((Token::Char('a'), Span::new(6, 7))));
        assert_eq!(sub.bump(), Some((Token::Char('+'), Span::new(7, 8))));
        assert_eq!(sub.bump(), Some((Token::Char('b'), Span::new(8, 9))));
        assert_eq!(sub.peek_span(), Span::point(9));

        let second = cursor.scan_braced(true).unwrap();
        assert!(second.inner.is_empty());
        let empty_sub = cursor.sub_window(second.inner.clone(), second.open_span.end);
        assert!(empty_sub.is_at_end());
        assert_eq!(empty_sub.eoi_pos(), 11);
        assert_eq!(empty_sub.peek_span(), Span::point(11));
    }

    #[test]
    fn scan_braced_and_bracketed_handle_nesting_and_errors() {
        let src = "[a{]}b][c[d]e]";
        let stream = lex_source(src).unwrap();
        let mut cursor = Cursor::from_stream(&stream, src.len());

        let first = cursor.scan_bracketed(false).unwrap();
        assert_eq!(
            &src[first.full_span().start..first.full_span().end],
            "[a{]}b]"
        );

        let second = cursor.scan_bracketed(true).unwrap();
        assert_eq!(
            &src[second.full_span().start..second.full_span().end],
            "[c[d]e]"
        );

        let nested_src = "{a{b}c}";
        let nested_stream = lex_source(nested_src).unwrap();
        let mut c1 = Cursor::from_stream(&nested_stream, nested_src.len());
        assert_eq!(
            c1.scan_braced(false),
            Err(BalancedScanError::NestedBrace {
                open_span: Span::new(0, 1)
            })
        );

        let unclosed_src = "[a}";
        let unclosed_stream = lex_source(unclosed_src).unwrap();
        let mut c2 = Cursor::from_stream(&unclosed_stream, unclosed_src.len());
        assert_eq!(
            c2.scan_bracketed(false),
            Err(BalancedScanError::UnbalancedBrace {
                open_span: Span::new(0, 1)
            })
        );
    }

    #[test]
    fn peek_env_marker_handles_contiguous_and_comment_split_names() {
        let src = r"\begin {align*}x\end{mat%c
rix}";
        let stream = lex_source(src).unwrap();
        let mut cursor = Cursor::from_stream(&stream, src.len());

        let (end_pos, name, span) = cursor.peek_env_marker(src, "begin").unwrap();
        assert!(matches!(name, Cow::Borrowed("align*")));
        assert_eq!(&src[span.start..span.end], r"\begin {align*}");
        assert_eq!(cursor.pos(), 0); // peek does not advance

        cursor.set_pos(end_pos);
        assert_eq!(cursor.bump().unwrap().0, Token::Char('x'));

        let (_, split_name, end_span) = cursor.peek_env_marker(src, "end").unwrap();
        assert!(matches!(split_name, Cow::Owned(_)));
        assert_eq!(split_name.as_ref(), "matrix");
        assert_eq!(&src[end_span.start..end_span.end], "\\end{mat%c\nrix}");
    }
}
