//! Parse-only entry point.
//!
//! A [`Parser`] owns a knowledge-base context (which command, environment, and
//! character names are known, and what argument shapes they take) and a default
//! [`ParseConfig`]. It turns LaTeX source into an editable
//! [`Document`](crate::Document) without normalizing it. Build one with
//! [`Parser::builder`], choosing the knowledge packages and any per-context
//! customizations. To normalize as well as parse, use
//! [`TransformEngine`](crate::TransformEngine), which owns a parser internally.

use texform_core::parse::{self, ContextItem, ParseConfig};

use crate::parse_result::ParseResult;

/// A configured LaTeX parser.
///
/// Holds an immutable knowledge-base context plus a default [`ParseConfig`].
/// Parsing is a pure read of that context, so a `Parser` is cheap to clone and
/// safe to share across many parse calls.
#[derive(Clone, Debug)]
pub struct Parser {
    inner: parse::ParseContext,
    default_config: ParseConfig,
}

/// Builder for [`Parser`].
///
/// Selects the knowledge packages, applies per-context customizations (added or
/// removed commands, environments, and delimiter controls), and sets the
/// default [`ParseConfig`], before producing a [`Parser`] with [`build`](Self::build).
pub struct ParserBuilder {
    inner: parse::ParseContextBuilder,
    default_config: ParseConfig,
}

/// Failure building a [`Parser`] from a [`ParserBuilder`].
///
/// Reported when a requested knowledge package cannot be loaded or a supplied
/// context item is invalid.
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
    /// Start building a parser.
    ///
    /// The builder starts from the default runtime knowledge packages and the
    /// default [`ParseConfig`] ([`LENIENT`](ParseConfig::LENIENT)). Restrict the
    /// package set with [`ParserBuilder::packages`], or drop all built-in
    /// knowledge with [`ParserBuilder::empty_knowledge`], before
    /// [`build`](ParserBuilder::build).
    ///
    /// # Examples
    ///
    /// ```
    /// use texform::Parser;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let parser = Parser::builder().packages(&["base", "ams"]).build()?;
    /// let result = parser.parse(r"\alpha + \beta");
    /// assert!(result.document().is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> ParserBuilder {
        ParserBuilder {
            inner: parse::ParseContext::builder(),
            default_config: ParseConfig::default(),
        }
    }

    /// Parse a LaTeX formula with this parser's default configuration.
    ///
    /// The standalone [`Parser`] default is [`ParseConfig::LENIENT`]. Use
    /// [`parse_with`](Self::parse_with) to override per call. When this parser
    /// comes from [`TransformEngine::parser`](crate::TransformEngine::parser),
    /// documents extracted from the returned [`ParseResult`] can be edited and
    /// then passed back to that same engine's
    /// [`transform`](crate::TransformEngine::transform) method.
    pub fn parse(&self, src: &str) -> ParseResult {
        ParseResult::from_core(self.inner.parse(src, &self.default_config))
    }

    /// Parse a LaTeX formula with an explicit parse configuration.
    ///
    /// Like [`parse`](Self::parse), documents extracted from the returned
    /// [`ParseResult`] keep the parser identity needed for the same
    /// `TransformEngine` to transform them in place when this parser came from
    /// [`TransformEngine::parser`](crate::TransformEngine::parser).
    pub fn parse_with(&self, src: &str, config: &ParseConfig) -> ParseResult {
        ParseResult::from_core(self.inner.parse(src, config))
    }

    /// Look up the active command record for `name` in the given content mode.
    ///
    /// Resolves through any mode-specific overrides, so the returned record is
    /// the one the parser would actually use in `mode`. Returns `None` if no
    /// command by that name is known in that mode.
    pub fn lookup_command(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveCommandRecord> {
        self.inner.lookup_command(name, mode)
    }

    /// Look up a command record without applying implicit fallbacks.
    ///
    /// Unlike [`lookup_command`](Self::lookup_command), this returns a record
    /// only when `name` is explicitly defined for `mode`, ignoring cross-mode
    /// fallbacks.
    pub fn lookup_explicit_command(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveCommandRecord> {
        self.inner.lookup_explicit_command(name, mode)
    }

    /// Look up the active character record for `name` in the given content mode.
    ///
    /// Returns `None` if no special character by that name is known in `mode`.
    pub fn lookup_character(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveCharacterRecord> {
        self.inner.lookup_character(name, mode)
    }

    /// Look up the active environment record for `name` in the given content mode.
    ///
    /// Returns `None` if no environment by that name is known in `mode`.
    pub fn lookup_env(
        &self,
        name: &str,
        mode: parse::ContentMode,
    ) -> Option<&parse::ActiveEnvironmentRecord> {
        self.inner.lookup_env(name, mode)
    }

    /// Whether `name` is registered as a delimiter-control command (such as
    /// `\left` or `\right`).
    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    /// Whether a command named `name` is known in any content mode.
    pub fn knows_command_name(&self, name: &str) -> bool {
        self.inner.knows_command_name(name)
    }

    /// Whether an environment named `name` is known in any content mode.
    pub fn knows_env_name(&self, name: &str) -> bool {
        self.inner.knows_env_name(name)
    }

    /// Whether a special character named `name` is known in any content mode.
    pub fn knows_character_name(&self, name: &str) -> bool {
        self.inner.knows_character_name(name)
    }

    pub(crate) fn inner(&self) -> &parse::ParseContext {
        &self.inner
    }

    /// The default [`ParseConfig`] used by [`parse`](Self::parse).
    pub fn default_parse_config(&self) -> &ParseConfig {
        &self.default_config
    }
}

impl ParserBuilder {
    /// Restrict the parser context to exactly the named knowledge packages.
    ///
    /// Each name must be a built-in package (`base`, `ams`, `physics`,
    /// `braket`, `bboldx`, `boldsymbol`, `textmacros`); see
    /// [`list_packages`](crate::list_packages) for the catalog. This replaces
    /// the package set rather than adding to it: the last call wins, and with no
    /// call at all the builder loads the default runtime packages.
    pub fn packages(mut self, packages: &[&str]) -> Self {
        self.inner = self.inner.packages(packages);
        self
    }

    /// Start from an empty knowledge base with no packages loaded.
    ///
    /// Every command, environment, and character is then unknown unless added
    /// back with [`item`](Self::item). Useful for testing parser behavior in
    /// isolation from the built-in catalog.
    pub fn empty_knowledge(mut self) -> Self {
        self.inner = parse::ParseContextBuilder::empty();
        self
    }

    /// Set the default [`ParseConfig`] used by [`Parser::parse`].
    pub fn default_parse_config(mut self, config: ParseConfig) -> Self {
        self.default_config = config;
        self
    }

    /// Add a single context item (a command, environment, or delimiter-control
    /// definition) to the knowledge base.
    pub fn item(mut self, item: impl Into<ContextItem>) -> Self {
        self.inner = self.inner.insert_item(item);
        self
    }

    /// Alias for [`item`](Self::item).
    pub fn insert_item(self, item: impl Into<ContextItem>) -> Self {
        self.item(item)
    }

    /// Remove a command from the knowledge base by name.
    ///
    /// After removal the name parses as unknown, subject to
    /// [`ParseConfig::reject_unknown`].
    pub fn remove_command(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.remove_command(name);
        self
    }

    /// Remove an environment from the knowledge base by name.
    pub fn remove_environment(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.remove_environment(name);
        self
    }

    /// Remove a delimiter-control command (such as `\left`) by name.
    pub fn remove_delimiter_control(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.remove_delimiter_control(name);
        self
    }

    /// Build the [`Parser`].
    ///
    /// # Errors
    ///
    /// Returns [`ParserBuildError`] if a requested package fails to load or a
    /// supplied context item is invalid.
    pub fn build(self) -> Result<Parser, ParserBuildError> {
        Ok(Parser {
            inner: self.inner.build().map_err(ParserBuildError)?,
            default_config: self.default_config,
        })
    }
}
