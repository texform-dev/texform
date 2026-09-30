//! Parse-only entry point using a shared immutable knowledge base.
//!
//! A [`Parser`] pairs a [`KnowledgeBase`] with a default [`ParseConfig`] and
//! turns LaTeX source into an editable [`Document`](crate::Document) without
//! normalizing it. To normalize as well, use
//! [`TransformEngine`](crate::TransformEngine), which owns a parser internally.

use texform_core::parse::{self, ParseConfig};

use crate::{KnowledgeBase, parse_result::ParseResult};

/// A configured LaTeX parser sharing an immutable knowledge base.
///
/// Cloning is cheap; parsed documents share the parser's knowledge base.
#[derive(Clone, Debug)]
pub struct Parser {
    inner: KnowledgeBase,
    default_config: ParseConfig,
}

/// Builder selecting a knowledge base and default parse configuration.
pub struct ParserBuilder {
    inner: KnowledgeBase,
    default_config: ParseConfig,
}

impl Parser {
    /// Start building a parser with the shared default knowledge base.
    ///
    /// Build a custom [`KnowledgeBase`] once and share it with every parser,
    /// engine, and document that must interoperate:
    ///
    /// ```
    /// use texform::{KnowledgeBase, Parser, Profile, TransformEngine};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let kb = KnowledgeBase::builder().packages(&["base", "ams"]).build()?;
    /// let parser = Parser::builder().knowledge_base(kb.clone()).build();
    /// let engine = TransformEngine::builder()
    ///     .knowledge_base(kb)
    ///     .profile(Profile::Corpus)
    ///     .build()?;
    ///
    /// let (mut document, _) = parser.parse(r"a \over b").try_into_document()?;
    /// engine.transform(&mut document)?;
    /// assert_eq!(document.to_latex()?, r"\frac { a } { b }");
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> ParserBuilder {
        ParserBuilder {
            inner: KnowledgeBase::default(),
            default_config: ParseConfig::default(),
        }
    }

    /// Parse a formula with this parser's default configuration.
    pub fn parse(&self, src: &str) -> ParseResult {
        ParseResult::from_core(self.inner.parse(src, &self.default_config))
    }

    /// Parse a formula with an explicit configuration.
    ///
    /// The returned document shares this parser's knowledge base.
    pub fn parse_with(&self, src: &str, config: &ParseConfig) -> ParseResult {
        ParseResult::from_core(self.inner.parse(src, config))
    }

    /// The immutable knowledge base shared by this parser and its documents.
    pub fn knowledge_base(&self) -> &KnowledgeBase {
        &self.inner
    }

    pub(crate) fn inner(&self) -> &parse::ParseContext {
        &self.inner
    }

    /// The default configuration used by [`parse`](Self::parse).
    pub fn default_parse_config(&self) -> &ParseConfig {
        &self.default_config
    }
}

impl ParserBuilder {
    /// Use an existing knowledge-base instance.
    pub fn knowledge_base(mut self, knowledge_base: KnowledgeBase) -> Self {
        self.inner = knowledge_base;
        self
    }

    /// Set the default configuration used by [`Parser::parse`].
    pub fn default_parse_config(mut self, config: ParseConfig) -> Self {
        self.default_config = config;
        self
    }

    /// Build the parser; knowledge-base errors surface earlier from
    /// [`KnowledgeBaseBuilder::build`](crate::KnowledgeBaseBuilder::build).
    pub fn build(self) -> Parser {
        Parser {
            inner: self.inner,
            default_config: self.default_config,
        }
    }
}
