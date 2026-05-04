//! Parse context that owns a per-instance immutable knowledge base.
//!
//! [`ParseContext`] is the primary public API surface for freezing a knowledge
//! base and parsing LaTeX formulas with a stable package-backed view.
//!
//! The module also defines the shared output types ([`ParseOutput`],
//! [`ParseResult`], [`ParseDiagnostic`]) used by every parse entry point.

use std::collections::HashSet;
use std::sync::OnceLock;

use chumsky::prelude::*;
use logos::Logos;
use serde::Serialize;
pub use texform_argspec::ArgSpecParseError;
use texform_interface::syntax_node::SyntaxNode;

pub use texform_interface::syntax_node::ContentMode;
use texform_specs::builtin::PackageName;
pub use texform_specs::specs::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveDelimiterRecord, ActiveEnvironmentRecord,
    AllowedMode, CommandKind,
};

use crate::ast::Ast;
pub use crate::knowledge::KnowledgeBase;
pub use crate::knowledge::PackageLoadError;
use crate::knowledge::default_package_names;
use crate::lexer::Token;
use crate::parser::{self, RelativeSpanEntry, TokenStream, TrackedNode, build_token_stream};

type LexedSource = Vec<(Token, std::ops::Range<usize>)>;

// Diagnostic kind is propagated through two independent channels because chumsky
// may discard context labels during error merging/deduplication, while Custom
// message strings survive intact.  The context-label channel is the primary one
// (cheaper to attach), and the message-prefix channel acts as a fallback.
const DIAGNOSTIC_KIND_CONTEXT_PREFIX: &str = "__texform_diagnostic_kind:";
const DIAGNOSTIC_KIND_MESSAGE_PREFIX: &str = "\x1etexform-kind:";
const DIAGNOSTIC_KIND_MESSAGE_SEPARATOR: char = '\x1e';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseDiagnosticKind {
    ArgumentValidation,
    CommandModeError,
    EnvironmentModeError,
    EnvironmentNameMismatch,
    LeftRightDelimiter,
    RawExpectedFound,
    TextScriptError,
    UnclosedInlineMath,
    UnknownEnvironment,
}

impl ParseDiagnosticKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            ParseDiagnosticKind::ArgumentValidation => "argument-validation",
            ParseDiagnosticKind::CommandModeError => "command-mode-error",
            ParseDiagnosticKind::EnvironmentModeError => "environment-mode-error",
            ParseDiagnosticKind::EnvironmentNameMismatch => "environment-name-mismatch",
            ParseDiagnosticKind::LeftRightDelimiter => "left-right-delimiter",
            ParseDiagnosticKind::RawExpectedFound => "raw-expected-found",
            ParseDiagnosticKind::TextScriptError => "text-script-error",
            ParseDiagnosticKind::UnclosedInlineMath => "unclosed-inline-math",
            ParseDiagnosticKind::UnknownEnvironment => "unknown-environment",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "argument-validation" => Some(Self::ArgumentValidation),
            "command-mode-error" => Some(Self::CommandModeError),
            "environment-mode-error" => Some(Self::EnvironmentModeError),
            "environment-name-mismatch" => Some(Self::EnvironmentNameMismatch),
            "left-right-delimiter" => Some(Self::LeftRightDelimiter),
            "raw-expected-found" => Some(Self::RawExpectedFound),
            "text-script-error" => Some(Self::TextScriptError),
            "unclosed-inline-math" => Some(Self::UnclosedInlineMath),
            "unknown-environment" => Some(Self::UnknownEnvironment),
            _ => None,
        }
    }

    pub(crate) fn context_label(self) -> String {
        format!("{DIAGNOSTIC_KIND_CONTEXT_PREFIX}{}", self.as_str())
    }

    pub(crate) fn from_context_label(label: &str) -> Option<Self> {
        Self::from_str(label.strip_prefix(DIAGNOSTIC_KIND_CONTEXT_PREFIX)?)
    }

    pub(crate) fn wrap_message(self, message: impl AsRef<str>) -> String {
        format!(
            "{DIAGNOSTIC_KIND_MESSAGE_PREFIX}{}{DIAGNOSTIC_KIND_MESSAGE_SEPARATOR}{}",
            self.as_str(),
            message.as_ref()
        )
    }

    pub(crate) fn split_message(message: &str) -> (Option<Self>, &str) {
        let Some(rest) = message.strip_prefix(DIAGNOSTIC_KIND_MESSAGE_PREFIX) else {
            return (None, message);
        };
        let Some((kind, public_message)) = rest.split_once(DIAGNOSTIC_KIND_MESSAGE_SEPARATOR)
        else {
            return (None, message);
        };
        (Self::from_str(kind), public_message)
    }
}

fn lex_source(src: &str) -> LexedSource {
    Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (token, span)
        })
        .collect()
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

    /// Human-readable tag for error messages (`"command"`, `"environment"`, etc.)
    pub const fn target_tag(&self) -> &'static str {
        match self {
            ContextItem::Command(_) => "command",
            ContextItem::Environment(_) => "environment",
            ContextItem::DelimiterControl(_) => "delimiter control",
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
pub(crate) struct MutationSummary {
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
    for package in texform_specs::builtin::MANAGED_PACKAGE_IMPORT_ORDER {
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

/// Successful (possibly partial) parse result.
///
/// Even when diagnostics are present, a partial syntax tree may still be
/// available here, allowing consumers to inspect whatever the parser managed
/// to produce.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct NodeSpanEntry {
    pub id: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi))]
pub struct ParseResult {
    /// The syntax tree produced by parsing
    pub node: SyntaxNode,
    /// Byte range of the parsed input
    pub span: Span,
    pub node_spans: Vec<NodeSpanEntry>,
}

impl ParseResult {
    pub fn span_for(&self, id: &str) -> Option<&Span> {
        self.node_spans
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| &entry.span)
    }
}

/// A single diagnostic produced during parsing.
///
/// Diagnostics carry both a human-readable message and structured
/// expected/found information for richer error reporting.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseDiagnostic {
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

/// Unified parse output carrying an optional result and zero or more diagnostics.
///
/// The design mirrors chumsky's `output + errors` semantics: a partial tree
/// may coexist with diagnostics, so consumers always receive as much
/// information as the parser could extract.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseOutput {
    /// Parse result, present even when diagnostics exist (partial parse)
    pub result: Option<ParseResult>,
    /// Zero or more diagnostics; empty on full success
    pub diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Debug, Clone)]
pub enum ParseAstError {
    NoParseResult { diagnostics: Vec<ParseDiagnostic> },
    DiagnosticsPresent { diagnostics: Vec<ParseDiagnostic> },
}

impl std::fmt::Display for ParseAstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseAstError::NoParseResult { .. } => f.write_str("parse produced no syntax tree"),
            ParseAstError::DiagnosticsPresent { .. } => f.write_str("parse produced diagnostics"),
        }
    }
}

impl std::error::Error for ParseAstError {}

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
    math_kb: KnowledgeBase,
    text_kb: KnowledgeBase,
    mutation_summary: MutationSummary,
    enabled_packages: Vec<PackageName>,
}

impl std::fmt::Debug for ParseContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseContext")
            .field("math_kb", &self.math_kb)
            .field("text_kb", &self.text_kb)
            .field("enabled_packages", &self.enabled_packages)
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
    pub(crate) fn from_parts(
        math_kb: KnowledgeBase,
        text_kb: KnowledgeBase,
        mutation_summary: MutationSummary,
        enabled_packages: Vec<PackageName>,
    ) -> Self {
        ParseContext {
            math_kb,
            text_kb,
            mutation_summary,
            enabled_packages,
        }
    }

    pub(crate) fn mutation_summary(&self) -> &MutationSummary {
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
        shared_ctx()
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
    /// can coexist with diagnostics. Set `strict` to reject unknown commands.
    pub fn parse(&self, src: &str, strict: bool) -> ParseOutput {
        parse_with_context(self, src, strict)
    }

    pub fn parse_to_ast(&self, src: &str, strict: bool) -> Result<Ast, ParseAstError> {
        let output = self.parse(src, strict);
        match (output.result, output.diagnostics) {
            (Some(result), diagnostics) if diagnostics.is_empty() => {
                Ok(Ast::from_syntax_root(&result.node))
            }
            (Some(_), diagnostics) => Err(ParseAstError::DiagnosticsPresent { diagnostics }),
            (None, diagnostics) => Err(ParseAstError::NoParseResult { diagnostics }),
        }
    }

    /// Look up the active command metadata for `name`.
    ///
    /// The active entry may come from an explicit command definition or a
    /// character-derived zero-arg view. Returns `None` if the name is unknown
    /// or has been suppressed.
    pub fn kb_for(&self, mode: ContentMode) -> &KnowledgeBase {
        match mode {
            ContentMode::Math => &self.math_kb,
            ContentMode::Text => &self.text_kb,
        }
    }

    pub fn math_kb(&self) -> &KnowledgeBase {
        &self.math_kb
    }

    pub fn text_kb(&self) -> &KnowledgeBase {
        &self.text_kb
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

    fn knows_command_name_in(&self, name: &str, mode: ContentMode) -> bool {
        self.lookup_command(name, mode).is_some()
    }

    fn knows_env_name_in(&self, name: &str, mode: ContentMode) -> bool {
        self.lookup_env(name, mode).is_some()
    }
}

fn shared_ctx() -> &'static ParseContext {
    static DEFAULT: OnceLock<ParseContext> = OnceLock::new();
    DEFAULT.get_or_init(ParseContext::default)
}

pub(crate) fn parse_with_context(ctx: &ParseContext, src: &str, strict: bool) -> ParseOutput {
    let token_stream = build_token_stream(src);
    let (output, mut errors) = parse_raw(ctx, src, token_stream, strict);

    let result = output.map(|tracked| {
        let (node, span, records, diagnostics) = tracked.finish_root();
        errors.extend(diagnostics);
        let span = Span {
            start: span.start,
            end: span.end,
        };

        ParseResult {
            node,
            span: span.clone(),
            node_spans: records.into_iter().map(node_span_entry).collect(),
        }
    });

    let diagnostics = errors
        .into_iter()
        .map(|err| convert_diagnostic(ctx, src, err))
        .collect();

    ParseOutput {
        result,
        diagnostics,
    }
}

fn parse_raw(
    ctx: &ParseContext,
    src: &str,
    token_stream: TokenStream<'_>,
    strict: bool,
) -> (Option<TrackedNode>, Vec<Rich<'static, Token>>) {
    let (output, errors) = parser::math_block_parser_with_source(ctx, strict, src)
        .then_ignore(end())
        .parse(token_stream)
        .into_output_errors();

    // Convert borrowed errors to owned so they outlive the token stream.
    let errors = errors.into_iter().map(|e| e.into_owned()).collect();
    (output, errors)
}

fn node_span_entry(entry: RelativeSpanEntry) -> NodeSpanEntry {
    NodeSpanEntry {
        id: entry.path,
        span: Span {
            start: entry.span.start,
            end: entry.span.end,
        },
    }
}

fn convert_diagnostic(ctx: &ParseContext, src: &str, err: Rich<'static, Token>) -> ParseDiagnostic {
    let span = {
        let s = err.span();
        Span {
            start: s.start,
            end: s.end,
        }
    };

    let reason = err.reason();
    let mut kind = None;
    let contexts = err
        .contexts()
        .filter_map(|(label, span)| {
            let label = format!("{label}");
            if let Some(context_kind) = ParseDiagnosticKind::from_context_label(label.as_str()) {
                kind.get_or_insert(context_kind);
                return None;
            }

            Some(ParseDiagnosticContext {
                label,
                span: Span {
                    start: span.start,
                    end: span.end,
                },
            })
        })
        .collect();

    let (message, expected, found) = match reason {
        chumsky::error::RichReason::ExpectedFound {
            expected: exp,
            found: f,
        } => {
            let expected: Vec<String> = exp.iter().map(|p| format!("{p}")).collect();
            let found = f.as_ref().map(|t| format!("{}", &**t));

            let msg = format!("{reason}");
            (msg, expected, found)
        }
        chumsky::error::RichReason::Custom(msg) => {
            let (message_kind, public_message) = ParseDiagnosticKind::split_message(msg.as_str());
            if let Some(message_kind) = message_kind {
                kind.get_or_insert(message_kind);
            }
            (public_message.to_string(), Vec::new(), None)
        }
    };

    let kind = kind.or_else(|| infer_raw_diagnostic_kind(expected.as_slice(), found.as_deref()));

    let mut diagnostic = ParseDiagnostic {
        message,
        span,
        expected,
        found,
        contexts,
    };

    supplement_diagnostic_contexts(ctx, src, kind, &mut diagnostic);
    diagnostic
}

/// Best-effort fallback for chumsky-generated `ExpectedFound` errors that carry
/// no explicit `ParseDiagnosticKind`.  The heuristics here match the token
/// patterns that chumsky emits for known parser structures (e.g. `}` from an
/// environment-name mismatch, `\begin` from an unknown environment).
fn infer_raw_diagnostic_kind(
    expected: &[String],
    found: Option<&str>,
) -> Option<ParseDiagnosticKind> {
    if expected.iter().any(|pattern| pattern == "'$'")
        && matches!(found, None | Some("$") | Some("\\text"))
    {
        return Some(ParseDiagnosticKind::UnclosedInlineMath);
    }

    match found {
        Some("}") => Some(ParseDiagnosticKind::EnvironmentNameMismatch),
        Some("\\begin") => Some(ParseDiagnosticKind::UnknownEnvironment),
        Some(_) if !expected.is_empty() => Some(ParseDiagnosticKind::RawExpectedFound),
        None if !expected.is_empty() => Some(ParseDiagnosticKind::RawExpectedFound),
        Some(_) | None => None,
    }
}

fn supplement_diagnostic_contexts(
    ctx: &ParseContext,
    src: &str,
    kind: Option<ParseDiagnosticKind>,
    diagnostic: &mut ParseDiagnostic,
) {
    let mut lexed = None;

    supplement_unclosed_inline_math_message(kind, src, diagnostic);
    supplement_environment_mode_error_message(kind, ctx, src, &mut lexed, diagnostic);
    supplement_environment_mismatch_message(kind, src, &mut lexed, diagnostic);
    supplement_unknown_environment_message(kind, ctx, src, &mut lexed, diagnostic);
    supplement_inner_content_error_span(kind, src, &mut lexed, diagnostic);
    supplement_argument_validation_span(kind, src, &mut lexed, diagnostic);

    let needs_left_context = kind == Some(ParseDiagnosticKind::LeftRightDelimiter);
    if !needs_left_context {
        return;
    }

    let Some((left_span, env_span)) =
        find_invalid_left_context(ctx, lexed.get_or_insert_with(|| lex_source(src)))
    else {
        return;
    };

    if !diagnostic
        .contexts
        .iter()
        .any(|context| context.label == "left-delimited group")
    {
        diagnostic.contexts.push(ParseDiagnosticContext {
            label: "left-delimited group".to_string(),
            span: left_span,
        });
    }

    if let Some(env_span) = env_span
        && !diagnostic
            .contexts
            .iter()
            .any(|context| context.label == "environment body")
    {
        diagnostic.contexts.push(ParseDiagnosticContext {
            label: "environment body".to_string(),
            span: env_span,
        });
    }
}

/// Normalize the lone inline-math opener message so recoverable content
/// subparses report the same generic tail error shape as the top-level parser.
fn supplement_unclosed_inline_math_message(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::UnclosedInlineMath) {
        return;
    }

    diagnostic.message = "found '$' expected something else, or end of input".to_string();
    if diagnostic.expected == ["something else", "'$'"] {
        diagnostic.expected = vec!["something else".to_string(), "end of input".to_string()];
    }
    if diagnostic.found.as_deref() == Some("\\text")
        && let Some(span) = find_inline_math_shift_after_command(src, diagnostic.span.clone())
    {
        diagnostic.span = span;
        diagnostic.found = Some("$".to_string());
    }
}

/// Locate the `$` that immediately starts a braced inline-math argument after a command span.
fn find_inline_math_shift_after_command(src: &str, command_span: Span) -> Option<Span> {
    let mut offset = command_span.end;
    while matches!(src.as_bytes().get(offset), Some(b' ' | b'\t' | b'\n')) {
        offset += 1;
    }
    if src.as_bytes().get(offset) != Some(&b'{') || src.as_bytes().get(offset + 1) != Some(&b'$') {
        return None;
    }

    Some(Span {
        start: offset + 1,
        end: offset + 2,
    })
}

fn supplement_environment_mode_error_message(
    kind: Option<ParseDiagnosticKind>,
    ctx: &ParseContext,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    // Compatibility fallback: raw ExpectedFound errors come from chumsky before
    // TeXForm has a parser-private diagnostic kind to attach.
    if !matches!(
        kind,
        Some(ParseDiagnosticKind::RawExpectedFound | ParseDiagnosticKind::EnvironmentNameMismatch)
    ) {
        return;
    }

    let Some((name, disallowed_mode, span)) = find_environment_mode_error_at_span(
        ctx,
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.clone(),
    )
    .or_else(|| {
        if diagnostic.span.start == 0 {
            find_first_known_but_disallowed_environment(
                ctx,
                lexed.get_or_insert_with(|| lex_source(src)),
            )
        } else {
            None
        }
    }) else {
        return;
    };

    diagnostic.message = format!(
        "Environment {} is not allowed in {} mode",
        name, disallowed_mode
    );
    diagnostic.span = span;
    diagnostic.expected.clear();
    diagnostic.found = None;
}

fn supplement_environment_mismatch_message(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::EnvironmentNameMismatch) {
        return;
    }

    let Some((expected, found, span)) = find_environment_name_mismatch(
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.clone(),
    ) else {
        return;
    };

    diagnostic.message = format!(
        "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
        expected, found
    );
    diagnostic.span = span;
    diagnostic.expected = vec![format!("\\end{{{}}}", expected)];
    diagnostic.found = Some(format!("\\end{{{}}}", found));
}

fn supplement_unknown_environment_message(
    kind: Option<ParseDiagnosticKind>,
    ctx: &ParseContext,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::UnknownEnvironment) {
        return;
    }

    let Some((name, span)) = find_unknown_environment_at_span(
        ctx,
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.clone(),
    ) else {
        return;
    };

    diagnostic.message = format!("Unknown environment: {}", name);
    diagnostic.span = span;
    diagnostic.expected.clear();
    diagnostic.found = None;
}

fn supplement_argument_validation_span(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    if kind != Some(ParseDiagnosticKind::ArgumentValidation) {
        return;
    }

    let Some(span_text) = src.get(diagnostic.span.start..diagnostic.span.end) else {
        return;
    };
    if !span_text.starts_with('\\') {
        return;
    }

    let Some(argument_span) = find_argument_surface_span(
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.end,
    ) else {
        return;
    };
    diagnostic.span = argument_span;
}

fn supplement_inner_content_error_span(
    kind: Option<ParseDiagnosticKind>,
    src: &str,
    lexed: &mut Option<LexedSource>,
    diagnostic: &mut ParseDiagnostic,
) {
    if !matches!(
        kind,
        Some(ParseDiagnosticKind::CommandModeError | ParseDiagnosticKind::TextScriptError)
    ) {
        return;
    }

    let Some(span_text) = src.get(diagnostic.span.start..diagnostic.span.end) else {
        return;
    };
    if !span_text.starts_with('\\') {
        return;
    }

    let Some(argument_span) = find_argument_surface_span(
        lexed.get_or_insert_with(|| lex_source(src)),
        diagnostic.span.end,
    ) else {
        return;
    };

    if kind == Some(ParseDiagnosticKind::TextScriptError)
        && let Some(span) = find_first_script_marker_in_span(src, argument_span.clone())
    {
        diagnostic.span = span;
        return;
    }

    let Some(command_name) = diagnostic
        .message
        .strip_prefix("Command ")
        .and_then(|rest| rest.split(" is not allowed in ").next())
    else {
        return;
    };

    if span_text == command_name {
        return;
    }

    if let Some(span) = find_command_name_in_span(src, argument_span, command_name) {
        diagnostic.span = span;
    }
}

fn find_first_script_marker_in_span(src: &str, span: Span) -> Option<Span> {
    let slice = src.get(span.start..span.end)?;
    let offset = slice.find(['^', '_'])?;
    Some(Span {
        start: span.start + offset,
        end: span.start + offset + 1,
    })
}

fn find_command_name_in_span(src: &str, span: Span, command_name: &str) -> Option<Span> {
    let slice = src.get(span.start..span.end)?;
    let offset = slice.find(command_name)?;
    Some(Span {
        start: span.start + offset,
        end: span.start + offset + command_name.len(),
    })
}

fn find_argument_surface_span(tokens: &LexedSource, after: usize) -> Option<Span> {
    let mut index = 0;
    while index < tokens.len() && tokens[index].1.end <= after {
        index += 1;
    }
    while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
        index += 1;
    }

    let Some((token, span)) = tokens.get(index) else {
        return None;
    };

    match token {
        Token::LBracket => {
            let mut brace_depth = 0usize;
            let mut bracket_depth = 0usize;
            let start = span.start;
            for (token, span) in tokens.iter().skip(index + 1) {
                match token {
                    Token::LBracket if brace_depth == 0 => bracket_depth += 1,
                    Token::RBracket if brace_depth == 0 => {
                        if bracket_depth == 0 {
                            return Some(Span {
                                start,
                                end: span.end,
                            });
                        }
                        bracket_depth -= 1;
                    }
                    Token::LBrace => brace_depth += 1,
                    Token::RBrace if brace_depth > 0 => brace_depth -= 1,
                    _ => {}
                }
            }
            None
        }
        Token::LBrace => {
            let mut depth = 0usize;
            let start = span.start;
            for (token, span) in tokens.iter().skip(index + 1) {
                match token {
                    Token::LBrace => depth += 1,
                    Token::RBrace => {
                        if depth == 0 {
                            return Some(Span {
                                start,
                                end: span.end,
                            });
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

fn find_invalid_left_context(
    ctx: &ParseContext,
    tokens: &LexedSource,
) -> Option<(Span, Option<Span>)> {
    let mut environment_stack = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        match &tokens[index].0 {
            Token::ControlSeq(name) if name == "begin" => {
                environment_stack.push(environment_body_start(&tokens, index));
            }
            Token::ControlSeq(name) if name == "end" => {
                environment_stack.pop();
            }
            Token::ControlSeq(name) if name == "left" => {
                let mut next = index + 1;
                while matches!(tokens.get(next), Some((Token::Whitespaces, _))) {
                    next += 1;
                }

                let Some((token, token_span)) = tokens.get(next) else {
                    let left_span = Span {
                        start: tokens[index].1.start,
                        end: tokens[index].1.end,
                    };
                    let env_span = environment_stack.last().map(|start| Span {
                        start: *start,
                        end: left_span.end,
                    });
                    return Some((left_span, env_span));
                };

                let is_valid_delimiter = match token {
                    Token::Char(c) => ctx
                        .lookup_delimiter(c.to_string().as_str(), false, ContentMode::Math)
                        .is_some(),
                    Token::LBracket => ctx
                        .lookup_delimiter("[", false, ContentMode::Math)
                        .is_some(),
                    Token::RBracket => ctx
                        .lookup_delimiter("]", false, ContentMode::Math)
                        .is_some(),
                    Token::ControlSeq(name) => ctx
                        .lookup_delimiter(name.as_str(), true, ContentMode::Math)
                        .is_some(),
                    _ => false,
                };

                if !is_valid_delimiter {
                    let left_span = Span {
                        start: tokens[index].1.start,
                        end: token_span.end,
                    };
                    let env_span = environment_stack.last().map(|start| Span {
                        start: *start,
                        end: token_span.end,
                    });
                    return Some((left_span, env_span));
                }
            }
            _ => {}
        }

        index += 1;
    }

    None
}

fn find_environment_name_mismatch(
    tokens: &LexedSource,
    target_span: Span,
) -> Option<(String, String, Span)> {
    let mut stack = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        let Some((Token::ControlSeq(head), _)) = tokens.get(index) else {
            index += 1;
            continue;
        };

        if !matches!(head.as_str(), "begin" | "end") {
            index += 1;
            continue;
        }

        let mut next = index + 1;
        while matches!(tokens.get(next), Some((Token::Whitespaces, _))) {
            next += 1;
        }
        if !matches!(tokens.get(next), Some((Token::LBrace, _))) {
            index += 1;
            continue;
        }
        next += 1;

        let mut env_name = String::new();
        while let Some((token, _)) = tokens.get(next) {
            match token {
                Token::Char(c) => env_name.push(*c),
                Token::Star => env_name.push('*'),
                Token::RBrace => break,
                _ => {
                    env_name.clear();
                    break;
                }
            }
            next += 1;
        }

        if env_name.is_empty() {
            index += 1;
            continue;
        }

        if head == "begin" {
            stack.push(env_name);
        } else if let Some(expected) = stack.last() {
            if expected == &env_name {
                stack.pop();
            } else {
                let mismatch_closer_span = Span {
                    start: tokens[next].1.start,
                    end: tokens[next].1.end,
                };
                if mismatch_closer_span.start != target_span.start
                    || mismatch_closer_span.end != target_span.end
                {
                    index += 1;
                    continue;
                }

                return Some((
                    expected.clone(),
                    env_name,
                    Span {
                        start: tokens[index].1.start,
                        end: tokens[next].1.end,
                    },
                ));
            }
        }

        index += 1;
    }

    None
}

fn find_unknown_environment_at_span(
    ctx: &ParseContext,
    tokens: &LexedSource,
    target_span: Span,
) -> Option<(String, Span)> {
    let mut index = 0;
    while index < tokens.len() {
        let Some((Token::ControlSeq(name), begin_span)) = tokens.get(index) else {
            index += 1;
            continue;
        };

        if name != "begin"
            || begin_span.start != target_span.start
            || begin_span.end != target_span.end
        {
            index += 1;
            continue;
        }

        index += 1;
        while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
            index += 1;
        }

        let Some((Token::LBrace, _)) = tokens.get(index) else {
            return None;
        };
        index += 1;

        let name_start = tokens.get(index)?.1.start;
        let mut parsed_name = String::new();
        let mut name_end = name_start;
        while let Some((token, span)) = tokens.get(index) {
            match token {
                Token::Char(ch) => {
                    parsed_name.push(*ch);
                    name_end = span.end;
                    index += 1;
                }
                Token::Star => {
                    parsed_name.push('*');
                    name_end = span.end;
                    index += 1;
                }
                Token::RBrace => break,
                _ => return None,
            }
        }

        if parsed_name.is_empty() || ctx.knows_env_name(parsed_name.as_str()) {
            return None;
        }

        return Some((
            parsed_name,
            Span {
                start: name_start,
                end: name_end,
            },
        ));
    }

    None
}

fn find_first_known_but_disallowed_environment(
    ctx: &ParseContext,
    tokens: &LexedSource,
) -> Option<(String, ContentMode, Span)> {
    let mut index = 0;
    while index < tokens.len() {
        let Some((Token::ControlSeq(name), head_span)) = tokens.get(index) else {
            index += 1;
            continue;
        };
        if name != "begin" {
            index += 1;
            continue;
        }

        let begin_start = head_span.start;
        index += 1;
        while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
            index += 1;
        }
        if !matches!(tokens.get(index), Some((Token::LBrace, _))) {
            continue;
        }
        index += 1;

        let mut parsed_name = String::new();
        while let Some((token, _)) = tokens.get(index) {
            match token {
                Token::Char(ch) => {
                    parsed_name.push(*ch);
                    index += 1;
                }
                Token::Star => {
                    parsed_name.push('*');
                    index += 1;
                }
                Token::RBrace => break,
                _ => return None,
            }
        }

        let Some((Token::RBrace, close_span)) = tokens.get(index) else {
            return None;
        };
        if parsed_name.is_empty() {
            index += 1;
            continue;
        }

        let math_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Math)
            .is_some();
        let text_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Text)
            .is_some();
        let disallowed_mode = match (math_known, text_known) {
            (false, true) => ContentMode::Math,
            (true, false) => ContentMode::Text,
            _ => {
                index += 1;
                continue;
            }
        };

        return Some((
            parsed_name,
            disallowed_mode,
            Span {
                start: begin_start,
                end: close_span.end,
            },
        ));
    }

    None
}

fn find_environment_mode_error_at_span(
    ctx: &ParseContext,
    tokens: &LexedSource,
    target_span: Span,
) -> Option<(String, ContentMode, Span)> {
    let mut index = 0;
    while index < tokens.len() {
        let Some((Token::ControlSeq(name), _)) = tokens.get(index) else {
            index += 1;
            continue;
        };
        if name != "begin" {
            index += 1;
            continue;
        }

        let begin_start = tokens[index].1.start;
        index += 1;
        while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
            index += 1;
        }
        if !matches!(tokens.get(index), Some((Token::LBrace, _))) {
            continue;
        }
        index += 1;

        let mut parsed_name = String::new();
        while let Some((token, _)) = tokens.get(index) {
            match token {
                Token::Char(ch) => {
                    parsed_name.push(*ch);
                    index += 1;
                }
                Token::Star => {
                    parsed_name.push('*');
                    index += 1;
                }
                Token::RBrace => break,
                _ => return None,
            }
        }

        let Some((Token::RBrace, close_span)) = tokens.get(index) else {
            return None;
        };

        let matches_target =
            close_span.start == target_span.start || close_span.end == target_span.end;
        if !matches_target || parsed_name.is_empty() {
            index += 1;
            continue;
        }

        let math_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Math)
            .is_some();
        let text_known = ctx
            .lookup_env(parsed_name.as_str(), ContentMode::Text)
            .is_some();
        let disallowed_mode = match (math_known, text_known) {
            (false, true) => ContentMode::Math,
            (true, false) => ContentMode::Text,
            _ => return None,
        };

        return Some((
            parsed_name,
            disallowed_mode,
            Span {
                start: begin_start,
                end: close_span.end,
            },
        ));
    }

    None
}

fn environment_body_start(tokens: &[(Token, std::ops::Range<usize>)], begin_index: usize) -> usize {
    let mut index = begin_index + 1;
    while matches!(tokens.get(index), Some((Token::Whitespaces, _))) {
        index += 1;
    }

    if !matches!(tokens.get(index), Some((Token::LBrace, _))) {
        return tokens[begin_index].1.start;
    }
    index += 1;

    while let Some((token, span)) = tokens.get(index) {
        if matches!(token, Token::RBrace) {
            return span.end;
        }
        index += 1;
    }

    tokens[begin_index].1.start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_context_debug_omits_mutation_summary() {
        let debug = format!("{:?}", ParseContext::empty());
        assert!(debug.contains("ParseContext"));
        assert!(debug.contains("kb"));
        assert!(!debug.contains("mutation_summary"));
    }

    #[test]
    fn eof_unclosed_inline_math_is_normalized() {
        let expected = vec!["something else".to_string(), "'$'".to_string()];
        let mut diagnostic = ParseDiagnostic {
            message: "found end of input expected something else, or '$'".to_string(),
            span: Span { start: 0, end: 2 },
            expected,
            found: None,
            contexts: Vec::new(),
        };

        supplement_diagnostic_contexts(
            &ParseContext::empty(),
            "$x",
            Some(ParseDiagnosticKind::UnclosedInlineMath),
            &mut diagnostic,
        );

        assert_eq!(
            diagnostic.message,
            "found '$' expected something else, or end of input"
        );
        assert_eq!(diagnostic.expected, ["something else", "end of input"]);
        assert_eq!(diagnostic.found, None);
    }

    #[test]
    fn argument_validation_span_uses_kind_not_message() {
        let mut diagnostic = ParseDiagnostic {
            message: "argument value was rejected".to_string(),
            span: Span { start: 0, end: 7 },
            expected: Vec::new(),
            found: None,
            contexts: Vec::new(),
        };

        supplement_diagnostic_contexts(
            &ParseContext::empty(),
            "\\hspace{bad}",
            Some(ParseDiagnosticKind::ArgumentValidation),
            &mut diagnostic,
        );

        assert_eq!(diagnostic.span, Span { start: 7, end: 12 });
    }
}
