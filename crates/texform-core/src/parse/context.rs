//! Parse context that owns a per-instance immutable knowledge base.
//!
//! [`ParseContext`] is the primary public API surface for freezing a knowledge
//! base and parsing LaTeX formulas with a stable package-backed view.
//!
//! The module also defines the shared output types ([`ParseResult`],
//! [`ParseDiagnostic`]) used by every parse entry point.

use super::diagnostics::convert_diagnostic;
use crate::parse::error::ParseFailure;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
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
pub use crate::knowledge::KnowledgeBase;
pub use crate::knowledge::PackageLoadError;
use crate::knowledge::default_package_names;

use crate::parse::grammar::{self, TokenStream, TrackedNode, build_token_stream};
use crate::parse::{ParseConfig, ParserState};

/// Process-wide identity for a fully-built parser context.
///
/// Documents parsed by a context carry this id so transform engines can reject
/// trees produced under a different parser context. Cloned contexts keep the
/// same id; independently built contexts get distinct ids even when their
/// package configuration is equivalent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParseContextId(u64);

static NEXT_PARSE_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_parse_context_id() -> ParseContextId {
    ParseContextId(NEXT_PARSE_CONTEXT_ID.fetch_add(1, Ordering::Relaxed))
}

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

/// Runtime command definition to be injected into a [`ParseContext`].
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

/// Runtime environment definition to be injected into a [`ParseContext`].
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

#[derive(Debug)]
pub enum ParseContextBuildError {
    PackageLoad(PackageLoadError),
    InvalidContextItem {
        name: String,
        source: ArgSpecParseError,
    },
}

enum KnowledgeBaseMode {
    DefaultPackages,
    Packages(Vec<String>),
    Empty,
}

pub struct ParseContextBuilder {
    mode: KnowledgeBaseMode,
    ops: Vec<BuilderOp>,
}

impl ParseContextBuilder {
    pub fn empty() -> Self {
        Self {
            mode: KnowledgeBaseMode::Empty,
            ops: Vec::new(),
        }
    }

    pub fn packages(mut self, packages: &[&str]) -> Self {
        self.mode =
            KnowledgeBaseMode::Packages(packages.iter().map(|name| (*name).to_string()).collect());
        self
    }

    pub fn insert_item(mut self, item: impl Into<ContextItem>) -> Self {
        self.ops.push(BuilderOp::Insert(item.into()));
        self
    }

    pub fn remove_command(mut self, name: impl Into<String>) -> Self {
        self.ops.push(BuilderOp::RemoveCommand(name.into()));
        self
    }

    pub fn remove_environment(mut self, name: impl Into<String>) -> Self {
        self.ops.push(BuilderOp::RemoveEnvironment(name.into()));
        self
    }

    pub fn remove_delimiter_control(mut self, name: impl Into<String>) -> Self {
        self.ops
            .push(BuilderOp::RemoveDelimiterControl(name.into()));
        self
    }

    pub fn build(self) -> Result<ParseContext, ParseContextBuildError> {
        let (mut math_kb, mut text_kb, enabled_packages) = match self.mode {
            KnowledgeBaseMode::Empty => {
                (KnowledgeBase::empty(), KnowledgeBase::empty(), Vec::new())
            }
            KnowledgeBaseMode::DefaultPackages => {
                let refs = default_package_names().to_vec();
                let enabled_packages = canonical_enabled_package_names(refs.as_slice())?;
                let math_kb = KnowledgeBase::try_build_from_packages_for_mode(
                    refs.as_slice(),
                    ContentMode::Math,
                )
                .map_err(ParseContextBuildError::PackageLoad)?;
                let text_kb = KnowledgeBase::try_build_from_packages_for_mode(
                    refs.as_slice(),
                    ContentMode::Text,
                )
                .map_err(ParseContextBuildError::PackageLoad)?;

                (math_kb, text_kb, enabled_packages)
            }
            KnowledgeBaseMode::Packages(packages) => {
                let refs = packages.iter().map(String::as_str).collect::<Vec<_>>();
                let enabled_packages = canonical_enabled_package_names(refs.as_slice())?;
                (
                    KnowledgeBase::try_build_from_packages_for_mode(
                        refs.as_slice(),
                        ContentMode::Math,
                    )
                    .map_err(ParseContextBuildError::PackageLoad)?,
                    KnowledgeBase::try_build_from_packages_for_mode(
                        refs.as_slice(),
                        ContentMode::Text,
                    )
                    .map_err(ParseContextBuildError::PackageLoad)?,
                    enabled_packages,
                )
            }
        };

        let mut mutation_summary = MutationSummary::default();

        for op in self.ops {
            match op {
                BuilderOp::Insert(item) => {
                    record_insert(&mut mutation_summary, &item);
                    insert_item_into_lane(&mut math_kb, &item, ContentMode::Math).map_err(
                        |source| ParseContextBuildError::InvalidContextItem {
                            name: item.name().to_string(),
                            source,
                        },
                    )?;
                    insert_item_into_lane(&mut text_kb, &item, ContentMode::Text).map_err(
                        |source| ParseContextBuildError::InvalidContextItem {
                            name: item.name().to_string(),
                            source,
                        },
                    )?;
                }
                BuilderOp::RemoveCommand(name) => {
                    mutation_summary.touched_commands.insert(name.clone());
                    math_kb.remove_command_by_name(name.as_str());
                    text_kb.remove_command_by_name(name.as_str());
                }
                BuilderOp::RemoveEnvironment(name) => {
                    mutation_summary.touched_environments.insert(name.clone());
                    math_kb.remove_environment_by_name(name.as_str());
                    text_kb.remove_environment_by_name(name.as_str());
                }
                BuilderOp::RemoveDelimiterControl(name) => {
                    let item = DelimiterControlItem::new(name);
                    math_kb.remove_item(item.clone());
                    text_kb.remove_item(item);
                }
            }
        }

        Ok(ParseContext::from_parts(
            math_kb,
            text_kb,
            mutation_summary,
            enabled_packages,
        ))
    }
}

fn canonical_enabled_package_names(
    requested: &[&str],
) -> Result<Vec<PackageName>, ParseContextBuildError> {
    let mut packages = Vec::new();
    for package in texform_knowledge::builtin::MANAGED_PACKAGE_IMPORT_ORDER {
        if requested.contains(&package.as_str()) {
            packages.push(*package);
        }
    }

    for requested_name in requested {
        if PackageName::from_str(requested_name).is_none() {
            return Err(ParseContextBuildError::PackageLoad(
                PackageLoadError::UnknownPackage {
                    name: (*requested_name).to_string(),
                },
            ));
        }
    }

    Ok(packages)
}

fn insert_item_into_lane(
    kb: &mut KnowledgeBase,
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

impl Default for ParseContextBuilder {
    fn default() -> Self {
        Self {
            mode: KnowledgeBaseMode::DefaultPackages,
            ops: Vec::new(),
        }
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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

/// Immutable parse context owning an isolated knowledge base.
///
/// A `ParseContext` is the main integration surface for callers that need to
/// freeze a fully-built knowledge base, query metadata, and parse LaTeX
/// formulas repeatedly.
///
/// # Construction
///
/// | Constructor | Loaded knowledge |
/// |---|---|
/// | [`empty()`](Self::empty) | Nothing |
/// | [`from_packages()`](Self::from_packages) | Named packages only |
/// | `Default::default()` | Default runtime packages |
/// | [`shared()`](Self::shared) | Same as above, lazily cached `&'static` ref |
///
#[derive(Clone)]
pub struct ParseContext {
    math_kb: Arc<KnowledgeBase>,
    text_kb: Arc<KnowledgeBase>,
    mutation_summary: MutationSummary,
    enabled_packages: Vec<PackageName>,
    id: ParseContextId,
}

impl std::fmt::Debug for ParseContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseContext")
            .field("math_kb", &self.math_kb)
            .field("text_kb", &self.text_kb)
            .field("enabled_packages", &self.enabled_packages)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Default for ParseContext {
    fn default() -> Self {
        ParseContextBuilder::default()
            .build()
            .expect("default parse context should build")
    }
}

impl ParseContext {
    pub fn builder() -> ParseContextBuilder {
        ParseContextBuilder::default()
    }

    pub(crate) fn from_parts(
        math_kb: KnowledgeBase,
        text_kb: KnowledgeBase,
        mutation_summary: MutationSummary,
        enabled_packages: Vec<PackageName>,
    ) -> Self {
        ParseContext {
            math_kb: Arc::new(math_kb),
            text_kb: Arc::new(text_kb),
            mutation_summary,
            enabled_packages,
            id: next_parse_context_id(),
        }
    }

    /// Stable identity for this parser context and its clones.
    ///
    /// Parsed [`Document`] values store this id. Transform engines compare it
    /// with the id of their own parser context before mutating a live document.
    pub fn id(&self) -> ParseContextId {
        self.id
    }

    pub fn mutation_summary(&self) -> &MutationSummary {
        &self.mutation_summary
    }

    pub fn enabled_packages(&self) -> &[PackageName] {
        self.enabled_packages.as_slice()
    }

    pub fn has_enabled_package(&self, package: PackageName) -> bool {
        self.enabled_packages.contains(&package)
    }

    /// Build an empty context with no package specs loaded.
    ///
    /// Useful as a blank slate when every definition will be injected manually.
    pub fn empty() -> Self {
        ParseContextBuilder::empty()
            .build()
            .expect("empty parse context should build")
    }

    /// Build a context from an explicit list of package names.
    /// The listed packages are imported in canonical order.
    ///
    /// # Panics
    ///
    /// Panics if any package name is unrecognized. Use [`try_from_packages`](Self::try_from_packages)
    /// for fallible loading.
    pub fn from_packages(packages: &[&str]) -> Self {
        ParseContextBuilder::empty()
            .packages(packages)
            .build()
            .expect("package parse context should build")
    }

    /// Fallible variant of [`from_packages`](Self::from_packages).
    ///
    /// Returns [`PackageLoadError`] instead of panicking when a package name
    /// is unrecognized.
    pub fn try_from_packages(packages: &[&str]) -> Result<Self, PackageLoadError> {
        ParseContextBuilder::empty()
            .packages(packages)
            .build()
            .map_err(|error| match error {
                ParseContextBuildError::PackageLoad(error) => error,
                ParseContextBuildError::InvalidContextItem { .. } => {
                    panic!("try_from_packages should not hit invalid context item")
                }
            })
    }

    /// Borrow the lazily-initialized default-package context.
    ///
    /// This is the cheapest way to parse with the default knowledge base: the
    /// context is built once on first call and shared for the process lifetime.
    pub fn shared() -> &'static ParseContext {
        shared_parser()
    }

    /// Check whether `name` is a registered delimiter control sequence.
    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.math_kb.is_delimiter_control(name) || self.text_kb.is_delimiter_control(name)
    }

    /// Look up a delimiter control by name, returning the interned name.
    pub fn lookup_delimiter_control(&self, name: &str) -> Option<&'static str> {
        self.math_kb
            .lookup_delimiter_control(name)
            .or_else(|| self.text_kb.lookup_delimiter_control(name))
    }

    pub fn lookup_delimiter(
        &self,
        name: &str,
        is_control_sequence: bool,
        mode: ContentMode,
    ) -> Option<&ActiveDelimiterRecord> {
        self.kb_for(mode)
            .lookup_delimiter(name, is_control_sequence)
    }

    /// Parse a LaTeX formula and return a unified output.
    ///
    /// Uses chumsky's output+errors semantics so that a partial syntax tree
    /// can coexist with diagnostics.
    pub fn parse(&self, src: &str, config: &ParseConfig) -> ParseResult {
        parse_with_context(self, src, config)
    }

    /// Look up the active command metadata for `name`.
    ///
    /// The active entry may come from an explicit command definition or a
    /// character-derived zero-arg view. Returns `None` if the name is unknown
    /// or has been suppressed.
    pub fn kb_for(&self, mode: ContentMode) -> &KnowledgeBase {
        match mode {
            ContentMode::Math => self.math_kb.as_ref(),
            ContentMode::Text => self.text_kb.as_ref(),
        }
    }

    pub fn math_kb(&self) -> &KnowledgeBase {
        self.math_kb.as_ref()
    }

    pub fn text_kb(&self) -> &KnowledgeBase {
        self.text_kb.as_ref()
    }

    /// Look up the active command metadata for `name` in the selected lane.
    pub fn lookup_command(&self, name: &str, mode: ContentMode) -> Option<&ActiveCommandRecord> {
        self.kb_for(mode).lookup_command(name)
    }

    /// Look up only the explicit (non-character-derived) command for `name`.
    pub fn lookup_explicit_command(
        &self,
        name: &str,
        mode: ContentMode,
    ) -> Option<&ActiveCommandRecord> {
        self.kb_for(mode).lookup_explicit_command(name)
    }

    /// Look up character metadata for a control sequence name.
    pub fn lookup_character(
        &self,
        name: &str,
        mode: ContentMode,
    ) -> Option<&ActiveCharacterRecord> {
        self.kb_for(mode).lookup_character(name)
    }

    /// Look up environment metadata by name.
    pub fn lookup_env(&self, name: &str, mode: ContentMode) -> Option<&ActiveEnvironmentRecord> {
        self.kb_for(mode).lookup_env(name)
    }

    pub fn knows_command_name(&self, name: &str) -> bool {
        self.knows_command_name_in(name, ContentMode::Math)
            || self.knows_command_name_in(name, ContentMode::Text)
    }

    pub fn knows_env_name(&self, name: &str) -> bool {
        self.knows_env_name_in(name, ContentMode::Math)
            || self.knows_env_name_in(name, ContentMode::Text)
    }

    pub fn knows_character_name(&self, name: &str) -> bool {
        self.knows_character_name_in(name, ContentMode::Math)
            || self.knows_character_name_in(name, ContentMode::Text)
    }

    fn knows_command_name_in(&self, name: &str, mode: ContentMode) -> bool {
        self.lookup_command(name, mode).is_some()
    }

    fn knows_env_name_in(&self, name: &str, mode: ContentMode) -> bool {
        self.lookup_env(name, mode).is_some()
    }

    fn knows_character_name_in(&self, name: &str, mode: ContentMode) -> bool {
        self.lookup_character(name, mode).is_some()
    }
}

fn shared_parser() -> &'static ParseContext {
    static DEFAULT: OnceLock<ParseContext> = OnceLock::new();
    DEFAULT.get_or_init(ParseContext::default)
}

pub(crate) fn parse_with_context(
    ctx: &ParseContext,
    src: &str,
    config: &ParseConfig,
) -> ParseResult {
    let token_stream = build_token_stream(src);
    let (output, mut errors) = parse_raw(ctx, src, token_stream, config);

    let document = output.map(|tracked| {
        let (node, span_tree, diagnostics) = tracked.finish_root();
        errors.extend(diagnostics);
        let mut document = Document::from_syntax_with_spans(&node, &span_tree)
            .expect("parser must produce a syntax root accepted by Document");
        document.set_parse_context_id(ctx.id());
        document
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
) -> (Option<TrackedNode>, Vec<ParseFailure<'static>>) {
    let state = ParserState::new(ctx, config, src);
    let (output, errors) = grammar::math_block_parser_with_source(&state, src)
        .then_ignore(end())
        .parse(token_stream)
        .into_output_errors();

    // Convert borrowed errors to owned so they outlive the token stream.
    let mut collected_errors = state.take_recovery_diagnostics();
    collected_errors.extend(errors.into_iter().map(|e| e.into_owned()));
    (output, collected_errors)
}
