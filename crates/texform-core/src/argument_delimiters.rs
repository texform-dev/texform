//! Token boundaries shared by argument collection and protective serialization.

use crate::lexer::Token;

pub(crate) struct ContentBoundary {
    open: Option<Token>,
    close: Token,
    braces: usize,
    nested: usize,
}

impl ContentBoundary {
    pub(crate) fn new(open: Option<Token>, close: Token) -> Self {
        Self {
            open,
            close,
            braces: 0,
            nested: 0,
        }
    }

    /// Returns true only for a token that would end the surrounding argument.
    pub(crate) fn ends_at(&mut self, token: &Token) -> bool {
        if self.open != Some(Token::LBrace) {
            match token {
                Token::LBrace => {
                    self.braces += 1;
                    return false;
                }
                Token::RBrace if self.braces > 0 => {
                    self.braces -= 1;
                    return false;
                }
                _ => {}
            }
            if self.braces > 0 {
                return false;
            }
        }
        if *token == self.close {
            if self.nested == 0 {
                return true;
            }
            self.nested -= 1;
        } else if self.open.as_ref() == Some(token) {
            self.nested += 1;
        }
        false
    }

    pub(crate) fn needs_protection(mut self, tokens: &[Token]) -> bool {
        tokens.iter().any(|token| self.ends_at(token))
    }
}

/// Identify exactly one enclosing brace group, ignoring surrounding whitespace.
pub(crate) fn enclosing_braces(tokens: &[Token]) -> Option<std::ops::Range<usize>> {
    let start = tokens.iter().position(|t| *t != Token::Whitespaces)?;
    let end = tokens.iter().rposition(|t| *t != Token::Whitespaces)?;
    if tokens[start] != Token::LBrace || tokens[end] != Token::RBrace {
        return None;
    }
    let mut depth = 0usize;
    for (offset, token) in tokens[start..=end].iter().enumerate() {
        match token {
            Token::LBrace => depth += 1,
            Token::RBrace => {
                depth = depth.checked_sub(1)?;
            }
            _ => {}
        }
        if depth == 0 && start + offset != end {
            return None;
        }
    }
    (depth == 0).then_some(start + 1..end)
}
