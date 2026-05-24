use texform_core::ast::Ast;
use texform_core::parse::ParseConfig;
use texform_transform::{BuildConfig, Profile, TransformContext, TransformReport};

use crate::config::{NormalizeConfig, TransformConfig};
use crate::error::Error;
use crate::parser::{Parser, ParserBuilder};

pub struct Engine {
    parser: Parser,
    transform: TransformContext,
}

pub struct EngineBuilder {
    parser: ParserBuilder,
    profile: Option<Profile>,
    build_config: Option<BuildConfig>,
}

pub struct NormalizeResult {
    pub normalized: String,
    pub report: TransformReport,
}

impl Engine {
    pub fn builder() -> EngineBuilder {
        EngineBuilder {
            parser: Parser::builder().default_parse_config(ParseConfig::STRICT),
            profile: None,
            build_config: None,
        }
    }

    pub fn parser(&self) -> &Parser {
        &self.parser
    }

    pub fn transform_ast(&self, ast: &mut Ast) -> Result<TransformReport, Error> {
        Ok(self.transform.run(ast, self.parser.inner())?)
    }

    pub fn transform_ast_with(
        &self,
        ast: &mut Ast,
        config: &TransformConfig,
    ) -> Result<TransformReport, Error> {
        Ok(self.transform.run_with(ast, self.parser.inner(), config)?)
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
        let mut ast = self.parser.parse_to_ast_with(src, &config.parse)?;
        let report = self
            .transform
            .run_with(&mut ast, self.parser.inner(), &config.transform)?;
        Ok(NormalizeResult {
            normalized: crate::serialize::serialize(&ast)?,
            report,
        })
    }

    pub fn default_transform_config(&self) -> &TransformConfig {
        self.transform.default_config()
    }
}

impl EngineBuilder {
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
        let config = self
            .build_config
            .take()
            .or_else(|| self.profile.map(BuildConfig::profile))
            .expect("profile must be set before disabling rules");
        self.build_config = Some(config.disable_rule(key));
        self
    }

    pub fn disable_rule_by_name(self, name: impl AsRef<str>) -> Result<Self, Error> {
        let name = name.as_ref();
        let key =
            crate::rule_key_from_name(name).ok_or_else(|| Error::UnknownRule(name.to_owned()))?;
        Ok(self.disable_rule(key))
    }

    pub fn build(self) -> Result<Engine, Error> {
        let build_config = self
            .build_config
            .or_else(|| self.profile.map(BuildConfig::profile))
            .ok_or(Error::MissingProfile)?;
        let parser = self.parser.build().map_err(Error::ParserBuild)?;
        let transform = TransformContext::from_build_config(build_config, parser.inner())
            .map_err(Error::TransformBuild)?;
        Ok(Engine { parser, transform })
    }
}
