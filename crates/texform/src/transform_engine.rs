//! Normalization entry point.
//!
//! A [`TransformEngine`] pairs a parser with a transform pipeline configured for
//! one [`Profile`]. It offers a string-to-string
//! [`normalize`](TransformEngine::normalize) path and an in-place
//! [`transform`](TransformEngine::transform) path over a live
//! [`Document`]. In-place transformation accepts complete documents sharing
//! the engine's immutable knowledge-base instance.

use texform_core::parse::ParseConfig;
use texform_transform::{BuildConfig, Profile, TransformContext};

use crate::config::{NormalizeConfig, TransformConfig};
use crate::diagnostics::{NormalizeReportResult, TransformReport};
use crate::document::Document;
use crate::error::Error;
use crate::parser::{Parser, ParserBuilder};

/// Parser plus transform pipeline for one normalization profile.
///
/// Documents passed to transformation must share the engine's knowledge-base
/// instance. Independently built knowledge bases are distinct even when their
/// contents match; cloning a knowledge base preserves its identity.
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

impl TransformEngine {
    /// Start building a transform engine.
    pub fn builder() -> TransformEngineBuilder {
        TransformEngineBuilder {
            parser: Parser::builder().default_parse_config(ParseConfig::LENIENT),
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

    /// The immutable knowledge base shared by this engine and its parser.
    pub fn knowledge_base(&self) -> &crate::KnowledgeBase {
        self.parser.knowledge_base()
    }

    /// Normalize a parsed document in place with the default transform config.
    ///
    /// A different knowledge-base instance returns [`Error::KnowledgeBaseMismatch`].
    /// A document containing parse errors returns [`Error::IncompleteTree`].
    pub fn transform(&self, document: &mut Document) -> Result<(), Error> {
        self.transform_with(document, self.transform.default_config())
    }

    /// Normalize a parsed document in place with an explicit transform config.
    ///
    /// The document must share this engine's knowledge-base instance.
    /// The call does not collect a report.
    pub fn transform_with(
        &self,
        document: &mut Document,
        config: &TransformConfig,
    ) -> Result<(), Error> {
        self.ensure_engine_document(document)?;
        self.transform.run_with(
            document.core_mut().__texform_engine_ast_mut(),
            self.parser.inner(),
            config,
        )?;
        Ok(())
    }

    /// Normalize a parsed document in place and return this call's diagnostic report.
    ///
    /// The config is the same [`TransformConfig`] accepted by
    /// [`transform_with`](Self::transform_with). There is no default-config
    /// overload; pass [`default_transform_config`](Self::default_transform_config)
    /// when the engine defaults should apply. Knowledge-base identity, completeness,
    /// and transform errors match [`transform`](Self::transform). A failure
    /// does not return a partial report.
    pub fn transform_with_report(
        &self,
        document: &mut Document,
        config: &TransformConfig,
    ) -> Result<TransformReport, Error> {
        self.ensure_engine_document(document)?;
        Ok(self.transform.run_with_report(
            document.core_mut().__texform_engine_ast_mut(),
            self.parser.inner(),
            config,
        )?)
    }

    /// Unstable research entry: apply a sparse FlattenGroups guard overlay.
    ///
    /// This method is not part of the stable facade. It always collects a
    /// report, does not change engine defaults, cannot re-enable a disabled
    /// FlattenGroups phase, and still enforces knowledge-base identity,
    /// completeness, and slot/mode/contract checks.
    #[doc(hidden)]
    pub fn transform_with_flatten_groups_guards(
        &self,
        document: &mut Document,
        config: &TransformConfig,
        overlay: &texform_transform::FlattenGroupsGuardsOverlay,
    ) -> Result<TransformReport, Error> {
        self.ensure_engine_document(document)?;
        Ok(self.transform.run_with_flatten_groups_guards(
            document.core_mut().__texform_engine_ast_mut(),
            self.parser.inner(),
            config,
            overlay,
        )?)
    }

    /// Parse, transform, and serialize a LaTeX formula.
    ///
    /// This is the string-to-string convenience path. Use
    /// [`parser`](Self::parser) plus [`transform`](Self::transform) when you
    /// need to keep editing the live [`Document`] before serialization.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`] if the source does not parse into a complete
    /// tree, [`Error::Transform`] if a rule fails, or [`Error::Serialize`] if
    /// the normalized tree cannot be serialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use texform::{Profile, TransformEngine};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let engine = TransformEngine::builder().profile(Profile::Corpus).build()?;
    /// assert_eq!(engine.normalize(r"a \over b")?, r"\frac { a } { b }");
    /// # Ok(())
    /// # }
    /// ```
    pub fn normalize(&self, src: &str) -> Result<String, Error> {
        self.normalize_with(src, &self.default_normalize_config())
    }

    /// Parse, transform, and serialize a LaTeX formula with explicit configs.
    ///
    /// # Errors
    ///
    /// Same failure modes as [`normalize`](Self::normalize), but using the
    /// supplied [`NormalizeConfig`] for both parsing and
    /// transformation. The call does not collect a report.
    pub fn normalize_with(&self, src: &str, config: &NormalizeConfig) -> Result<String, Error> {
        let mut document = self.parse_complete(src, config)?;
        self.transform_with(&mut document, &config.transform)?;
        Ok(document.to_latex()?)
    }

    /// Parse, transform, and serialize a formula, returning text and a diagnostic report.
    ///
    /// The config is the same [`NormalizeConfig`] accepted by
    /// [`normalize_with`](Self::normalize_with). There is no default-config
    /// overload; pass [`default_normalize_config`](Self::default_normalize_config)
    /// when the engine defaults should apply. Output text and errors match
    /// [`normalize`](Self::normalize). A failure does not return a partial report.
    pub fn normalize_with_report(
        &self,
        src: &str,
        config: &NormalizeConfig,
    ) -> Result<NormalizeReportResult, Error> {
        let mut document = self.parse_complete(src, config)?;
        let report = self.transform_with_report(&mut document, &config.transform)?;
        Ok(NormalizeReportResult {
            normalized: document.to_latex()?,
            report,
        })
    }

    /// Unstable research entry for string-to-string FlattenGroups guard overlays.
    ///
    /// This method is not part of the stable facade, always collects a report,
    /// and may change without notice.
    #[doc(hidden)]
    pub fn normalize_with_flatten_groups_guards(
        &self,
        src: &str,
        config: &NormalizeConfig,
        overlay: &texform_transform::FlattenGroupsGuardsOverlay,
    ) -> Result<NormalizeReportResult, Error> {
        let mut document = self.parse_complete(src, config)?;
        let report =
            self.transform_with_flatten_groups_guards(&mut document, &config.transform, overlay)?;
        Ok(NormalizeReportResult {
            normalized: document.to_latex()?,
            report,
        })
    }

    fn ensure_engine_document(&self, document: &Document) -> Result<(), Error> {
        if !document.knowledge_base().ptr_eq(self.knowledge_base()) {
            return Err(Error::KnowledgeBaseMismatch);
        }
        if document.has_errors() {
            return Err(Error::IncompleteTree);
        }
        Ok(())
    }

    fn parse_complete(&self, src: &str, config: &NormalizeConfig) -> Result<Document, Error> {
        let (document, _) = self
            .parser
            .parse_with(src, &config.parse)
            .try_into_document()?;
        Ok(document)
    }

    /// Default transform configuration used by [`transform`](Self::transform).
    pub fn default_transform_config(&self) -> &TransformConfig {
        self.transform.default_config()
    }

    /// Combined parse and transform defaults used by [`normalize`](Self::normalize).
    ///
    /// Parse defaults come from this engine's parser; transform defaults come
    /// from the selected profile. Language bindings should take the normalize
    /// baseline from this method rather than reassembling the two halves.
    pub fn default_normalize_config(&self) -> NormalizeConfig {
        NormalizeConfig {
            parse: self.parser.default_parse_config().clone(),
            transform: *self.transform.default_config(),
        }
    }
}

impl TransformEngineBuilder {
    /// Share an existing immutable knowledge-base instance with the engine.
    pub fn knowledge_base(mut self, knowledge_base: crate::KnowledgeBase) -> Self {
        self.parser = self.parser.knowledge_base(knowledge_base);
        self
    }

    /// Set the default [`ParseConfig`] for the engine's parser.
    ///
    /// The engine defaults to [`ParseConfig::LENIENT`].
    pub fn default_parse_config(mut self, config: ParseConfig) -> Self {
        self.parser = self.parser.default_parse_config(config);
        self
    }

    /// Select the normalization [`Profile`].
    ///
    /// A profile is required: [`build`](Self::build) fails with
    /// [`Error::MissingProfile`] if none is set. Setting a profile also resets
    /// the build configuration to that profile's defaults.
    pub fn profile(mut self, profile: Profile) -> Self {
        self.profile = Some(profile);
        self.build_config = Some(BuildConfig::profile(profile));
        self
    }

    /// Disable a specific transform rule by [`RuleKey`](crate::RuleKey).
    ///
    /// Disabled rules accumulate across calls.
    pub fn disable_rule(mut self, key: crate::RuleKey) -> Self {
        self.disabled_rules.push(key);
        self
    }

    /// Disable a specific transform rule by its stable string name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownRule`] if no rule carries the given name.
    pub fn disable_rule_by_name(self, name: impl AsRef<str>) -> Result<Self, Error> {
        let name = name.as_ref();
        let key =
            crate::rule_key_from_name(name).ok_or_else(|| Error::UnknownRule(name.to_owned()))?;
        Ok(self.disable_rule(key))
    }

    /// Build the [`TransformEngine`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingProfile`] if no profile was selected,
    /// or [`Error::TransformBuild`] if the transform plan cannot be built.
    pub fn build(self) -> Result<TransformEngine, Error> {
        let mut build_config = self
            .build_config
            .or_else(|| self.profile.map(BuildConfig::profile))
            .ok_or(Error::MissingProfile)?;
        for key in self.disabled_rules {
            build_config = build_config.disable_rule(key);
        }
        let parser = self.parser.build();
        let transform = TransformContext::from_build_config(build_config, parser.inner())?;
        Ok(TransformEngine { parser, transform })
    }
}
