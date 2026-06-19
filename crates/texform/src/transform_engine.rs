use texform_core::parse::ParseConfig;
use texform_transform::{BuildConfig, Profile, TransformContext, TransformReport};

use crate::config::{NormalizeConfig, TransformConfig};
use crate::document::Document;
use crate::error::Error;
use crate::parser::{Parser, ParserBuilder};

/// Parser plus transform pipeline for one normalization profile.
///
/// A `TransformEngine` owns the parser context used to build its transform
/// plan. Documents passed to [`transform`](Self::transform) or
/// [`transform_with`](Self::transform_with) must come from this engine's
/// [`parser`](Self::parser) and extracted from its parse result; documents
/// parsed by another parser, built with [`Document::new`], or rebuilt with
/// [`Document::from_syntax`] are rejected with [`Error::ForeignDocument`].
pub struct TransformEngine {
    parser: Parser,
    transform: TransformContext,
}

/// Builder for [`TransformEngine`].
pub struct TransformEngineBuilder {
    parser: ParserBuilder,
    profile: Option<Profile>,
    build_config: Option<BuildConfig>,
    disabled_rules: Vec<crate::RuleKey>,
}

/// Result returned by [`TransformEngine::normalize`] and
/// [`TransformEngine::normalize_with`].
pub struct NormalizeResult {
    /// Serialized LaTeX after parsing and normalization.
    pub normalized: String,
    /// Report describing which transform phases and rules changed the tree.
    pub report: TransformReport,
}

impl TransformEngine {
    /// Start building a transform engine.
    pub fn builder() -> TransformEngineBuilder {
        TransformEngineBuilder {
            parser: Parser::builder().default_parse_config(ParseConfig::STRICT),
            profile: None,
            build_config: None,
            disabled_rules: Vec::new(),
        }
    }

    /// Parser owned by this engine.
    ///
    /// Parse with this parser when you intend to keep editing the live
    /// [`Document`] and then call [`transform`](Self::transform) on it.
    pub fn parser(&self) -> &Parser {
        &self.parser
    }

    /// Normalize a parsed document in place with the default transform config.
    ///
    /// The document must have been extracted from a parse result produced by
    /// this engine's [`parser`](Self::parser). Passing a document from another
    /// parser, from [`Document::new`], or from [`Document::from_syntax`]
    /// returns [`Error::ForeignDocument`]. Passing a parsed document that
    /// contains parse errors returns [`Error::IncompleteTree`].
    pub fn transform(&self, document: &mut Document) -> Result<TransformReport, Error> {
        self.transform_with(document, self.transform.default_config())
    }

    /// Normalize a parsed document in place with an explicit transform config.
    ///
    /// This has the same document-source requirement as
    /// [`transform`](Self::transform): the document must come from this
    /// engine's [`parser`](Self::parser), otherwise the method returns
    /// [`Error::ForeignDocument`].
    pub fn transform_with(
        &self,
        document: &mut Document,
        config: &TransformConfig,
    ) -> Result<TransformReport, Error> {
        if document.parse_context_id() != Some(self.parser.inner().id()) {
            return Err(Error::ForeignDocument);
        }
        if document.has_errors() {
            return Err(Error::IncompleteTree);
        }

        Ok(self.transform.run_with(
            document.core_mut().__texform_engine_ast_mut(),
            self.parser.inner(),
            config,
        )?)
    }

    /// Parse, transform, and serialize a LaTeX formula.
    ///
    /// This is the string-to-string convenience path. Use
    /// [`parser`](Self::parser) plus [`transform`](Self::transform) when you
    /// need to keep editing the live [`Document`] before serialization.
    pub fn normalize(&self, src: &str) -> Result<NormalizeResult, Error> {
        let config = NormalizeConfig {
            parse: self.parser.default_parse_config().clone(),
            transform: *self.transform.default_config(),
        };
        self.normalize_with(src, &config)
    }

    /// Parse, transform, and serialize a LaTeX formula with explicit configs.
    pub fn normalize_with(
        &self,
        src: &str,
        config: &NormalizeConfig,
    ) -> Result<NormalizeResult, Error> {
        let (mut document, _) = self
            .parser
            .parse_with(src, &config.parse)
            .try_into_document()?;
        let report = self.transform_with(&mut document, &config.transform)?;
        Ok(NormalizeResult {
            normalized: document.to_latex()?,
            report,
        })
    }

    /// Default transform configuration used by [`transform`](Self::transform).
    pub fn default_transform_config(&self) -> &TransformConfig {
        self.transform.default_config()
    }
}

impl TransformEngineBuilder {
    pub fn packages(mut self, packages: &[&str]) -> Self {
        self.parser = self.parser.packages(packages);
        self
    }

    pub fn empty_knowledge(mut self) -> Self {
        self.parser = self.parser.empty_knowledge();
        self
    }

    pub fn default_parse_config(mut self, config: ParseConfig) -> Self {
        self.parser = self.parser.default_parse_config(config);
        self
    }

    pub fn item(mut self, item: impl Into<texform_core::parse::ContextItem>) -> Self {
        self.parser = self.parser.item(item);
        self
    }

    pub fn remove_command(mut self, name: impl Into<String>) -> Self {
        self.parser = self.parser.remove_command(name);
        self
    }

    pub fn remove_environment(mut self, name: impl Into<String>) -> Self {
        self.parser = self.parser.remove_environment(name);
        self
    }

    pub fn remove_delimiter_control(mut self, name: impl Into<String>) -> Self {
        self.parser = self.parser.remove_delimiter_control(name);
        self
    }

    pub fn profile(mut self, profile: Profile) -> Self {
        self.profile = Some(profile);
        self.build_config = Some(BuildConfig::profile(profile));
        self
    }

    pub fn disable_rule(mut self, key: crate::RuleKey) -> Self {
        self.disabled_rules.push(key);
        self
    }

    pub fn disable_rule_by_name(self, name: impl AsRef<str>) -> Result<Self, Error> {
        let name = name.as_ref();
        let key =
            crate::rule_key_from_name(name).ok_or_else(|| Error::UnknownRule(name.to_owned()))?;
        Ok(self.disable_rule(key))
    }

    pub fn build(self) -> Result<TransformEngine, Error> {
        let mut build_config = self
            .build_config
            .or_else(|| self.profile.map(BuildConfig::profile))
            .ok_or(Error::MissingProfile)?;
        for key in self.disabled_rules {
            build_config = build_config.disable_rule(key);
        }
        let parser = self.parser.build().map_err(Error::ParserBuild)?;
        let transform = TransformContext::from_build_config(build_config, parser.inner())?;
        Ok(TransformEngine { parser, transform })
    }
}
