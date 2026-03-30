//! Stateful parse context that owns a per-instance knowledge base.
//!
//! [`ParseContext`] is the primary public API surface for building a knowledge
//! base, injecting temporary definitions, and parsing LaTeX formulas. Each
//! context owns an independent knowledge base so concurrent callers cannot
//! interfere with each other.
//!
//! The module also defines the shared output types ([`ParseOutput`],
//! [`ParseResult`], [`ParseDiagnostic`]) used by every parse entry point.

use std::sync::OnceLock;

use chumsky::prelude::*;
use serde::Serialize;
use texform_interface::syntax_node::SyntaxNode;

pub use texform_interface::syntax_node::ContentMode;
pub use texform_specs::specs::{
    AllowedMode, ArgSpecParseError, CharacterMeta, CommandKind, CommandMeta, EnvMeta,
};

pub use crate::knowledge::PackageLoadError;
use crate::knowledge::{self, KnowledgeBase};
use crate::lexer::Token;
use crate::parser::{self, Spanned, TokenStream, build_token_stream};

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

/// Byte-offset span within the original source string.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct Span {
    /// Inclusive start byte offset
    pub start: usize,
    /// Exclusive end byte offset
    pub end: usize,
}

/// Successful (possibly partial) parse result.
///
/// Even when diagnostics are present, a partial syntax tree may still be
/// available here, allowing consumers to inspect whatever the parser managed
/// to produce.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi))]
pub struct ParseResult {
    /// The syntax tree produced by parsing
    pub node: SyntaxNode,
    /// Byte range of the parsed input
    pub span: Span,
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

/// Stateful parse context owning an isolated knowledge base.
///
/// A `ParseContext` is the main integration surface for callers that need to
/// configure which packages are loaded, inject temporary definitions, query
/// metadata, and parse LaTeX formulas repeatedly.
///
/// # Construction
///
/// | Constructor | Loaded knowledge |
/// |---|---|
/// | [`empty()`](Self::empty) | Nothing — not even core |
/// | [`core_only()`](Self::core_only) | Core package only |
/// | [`from_packages()`](Self::from_packages) | Core + named packages |
/// | [`all_packages()`](Self::all_packages) | Core + every registered package |
/// | [`all_packages_shared()`](Self::all_packages_shared) | Same as above, lazily cached `&'static` ref |
///
/// After construction, use [`insert_item`](Self::insert_item) /
/// [`remove_item`](Self::remove_item) to mutate the knowledge base at
/// runtime.
#[derive(Debug, Clone)]
pub struct ParseContext {
    kb: KnowledgeBase,
}

impl ParseContext {
    pub(crate) fn from_kb(kb: KnowledgeBase) -> Self {
        ParseContext { kb }
    }

    pub(crate) fn kb(&self) -> &KnowledgeBase {
        &self.kb
    }

    /// Build an empty context with no package specs loaded.
    ///
    /// Useful as a blank slate when every definition will be injected manually.
    pub fn empty() -> Self {
        Self::from_kb(knowledge::build_empty_kb())
    }

    /// Build a context containing only core knowledge (line breaks, etc.)
    pub fn core_only() -> Self {
        Self::from_kb(knowledge::build_core_only_kb())
    }

    /// Build a context from an explicit list of package names.
    ///
    /// Core knowledge is always loaded first; the listed packages are imported
    /// in canonical order on top.
    ///
    /// # Panics
    ///
    /// Panics if any package name is unrecognized. Use [`try_from_packages`](Self::try_from_packages)
    /// for fallible loading.
    pub fn from_packages(packages: &[&str]) -> Self {
        ParseContext {
            kb: knowledge::build_kb_from_packages(packages),
        }
    }

    /// Fallible variant of [`from_packages`](Self::from_packages).
    ///
    /// Returns [`PackageLoadError`] instead of panicking when a package name
    /// is unrecognized.
    pub fn try_from_packages(packages: &[&str]) -> Result<Self, PackageLoadError> {
        Ok(ParseContext {
            kb: knowledge::try_build_kb_from_packages(packages)?,
        })
    }

    /// Build a context containing all registered packages.
    pub fn all_packages() -> Self {
        let packages = texform_specs::packages::all_package_names();
        Self::from_packages(packages.as_slice())
    }

    /// Borrow the lazily-initialized all-packages context.
    ///
    /// This is the cheapest way to parse with the full knowledge base: the
    /// context is built once on first call and shared for the process lifetime.
    pub fn all_packages_shared() -> &'static ParseContext {
        all_packages_ctx()
    }

    /// Clone the cached all-packages context into an owned value.
    ///
    /// Use this when you need to mutate the context (e.g. inject items) while
    /// still starting from the full knowledge base.
    pub fn clone_all_packages() -> Self {
        Self::all_packages_shared().clone()
    }

    /// Inject a single context item into the knowledge base.
    ///
    /// Returns an error if the item's argument spec string is invalid.
    pub fn insert_item(&mut self, item: impl Into<ContextItem>) -> Result<(), ArgSpecParseError> {
        self.kb.insert_item(item.into())
    }

    /// Inject multiple context items in order.
    ///
    /// Stops at the first invalid spec and returns the error.
    pub fn insert_items<I, T>(&mut self, items: I) -> Result<(), ArgSpecParseError>
    where
        I: IntoIterator<Item = T>,
        T: Into<ContextItem>,
    {
        for item in items {
            self.insert_item(item)?;
        }
        Ok(())
    }

    /// Remove a previously inserted context item.
    ///
    /// Returns `true` if the item was found and removed.
    pub fn remove_item(&mut self, item: impl Into<ContextItem>) -> bool {
        self.kb.remove_item(item.into())
    }

    /// Check whether `name` is a registered delimiter control sequence.
    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.kb.is_delimiter_control(name)
    }

    /// Look up a delimiter control by name, returning the interned name.
    pub fn lookup_delimiter_control(&self, name: &str) -> Option<&'static str> {
        self.kb.lookup_delimiter_control(name)
    }

    /// Parse a LaTeX formula and return a unified output.
    ///
    /// Uses chumsky's output+errors semantics so that a partial syntax tree
    /// can coexist with diagnostics. Set `strict` to reject unknown commands.
    pub fn parse(&self, src: &str, strict: bool) -> ParseOutput {
        parse_with_kb(&self.kb, src, strict)
    }

    /// Look up the active command metadata for `name`.
    ///
    /// The active entry may come from an explicit command definition or a
    /// character-derived zero-arg view. Returns `None` if the name is unknown
    /// or has been suppressed.
    pub fn lookup_command(&self, name: &str) -> Option<&CommandMeta> {
        self.kb.lookup_command(name)
    }

    /// Look up only the explicit (non-character-derived) command for `name`.
    pub fn lookup_explicit_command(&self, name: &str) -> Option<&CommandMeta> {
        self.kb.lookup_explicit_command(name)
    }

    /// Look up character metadata for a control sequence name.
    pub fn lookup_character(&self, name: &str) -> Option<&CharacterMeta> {
        self.kb.lookup_character(name)
    }

    /// Look up environment metadata by name.
    pub fn lookup_env(&self, name: &str) -> Option<&EnvMeta> {
        self.kb.lookup_env(name)
    }
}

fn all_packages_ctx() -> &'static ParseContext {
    static DEFAULT: OnceLock<ParseContext> = OnceLock::new();
    DEFAULT.get_or_init(ParseContext::all_packages)
}

pub(crate) fn parse_with_kb(kb: &KnowledgeBase, src: &str, strict: bool) -> ParseOutput {
    let token_stream = build_token_stream(src);
    let (output, errors) = parse_raw(kb, token_stream, strict);

    let result = output.map(|(node, span)| ParseResult {
        node,
        span: Span {
            start: span.start,
            end: span.end,
        },
    });

    let diagnostics = errors.into_iter().map(convert_diagnostic).collect();

    ParseOutput {
        result,
        diagnostics,
    }
}

fn parse_raw(
    kb: &KnowledgeBase,
    token_stream: TokenStream<'_>,
    strict: bool,
) -> (Option<Spanned<SyntaxNode>>, Vec<Rich<'static, Token>>) {
    let (output, errors) = parser::math_block_parser(kb, strict)
        .map_with(|node, e| (node, e.span()))
        .then_ignore(end())
        .parse(token_stream)
        .into_output_errors();

    // Convert borrowed errors to owned so they outlive the token stream.
    let errors = errors.into_iter().map(|e| e.into_owned()).collect();
    (output, errors)
}

fn convert_diagnostic(err: Rich<'static, Token>) -> ParseDiagnostic {
    let span = {
        let s = err.span();
        Span {
            start: s.start,
            end: s.end,
        }
    };

    let reason = err.reason();

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
        chumsky::error::RichReason::Custom(msg) => (msg.clone(), Vec::new(), None),
    };

    ParseDiagnostic {
        message,
        span,
        expected,
        found,
    }
}
