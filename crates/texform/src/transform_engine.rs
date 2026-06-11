use texform_core::parse::ParseConfig;
use texform_transform::{BuildConfig, Profile, TransformContext, TransformReport};

use crate::config::{NormalizeConfig, TransformConfig};
use crate::document::Document;
use crate::error::Error;
use crate::parser::{Parser, ParserBuilder};

pub struct TransformEngine {
    parser: Parser,
    transform: TransformContext,
}

pub struct TransformEngineBuilder {
    parser: ParserBuilder,
    profile: Option<Profile>,
    build_config: Option<BuildConfig>,
    disabled_rules: Vec<crate::RuleKey>,
}

pub struct NormalizeResult {
    pub normalized: String,
    pub report: TransformReport,
}

impl TransformEngine {
    pub fn builder() -> TransformEngineBuilder {
        TransformEngineBuilder {
            parser: Parser::builder().default_parse_config(ParseConfig::STRICT),
            profile: None,
            build_config: None,
            disabled_rules: Vec::new(),
        }
    }

    pub fn parser(&self) -> &Parser {
        &self.parser
    }

    pub fn transform(&self, document: &mut Document) -> Result<TransformReport, Error> {
        self.transform_with(document, self.transform.default_config())
    }

    pub fn transform_with(
        &self,
        document: &mut Document,
        config: &TransformConfig,
    ) -> Result<TransformReport, Error> {
        if document.has_errors() {
            return Err(Error::IncompleteTree);
        }

        Ok(self.transform.run_with(
            document.core_mut().__texform_engine_ast_mut(),
            self.parser.inner(),
            config,
        )?)
    }

    pub fn normalize(&self, src: &str) -> Result<NormalizeResult, Error> {
        let config = NormalizeConfig {
            parse: self.parser.default_parse_config().clone(),
            transform: *self.transform.default_config(),
        };
        self.normalize_with(src, &config)
    }

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
