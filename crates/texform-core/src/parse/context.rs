//! Immutable knowledge construction and the internal parser integration.
//!
//! [`KnowledgeBase`] shares a stable package-backed view across documents
//! and parser calls.
//!
//! The module also defines the shared output types ([`ParseResult`],
//! [`ParseDiagnostic`]) used by every parse entry point.

use super::diagnostics::convert_diagnostic;
use crate::parse::error::ParseFailure;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use chumsky::prelude::*;

use serde::Serialize;
pub use texform_argspec::ArgSpecParseError;
pub use texform_interface::syntax_node::ContentMode;
use texform_knowledge::builtin::PackageName;
pub use texform_knowledge::specs::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveDelimiterRecord, ActiveEnvironmentRecord,
    AllowedMode, CommandKind,
};

use crate::document::Document;
use crate::knowledge::Catalog;
pub use crate::knowledge::PackageLoadError;
use crate::knowledge::default_package_names;

use crate::parse::grammar::{self, TokenStream, TrackedNode, build_token_stream};
use crate::parse::{ParseConfig, ParserState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[serde(rename_all = "kebab-case")]
/// Machine-readable category of a parse diagnostic.
///
/// Each variant labels why a diagnostic was emitted, letting callers branch on
/// the cause without parsing the human-readable message. The kebab-case form
/// produced by [`ParseDiagnosticKind::as_str`] is the stable wire value.
pub enum ParseDiagnosticKind {
    /// A control sequence usable as an infix operator appeared where its
    /// left/right operands could not be resolved unambiguously.
    AmbiguousInfix,
    /// A command or environment argument failed argspec validation.
    ArgumentValidation,
    /// A command was used in a content mode it is not allowed in.
    CommandModeError,
    /// An unescaped `%` started a comment that swallowed the rest of an
    /// argument, leaving it unclosed.
    CommentTruncatedArgument,
    /// An environment was used in a content mode it is not allowed in.
    EnvironmentModeError,
    /// An `\end{...}` name did not match the opening `\begin{...}`.
    EnvironmentNameMismatch,
    /// A source character could not be tokenized, so no document was produced.
    InvalidCharacter,
    /// A `\left ... \right` group had an invalid delimiter or a missing
    /// `\right`.
    LeftRightDelimiter,
    /// Group nesting exceeded the configured maximum depth.
    MaxGroupDepthExceeded,
    /// A low-level expected-vs-found mismatch with no more specific category.
    RawExpectedFound,
    /// Sub/superscript syntax appeared in text mode, where it is not allowed.
    TextScriptError,
    /// An inline math segment (`$ ... $`) was opened but never closed.
    UnclosedInlineMath,
    /// A math-shift `$` appeared where it is not expected inside a math formula.
    UnexpectedMathShift,
    /// A command name is not present in the knowledge base, under a config that
    /// rejects unknown names.
    UnknownCommand,
    /// An environment name is not present in the knowledge base, under a config
    /// that rejects unknown names.
    UnknownEnvironment,
}

impl ParseDiagnosticKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ParseDiagnosticKind::AmbiguousInfix => "ambiguous-infix",
            ParseDiagnosticKind::ArgumentValidation => "argument-validation",
            ParseDiagnosticKind::CommandModeError => "command-mode-error",
            ParseDiagnosticKind::CommentTruncatedArgument => "comment-truncated-argument",
            ParseDiagnosticKind::EnvironmentModeError => "environment-mode-error",
            ParseDiagnosticKind::EnvironmentNameMismatch => "environment-name-mismatch",
            ParseDiagnosticKind::InvalidCharacter => "invalid-character",
            ParseDiagnosticKind::LeftRightDelimiter => "left-right-delimiter",
            ParseDiagnosticKind::MaxGroupDepthExceeded => "max-group-depth-exceeded",
            ParseDiagnosticKind::RawExpectedFound => "raw-expected-found",
            ParseDiagnosticKind::TextScriptError => "text-script-error",
            ParseDiagnosticKind::UnclosedInlineMath => "unclosed-inline-math",
            ParseDiagnosticKind::UnexpectedMathShift => "unexpected-math-shift",
            ParseDiagnosticKind::UnknownCommand => "unknown-command",
            ParseDiagnosticKind::UnknownEnvironment => "unknown-environment",
        }
    }
}

/// A runtime-injectable definition that augments the knowledge base.
///
/// Context items let callers add temporary commands, environments, or
/// delimiter controls without modifying the underlying package specs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextItem {
    /// A command definition (prefix, infix, or declarative)
    Command(CommandItem),
    /// An environment definition
    Environment(EnvironmentItem),
    /// A delimiter control sequence (e.g. `langle`, `rangle`)
    DelimiterControl(DelimiterControlItem),
}

impl ContextItem {
    /// Return the name of the underlying item (command name, env name, etc.)
    pub fn name(&self) -> &str {
        match self {
            ContextItem::Command(item) => item.name.as_str(),
            ContextItem::Environment(item) => item.name.as_str(),
            ContextItem::DelimiterControl(item) => item.name.as_str(),
        }
    }
}

/// Runtime command definition to be injected into a [`KnowledgeBase`].
///
/// The `spec` field uses the xparse-style argument specification string
/// (e.g. `"m m"` for two mandatory args, `"s o m"` for star + optional + mandatory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    /// Command name without leading backslash
    pub name: String,
    /// Prefix, infix, or declarative
    pub kind: CommandKind,
    /// Which content modes this command may appear in
    pub allowed_mode: AllowedMode,
    /// xparse-style argument specification string
    pub spec: String,
    /// Metadata tags for transform-stage filtering
    pub tags: Vec<String>,
}

impl CommandItem {
    /// Create a command item with no tags.
    pub fn new(
        name: impl Into<String>,
        kind: CommandKind,
        allowed_mode: AllowedMode,
        spec: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            allowed_mode,
            spec: spec.into(),
            tags: Vec::new(),
        }
    }

    /// Builder method to attach metadata tags.
    pub fn with_tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

/// Runtime environment definition to be injected into a [`KnowledgeBase`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentItem {
    /// Environment name (e.g. `"matrix"`, `"align"`)
    pub name: String,
    /// Which content modes this environment may appear in
    pub allowed_mode: AllowedMode,
    /// Content mode used to parse the environment body
    pub body_mode: ContentMode,
    /// xparse-style argument specification string
    pub spec: String,
    /// Metadata tags for transform-stage filtering
    pub tags: Vec<String>,
}

impl EnvironmentItem {
    /// Create an environment item with no tags.
    pub fn new(
        name: impl Into<String>,
        allowed_mode: AllowedMode,
        body_mode: ContentMode,
        spec: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            allowed_mode,
            body_mode,
            spec: spec.into(),
            tags: Vec::new(),
        }
    }

    /// Builder method to attach metadata tags.
    pub fn with_tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

/// Runtime delimiter control sequence to be registered in the knowledge base.
///
/// Delimiter controls are names (without backslash) that may appear after
/// `\left` / `\right` or in delimiter-typed argument slots (e.g. `langle`,
/// `rangle`, `|`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelimiterControlItem {
    /// Delimiter name without leading backslash
    pub name: String,
}

impl DelimiterControlItem {
    /// Create a delimiter control item.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl From<CommandItem> for ContextItem {
    fn from(item: CommandItem) -> Self {
        ContextItem::Command(item)
    }
}

impl From<EnvironmentItem> for ContextItem {
    fn from(item: EnvironmentItem) -> Self {
        ContextItem::Environment(item)
    }
}

impl From<DelimiterControlItem> for ContextItem {
    fn from(item: DelimiterControlItem) -> Self {
        ContextItem::DelimiterControl(item)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MutationSummary {
    pub touched_commands: HashSet<String>,
    pub touched_environments: HashSet<String>,
}

enum BuilderOp {
    Insert(ContextItem),
    RemoveCommand(String),
    RemoveEnvironment(String),
    RemoveDelimiterControl(String),
}

fn record_insert(summary: &mut MutationSummary, item: &ContextItem) {
    match item {
        ContextItem::Command(command) => {
            summary.touched_commands.insert(command.name.clone());
        }
        ContextItem::Environment(environment) => {
            summary
                .touched_environments
                .insert(environment.name.clone());
        }
        ContextItem::DelimiterControl(_) => {}
    }
}

/// Error returned by [`KnowledgeBaseBuilder::build`].
#[derive(Debug)]
#[non_exhaustive]
pub enum KnowledgeBaseBuildError {
    /// A requested package name is not a built-in package.
    PackageLoad(PackageLoadError),
    /// An item name cannot be produced by the LaTeX lexer, so the parser could never match it.
    InvalidName {
        /// The rejected item name.
        name: String,
    },
    /// An item's argument specification failed to parse.
    InvalidContextItem {
        /// Name of the rejected item.
        name: String,
        /// The argument-specification error.
        source: ArgSpecParseError,
    },
}

impl std::fmt::Display for KnowledgeBaseBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PackageLoad(error) => error.fmt(f),
            Self::InvalidName { name } => write!(f, "invalid context item name '{name}'"),
            Self::InvalidContextItem { name, source } => {
                write!(f, "invalid context item '{name}': {source}")
            }
        }
    }
}

impl std::error::Error for KnowledgeBaseBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PackageLoad(error) => Some(error),
            Self::InvalidName { .. } => None,
            Self::InvalidContextItem { source, .. } => Some(source),
        }
    }
}

/// Builder for an immutable [`KnowledgeBase`].
///
/// Operations apply in call order on top of the selected packages, so a later
/// `remove_*` hides an earlier [`item`](Self::item) with the same name.
#[derive(Default)]
pub struct KnowledgeBaseBuilder {
    packages: Option<Vec<String>>,
    ops: Vec<BuilderOp>,
}

impl KnowledgeBaseBuilder {
    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default().packages(&[])
    }

    /// Load exactly these built-in packages instead of the defaults.
    ///
    /// Packages are imported in canonical order regardless of the given order.
    /// An empty slice loads no built-in knowledge.
    pub fn packages(mut self, packages: &[&str]) -> Self {
        self.packages = Some(packages.iter().map(|name| (*name).to_string()).collect());
        self
    }

    /// Add a runtime command, environment, or delimiter control.
    pub fn item(mut self, item: impl Into<ContextItem>) -> Self {
        self.ops.push(BuilderOp::Insert(item.into()));
        self
    }

    /// Remove a command by name in both content modes.
    pub fn remove_command(mut self, name: impl Into<String>) -> Self {
        self.ops.push(BuilderOp::RemoveCommand(name.into()));
        self
    }

    /// Remove an environment by name in both content modes.
    pub fn remove_environment(mut self, name: impl Into<String>) -> Self {
        self.ops.push(BuilderOp::RemoveEnvironment(name.into()));
        self
    }

    /// Remove a delimiter control by name.
    pub fn remove_delimiter_control(mut self, name: impl Into<String>) -> Self {
        self.ops
            .push(BuilderOp::RemoveDelimiterControl(name.into()));
        self
    }

    /// Build a new knowledge-base instance with its own identity.
    ///
    /// # Errors
    ///
    /// Returns [`KnowledgeBaseBuildError`] for an unknown package, an item
    /// name the lexer cannot produce, or an invalid argument specification.
    pub fn build(self) -> Result<KnowledgeBase, KnowledgeBaseBuildError> {
        let names = self
            .packages
            .as_ref()
            .map(|names| names.iter().map(String::as_str).collect::<Vec<_>>())
            .unwrap_or_else(|| default_package_names().to_vec());
        let enabled_packages = canonical_enabled_package_names(&names)?;
        let mut math_catalog = Catalog::try_build_from_packages_for_mode(&names, ContentMode::Math)
            .map_err(KnowledgeBaseBuildError::PackageLoad)?;
        let mut text_catalog = Catalog::try_build_from_packages_for_mode(&names, ContentMode::Text)
            .map_err(KnowledgeBaseBuildError::PackageLoad)?;

        let mut mutation_summary = MutationSummary::default();

        for op in self.ops {
            match op {
                BuilderOp::Insert(item) => {
                    let valid_name = match &item {
                        ContextItem::Command(_) => {
                            crate::document::conformance::valid_command_name(item.name())
                        }
                        ContextItem::DelimiterControl(_) => {
                            crate::document::conformance::valid_control_name(item.name())
                        }
                        ContextItem::Environment(_) => {
                            crate::document::conformance::valid_environment_name(item.name())
                        }
                    };
                    if !valid_name {
                        return Err(KnowledgeBaseBuildError::InvalidName {
                            name: item.name().to_string(),
                        });
                    }
                    record_insert(&mut mutation_summary, &item);
                    insert_item_into_lane(&mut math_catalog, &item, ContentMode::Math).map_err(
                        |source| KnowledgeBaseBuildError::InvalidContextItem {
                            name: item.name().to_string(),
                            source,
                        },
                    )?;
                    insert_item_into_lane(&mut text_catalog, &item, ContentMode::Text).map_err(
                        |source| KnowledgeBaseBuildError::InvalidContextItem {
                            name: item.name().to_string(),
                            source,
                        },
                    )?;
                }
                BuilderOp::RemoveCommand(name) => {
                    mutation_summary.touched_commands.insert(name.clone());
                    math_catalog.remove_command_by_name(name.as_str());
                    text_catalog.remove_command_by_name(name.as_str());
                }
                BuilderOp::RemoveEnvironment(name) => {
                    mutation_summary.touched_environments.insert(name.clone());
                    math_catalog.remove_environment_by_name(name.as_str());
                    text_catalog.remove_environment_by_name(name.as_str());
                }
                BuilderOp::RemoveDelimiterControl(name) => {
                    let item = DelimiterControlItem::new(name);
                    math_catalog.remove_item(item.clone());
                    text_catalog.remove_item(item);
                }
            }
        }

        Ok(ParseContext::from_parts(
            math_catalog,
            text_catalog,
            mutation_summary,
            enabled_packages,
        ))
    }
}

fn canonical_enabled_package_names(
    requested: &[&str],
) -> Result<Vec<PackageName>, KnowledgeBaseBuildError> {
    let mut packages = Vec::new();
    for package in texform_knowledge::builtin::MANAGED_PACKAGE_IMPORT_ORDER {
        if requested.contains(&package.as_str()) {
            packages.push(*package);
        }
    }

    for requested_name in requested {
        if PackageName::from_str(requested_name).is_none() {
            return Err(KnowledgeBaseBuildError::PackageLoad(
                PackageLoadError::UnknownPackage {
                    name: (*requested_name).to_string(),
                },
            ));
        }
    }

    Ok(packages)
}

fn insert_item_into_lane(
    kb: &mut Catalog,
    item: &ContextItem,
    mode: ContentMode,
) -> Result<(), ArgSpecParseError> {
    match item {
        ContextItem::Command(command) => {
            if command.allowed_mode.allows(mode) {
                kb.insert_item(command.clone())?;
            }
            Ok(())
        }
        ContextItem::Environment(environment) => {
            if environment.allowed_mode.allows(mode) {
                kb.insert_item(environment.clone())?;
            }
            Ok(())
        }
        ContextItem::DelimiterControl(item) => kb.insert_item(item.clone()),
    }
}

/// Byte-offset span within the original source string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct Span {
    /// Inclusive start byte offset
    pub start: usize,
    /// Exclusive end byte offset
    pub end: usize,
}

/// Additional source span attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseDiagnosticContext {
    /// Human-readable label for this related span
    pub label: String,
    /// Source location referenced by the label
    pub span: Span,
}

/// Unified parse result carrying an optional document and zero or more diagnostics.
///
/// The design mirrors chumsky's `output + errors` semantics: a partial document
/// may coexist with diagnostics, so consumers always receive as much
/// information as the parser could extract.
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// Parsed document, present even when diagnostics exist for recovered input.
    pub document: Option<Document>,
    /// Zero or more diagnostics; empty on full success.
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl ParseResult {
    /// Borrow the parsed document, if one was produced.
    pub fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    /// Borrow parse diagnostics.
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        self.diagnostics.as_slice()
    }

    /// Consume the result and return only diagnostics.
    pub fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    /// `true` when a recovered document contains one or more `Error` nodes.
    pub fn has_errors(&self) -> bool {
        self.document.as_ref().is_some_and(Document::has_errors)
    }

    /// Return the document and diagnostics when the document is editable.
    pub fn try_into_document(self) -> Result<(Document, Vec<ParseDiagnostic>), ParseError> {
        match (self.document, self.diagnostics) {
            (Some(document), diagnostics) if !document.has_errors() => Ok((document, diagnostics)),
            (document, diagnostics) => Err(ParseError {
                diagnostics,
                document: document.map(Box::new),
            }),
        }
    }

    /// Consume the result into its two public parts.
    pub fn into_parts(self) -> (Option<Document>, Vec<ParseDiagnostic>) {
        (self.document, self.diagnostics)
    }
}

/// A single diagnostic produced during parsing.
///
/// Diagnostics carry both a human-readable message and structured
/// expected/found information for richer error reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[non_exhaustive]
pub struct ParseDiagnostic {
    /// Stable machine-readable diagnostic kind, when available
    pub kind: Option<ParseDiagnosticKind>,
    /// Human-readable error description
    pub message: String,
    /// Source location of the error
    pub span: Span,
    /// Tokens or patterns the parser expected at this point
    pub expected: Vec<String>,
    /// Token actually found, if any
    pub found: Option<String>,
    /// Additional related source ranges for richer diagnostics
    pub contexts: Vec<ParseDiagnosticContext>,
}

impl ParseDiagnostic {
    pub fn new(
        message: impl Into<String>,
        span: Span,
        expected: Vec<String>,
        found: Option<String>,
        contexts: Vec<ParseDiagnosticContext>,
    ) -> Self {
        Self {
            kind: None,
            message: message.into(),
            span,
            expected,
            found,
            contexts,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub diagnostics: Vec<ParseDiagnostic>,
    pub document: Option<Box<Document>>,
}

impl ParseError {
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        self.diagnostics.as_slice()
    }

    pub fn document(&self) -> Option<&Document> {
        self.document.as_deref()
    }

    pub fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    pub fn into_parts(self) -> (Option<Document>, Vec<ParseDiagnostic>) {
        (self.document.map(|document| *document), self.diagnostics)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.document.is_some() {
            f.write_str("parse produced an incomplete document")
        } else {
            f.write_str("parse produced no document")
        }
    }
}

impl std::error::Error for ParseError {}

/// Immutable command, environment, character, and delimiter knowledge.
///
/// A knowledge base holds one catalog per content mode and never changes after
/// [`KnowledgeBaseBuilder::build`]. Cloning is cheap and preserves identity:
/// two handles are the same knowledge base only when [`ptr_eq`](Self::ptr_eq)
/// holds, even if independently built instances have identical contents.
/// [`Default`] returns a process-wide shared instance with the default packages.
#[derive(Clone, Debug)]
pub struct KnowledgeBase(Arc<KnowledgeBaseInner>);

#[derive(Debug)]
struct KnowledgeBaseInner {
    math_catalog: Catalog,
    text_catalog: Catalog,
    mutation_summary: MutationSummary,
    enabled_packages: Vec<PackageName>,
}

// Internal aliases of the `KnowledgeBase*` types, kept for parser and transform internals.
#[doc(hidden)]
pub type ParseContext = KnowledgeBase;
#[doc(hidden)]
pub type ParseContextBuilder = KnowledgeBaseBuilder;
#[doc(hidden)]
pub type ParseContextBuildError = KnowledgeBaseBuildError;

impl Default for KnowledgeBase {
    fn default() -> Self {
        Self::shared().clone()
    }
}

impl KnowledgeBase {
    /// Start building a knowledge base from the default packages.
    pub fn builder() -> KnowledgeBaseBuilder {
        KnowledgeBaseBuilder::default()
    }

    fn from_parts(
        math_catalog: Catalog,
        text_catalog: Catalog,
        mutation_summary: MutationSummary,
        enabled_packages: Vec<PackageName>,
    ) -> Self {
        Self(Arc::new(KnowledgeBaseInner {
            math_catalog,
            text_catalog,
            mutation_summary,
            enabled_packages,
        }))
    }

    /// Whether both handles share the same immutable knowledge instance.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Process-local identity for host-language identity hashing.
    #[doc(hidden)]
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }

    /// Loaded packages in canonical import order.
    pub fn packages(&self) -> Vec<&'static str> {
        self.0
            .enabled_packages
            .iter()
            .map(|package| package.as_str())
            .collect()
    }

    /// Active commands in `mode`, sorted by name.
    pub fn commands(&self, mode: ContentMode) -> Vec<&ActiveCommandRecord> {
        self.catalog(mode).commands()
    }

    /// Environments in `mode`, sorted by name.
    pub fn environments(&self, mode: ContentMode) -> Vec<&ActiveEnvironmentRecord> {
        self.catalog(mode).environments()
    }

    /// Characters in `mode`, sorted by name.
    pub fn characters(&self, mode: ContentMode) -> Vec<&ActiveCharacterRecord> {
        self.catalog(mode).characters()
    }

    /// Delimiters of both modes, deduplicated and sorted by name and control-sequence flag.
    pub fn delimiters(&self) -> Vec<&ActiveDelimiterRecord> {
        let mut records = self.0.math_catalog.delimiters();
        records.extend(self.0.text_catalog.delimiters());
        records.sort_by_key(|record| (record.name, record.is_control_sequence));
        records.dedup_by_key(|record| (record.name, record.is_control_sequence));
        records
    }

    #[doc(hidden)]
    pub fn mutation_summary(&self) -> &MutationSummary {
        &self.0.mutation_summary
    }

    #[doc(hidden)]
    pub fn enabled_packages(&self) -> &[PackageName] {
        self.0.enabled_packages.as_slice()
    }

    /// Build an empty context with no package specs loaded.
    #[doc(hidden)]
    pub fn empty() -> Self {
        KnowledgeBaseBuilder::empty()
            .build()
            .expect("empty parse context should build")
    }

    /// Panicking variant of [`try_from_packages`](Self::try_from_packages) for tests.
    #[doc(hidden)]
    pub fn from_packages(packages: &[&str]) -> Self {
        Self::try_from_packages(packages).expect("package knowledge base should build")
    }

    /// Build knowledge from explicit packages, returning unknown-package errors.
    #[doc(hidden)]
    pub fn try_from_packages(packages: &[&str]) -> Result<Self, PackageLoadError> {
        KnowledgeBaseBuilder::empty()
            .packages(packages)
            .build()
            .map_err(|error| match error {
                KnowledgeBaseBuildError::PackageLoad(error) => error,
                _ => unreachable!("package-only builds have no items"),
            })
    }

    /// Borrow the lazily-initialized default-package knowledge base.
    #[doc(hidden)]
    pub fn shared() -> &'static ParseContext {
        shared_parser()
    }

    /// Whether `name` is a registered delimiter control sequence in either mode.
    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.0.math_catalog.is_delimiter_control(name)
            || self.0.text_catalog.is_delimiter_control(name)
    }

    #[doc(hidden)]
    pub fn lookup_delimiter_control(&self, name: &str) -> Option<&'static str> {
        self.0
            .math_catalog
            .lookup_delimiter_control(name)
            .or_else(|| self.0.text_catalog.lookup_delimiter_control(name))
    }

    #[doc(hidden)]
    pub fn lookup_delimiter(
        &self,
        name: &str,
        is_control_sequence: bool,
        mode: ContentMode,
    ) -> Option<&ActiveDelimiterRecord> {
        self.catalog(mode)
            .lookup_delimiter(name, is_control_sequence)
    }

    /// Parse a LaTeX formula; the facade `Parser` is the public entry point.
    #[doc(hidden)]
    pub fn parse(&self, src: &str, config: &ParseConfig) -> ParseResult {
        parse_with_context(self, src, config)
    }

    #[doc(hidden)]
    pub fn catalog(&self, mode: ContentMode) -> &Catalog {
        match mode {
            ContentMode::Math => &self.0.math_catalog,
            ContentMode::Text => &self.0.text_catalog,
        }
    }

    #[doc(hidden)]
    pub fn math_catalog(&self) -> &Catalog {
        &self.0.math_catalog
    }

    #[doc(hidden)]
    pub fn text_catalog(&self) -> &Catalog {
        &self.0.text_catalog
    }

    /// Look up the active command for `name` in `mode`.
    ///
    /// The active entry may be an explicit command or a zero-argument view of
    /// a character. Returns `None` if the name is unknown or was removed.
    pub fn lookup_command(&self, name: &str, mode: ContentMode) -> Option<&ActiveCommandRecord> {
        self.catalog(mode).lookup_command(name)
    }

    /// Look up only an explicit (non-character-derived) command for `name` in `mode`.
    pub fn lookup_explicit_command(
        &self,
        name: &str,
        mode: ContentMode,
    ) -> Option<&ActiveCommandRecord> {
        self.catalog(mode).lookup_explicit_command(name)
    }

    /// Look up character metadata for a control-sequence name in `mode`.
    pub fn lookup_character(
        &self,
        name: &str,
        mode: ContentMode,
    ) -> Option<&ActiveCharacterRecord> {
        self.catalog(mode).lookup_character(name)
    }

    /// Look up environment metadata by name in `mode`.
    pub fn lookup_env(&self, name: &str, mode: ContentMode) -> Option<&ActiveEnvironmentRecord> {
        self.catalog(mode).lookup_env(name)
    }

    /// Whether `name` is an active command in either mode.
    pub fn knows_command_name(&self, name: &str) -> bool {
        self.lookup_command(name, ContentMode::Math).is_some()
            || self.lookup_command(name, ContentMode::Text).is_some()
    }

    /// Whether `name` is an environment in either mode.
    pub fn knows_env_name(&self, name: &str) -> bool {
        self.lookup_env(name, ContentMode::Math).is_some()
            || self.lookup_env(name, ContentMode::Text).is_some()
    }

    /// Whether `name` is a character in either mode.
    pub fn knows_character_name(&self, name: &str) -> bool {
        self.lookup_character(name, ContentMode::Math).is_some()
            || self.lookup_character(name, ContentMode::Text).is_some()
    }
}

fn shared_parser() -> &'static ParseContext {
    static DEFAULT: OnceLock<ParseContext> = OnceLock::new();
    DEFAULT.get_or_init(|| {
        KnowledgeBase::builder()
            .build()
            .expect("default knowledge packages must build")
    })
}

pub(crate) fn parse_with_context(
    ctx: &ParseContext,
    src: &str,
    config: &ParseConfig,
) -> ParseResult {
    parse_with_context_mode(ctx, src, config, ContentMode::Math)
}

pub(crate) fn parse_with_context_mode(
    ctx: &ParseContext,
    src: &str,
    config: &ParseConfig,
    mode: ContentMode,
) -> ParseResult {
    let token_stream = match build_token_stream(src) {
        Ok(tokens) => tokens,
        Err(error) => {
            return ParseResult {
                document: None,
                diagnostics: vec![convert_diagnostic(ctx, src, error).1],
            };
        }
    };
    let (output, mut errors) = parse_raw(ctx, src, token_stream, config, mode);

    let document = output.map(|tracked| {
        let (node, span_tree, diagnostics) = tracked.finish_root();
        errors.extend(diagnostics);
        Document::from_syntax_with_spans(ctx, &node, &span_tree)
    });

    let mut diagnostics: Vec<_> = errors
        .into_iter()
        .map(|err| convert_diagnostic(ctx, src, err))
        .collect();
    diagnostics.sort_by_key(|(priority, _)| *priority);
    let diagnostics = diagnostics
        .into_iter()
        .map(|(_, diagnostic)| diagnostic)
        .collect();

    ParseResult {
        document,
        diagnostics,
    }
}

fn parse_raw(
    ctx: &ParseContext,
    src: &str,
    token_stream: TokenStream<'_>,
    config: &ParseConfig,
    mode: ContentMode,
) -> (Option<TrackedNode>, Vec<ParseFailure<'static>>) {
    let state = ParserState::new(ctx, config, src);
    let (output, errors) = grammar::content_block_parser_with_source(mode, &state, src)
        .then_ignore(end())
        .parse(token_stream)
        .into_output_errors();

    // Convert borrowed errors to owned so they outlive the token stream.
    let mut collected_errors = state.take_recovery_diagnostics();
    collected_errors.extend(errors.into_iter().map(|e| e.into_owned()));
    (output, collected_errors)
}
