use texform_core::parse::{self, ContextItem, ParseConfig, ParseOutput};

#[derive(Clone, Debug)]
pub struct Parser {
    inner: parse::ParseContext,
}

pub struct ParserBuilder {
    inner: parse::ParseContextBuilder,
}

#[derive(Debug)]
pub struct ParserBuildError(parse::ParseContextBuildError);

impl std::fmt::Display for ParserBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            parse::ParseContextBuildError::PackageLoad(error) => error.fmt(f),
            parse::ParseContextBuildError::InvalidContextItem { name, source } => {
                write!(f, "invalid context item {name}: {source}")
            }
        }
    }
}

impl std::error::Error for ParserBuildError {}

impl Parser {
    pub fn builder() -> ParserBuilder {
        ParserBuilder {
            inner: parse::ParseContext::builder(),
        }
    }

    /// Parse a LaTeX formula with the facade default configuration.
    ///
    /// The facade default is [`ParseConfig::default`], currently
    /// [`ParseConfig::NONSTRICT_RECOVER`]. Use [`parse_with`](Self::parse_with)
    /// when a caller needs strict parsing or recovery disabled.
    pub fn parse(&self, src: &str) -> ParseOutput {
        self.inner.parse(src, &ParseConfig::default())
    }

    pub fn parse_with(&self, src: &str, config: &ParseConfig) -> ParseOutput {
        self.inner.parse(src, config)
    }

    pub fn parse_to_ast(
        &self,
        src: &str,
        config: &ParseConfig,
    ) -> Result<texform_core::ast::Ast, parse::ParseAstError> {
        self.inner.parse_to_ast(src, config)
    }

    pub fn lookup_command(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveCommandRecord> {
        self.inner.lookup_command(name, mode)
    }

    pub fn lookup_explicit_command(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveCommandRecord> {
        self.inner.lookup_explicit_command(name, mode)
    }

    pub fn lookup_character(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveCharacterRecord> {
        self.inner.lookup_character(name, mode)
    }

    pub fn lookup_env(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveEnvironmentRecord> {
        self.inner.lookup_env(name, mode)
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    pub fn knows_command_name(&self, name: &str) -> bool {
        self.inner.knows_command_name(name)
    }

    pub fn knows_env_name(&self, name: &str) -> bool {
        self.inner.knows_env_name(name)
    }

    pub fn knows_character_name(&self, name: &str) -> bool {
        self.inner.knows_character_name(name)
    }

    pub(crate) fn inner(&self) -> &parse::ParseContext {
        &self.inner
    }
}

impl ParserBuilder {
    pub fn packages(mut self, packages: &[&str]) -> Self {
        self.inner = self.inner.packages(packages);
        self
    }

    pub fn empty_knowledge(mut self) -> Self {
        self.inner = parse::ParseContextBuilder::empty();
        self
    }

    pub fn item(mut self, item: impl Into<ContextItem>) -> Self {
        self.inner = self.inner.insert_item(item);
        self
    }

    pub fn insert_item(self, item: impl Into<ContextItem>) -> Self {
        self.item(item)
    }

    pub fn remove_command(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.remove_command(name);
        self
    }

    pub fn remove_environment(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.remove_environment(name);
        self
    }

    pub fn remove_delimiter_control(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.remove_delimiter_control(name);
        self
    }

    pub fn build(self) -> Result<Parser, ParserBuildError> {
        Ok(Parser {
            inner: self.inner.build().map_err(ParserBuildError)?,
        })
    }
}
