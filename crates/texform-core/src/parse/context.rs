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
pub use texform_specs::specs::{AllowedMode, CharacterMeta, CommandKind, CommandMeta, EnvMeta};

use crate::ast::Ast;
pub use crate::knowledge::KnowledgeBase;
pub use crate::knowledge::PackageLoadError;
use crate::lexer::Token;
use crate::parser::{self, RelativeSpanEntry, TokenStream, TrackedNode, build_token_stream};

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

pub struct ParseContextBuilder {
    packages: Vec<String>,
    use_core_only: bool,
    use_empty: bool,
    ops: Vec<BuilderOp>,
}

impl ParseContextBuilder {
    pub fn new() -> Self {
        Self {
            packages: texform_specs::builtin::all_package_names()
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            use_core_only: false,
            use_empty: false,
            ops: Vec::new(),
        }
    }

    pub fn packages(mut self, packages: &[&str]) -> Self {
        self.packages = packages.iter().map(|name| (*name).to_string()).collect();
        self.use_core_only = false;
        self.use_empty = false;
        self
    }

    pub fn core_only(mut self) -> Self {
        self.use_core_only = true;
        self.use_empty = false;
        self.packages.clear();
        self
    }

    pub fn empty(mut self) -> Self {
        self.use_empty = true;
        self.use_core_only = false;
        self.packages.clear();
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
        let mut kb = if self.use_empty {
            KnowledgeBase::empty()
        } else if self.use_core_only {
            KnowledgeBase::core_only()
        } else {
            let refs = self.packages.iter().map(String::as_str).collect::<Vec<_>>();
            KnowledgeBase::try_build_from_packages(refs.as_slice())
                .map_err(ParseContextBuildError::PackageLoad)?
        };

        let mut mutation_summary = MutationSummary::default();

        for op in self.ops {
            match op {
                BuilderOp::Insert(item) => {
                    record_insert(&mut mutation_summary, &item);
                    kb.insert_item(item.clone()).map_err(|source| {
                        ParseContextBuildError::InvalidContextItem {
                            name: item.name().to_string(),
                            source,
                        }
                    })?;
                }
                BuilderOp::RemoveCommand(name) => {
                    mutation_summary.touched_commands.insert(name.clone());
                    kb.remove_item(CommandItem::new(
                        name,
                        CommandKind::Prefix,
                        AllowedMode::Math,
                        "",
                    ));
                }
                BuilderOp::RemoveEnvironment(name) => {
                    mutation_summary.touched_environments.insert(name.clone());
                    kb.remove_item(EnvironmentItem::new(
                        name,
                        AllowedMode::Math,
                        ContentMode::Math,
                        "",
                    ));
                }
                BuilderOp::RemoveDelimiterControl(name) => {
                    kb.remove_item(DelimiterControlItem::new(name));
                }
            }
        }

        Ok(ParseContext::from_parts(kb, mutation_summary))
    }
}

impl Default for ParseContextBuilder {
    fn default() -> Self {
        Self::new()
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
pub struct ParseAstOutput {
    pub ast: Ast,
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
/// | [`empty()`](Self::empty) | Nothing — not even core |
/// | [`core_only()`](Self::core_only) | Core package only |
/// | [`from_packages()`](Self::from_packages) | Core + named packages |
/// | [`all_packages()`](Self::all_packages) | Core + every registered package |
/// | [`all_packages_shared()`](Self::all_packages_shared) | Same as above, lazily cached `&'static` ref |
///
#[derive(Clone)]
pub struct ParseContext {
    kb: KnowledgeBase,
    mutation_summary: MutationSummary,
}

impl std::fmt::Debug for ParseContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseContext")
            .field("kb", &self.kb)
            .finish_non_exhaustive()
    }
}

impl ParseContext {
    pub(crate) fn from_parts(kb: KnowledgeBase, mutation_summary: MutationSummary) -> Self {
        ParseContext {
            kb,
            mutation_summary,
        }
    }

    pub(crate) fn kb(&self) -> &KnowledgeBase {
        &self.kb
    }

    pub(crate) fn mutation_summary(&self) -> &MutationSummary {
        &self.mutation_summary
    }

    /// Build an empty context with no package specs loaded.
    ///
    /// Useful as a blank slate when every definition will be injected manually.
    pub fn empty() -> Self {
        ParseContextBuilder::new()
            .empty()
            .build()
            .expect("empty parse context should build")
    }

    /// Build a context containing only core knowledge (line breaks, etc.)
    pub fn core_only() -> Self {
        ParseContextBuilder::new()
            .core_only()
            .build()
            .expect("core-only parse context should build")
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
        ParseContextBuilder::new()
            .packages(packages)
            .build()
            .expect("package parse context should build")
    }

    /// Fallible variant of [`from_packages`](Self::from_packages).
    ///
    /// Returns [`PackageLoadError`] instead of panicking when a package name
    /// is unrecognized.
    pub fn try_from_packages(packages: &[&str]) -> Result<Self, PackageLoadError> {
        ParseContextBuilder::new()
            .packages(packages)
            .build()
            .map_err(|error| match error {
                ParseContextBuildError::PackageLoad(error) => error,
                ParseContextBuildError::InvalidContextItem { .. } => {
                    panic!("try_from_packages should not hit invalid context item")
                }
            })
    }

    /// Build a context containing all registered packages.
    pub fn all_packages() -> Self {
        ParseContextBuilder::new()
            .build()
            .expect("all-packages parse context should build")
    }

    /// Borrow the lazily-initialized all-packages context.
    ///
    /// This is the cheapest way to parse with the full knowledge base: the
    /// context is built once on first call and shared for the process lifetime.
    pub fn all_packages_shared() -> &'static ParseContext {
        all_packages_ctx()
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

    pub fn parse_to_ast(&self, src: &str, strict: bool) -> Result<ParseAstOutput, ParseAstError> {
        let output = self.parse(src, strict);
        match (output.result, output.diagnostics) {
            (Some(result), diagnostics) if diagnostics.is_empty() => Ok(ParseAstOutput {
                ast: Ast::from_syntax_node(&result.node),
            }),
            (Some(_), diagnostics) => Err(ParseAstError::DiagnosticsPresent { diagnostics }),
            (None, diagnostics) => Err(ParseAstError::NoParseResult { diagnostics }),
        }
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
    let (output, errors) = parse_raw(kb, src, token_stream, strict);

    let result = output.map(|tracked| {
        let (node, span, records) = tracked.finish_root();
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
        .map(|err| convert_diagnostic(kb, src, err))
        .collect();

    ParseOutput {
        result,
        diagnostics,
    }
}

fn parse_raw(
    kb: &KnowledgeBase,
    src: &str,
    token_stream: TokenStream<'_>,
    strict: bool,
) -> (Option<TrackedNode>, Vec<Rich<'static, Token>>) {
    let (output, errors) = parser::math_block_parser_with_source(kb, strict, src)
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

fn convert_diagnostic(kb: &KnowledgeBase, src: &str, err: Rich<'static, Token>) -> ParseDiagnostic {
    let span = {
        let s = err.span();
        Span {
            start: s.start,
            end: s.end,
        }
    };

    let reason = err.reason();
    let contexts = err
        .contexts()
        .map(|(label, span)| ParseDiagnosticContext {
            label: format!("{label}"),
            span: Span {
                start: span.start,
                end: span.end,
            },
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
        chumsky::error::RichReason::Custom(msg) => (msg.clone(), Vec::new(), None),
    };

    let mut diagnostic = ParseDiagnostic {
        message,
        span,
        expected,
        found,
        contexts,
    };

    supplement_diagnostic_contexts(kb, src, &mut diagnostic);
    diagnostic
}

fn supplement_diagnostic_contexts(kb: &KnowledgeBase, src: &str, diagnostic: &mut ParseDiagnostic) {
    supplement_environment_mismatch_message(src, diagnostic);
    supplement_unknown_environment_message(kb, src, diagnostic);
    supplement_argument_validation_span(src, diagnostic);

    let needs_left_context = matches!(
        diagnostic.message.as_str(),
        "invalid \\left delimiter"
            | "missing \\right for \\left-delimited group"
            | "invalid \\right delimiter"
    );
    if !needs_left_context {
        return;
    }

    let Some((left_span, env_span)) = find_invalid_left_context(kb, src) else {
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

fn supplement_environment_mismatch_message(src: &str, diagnostic: &mut ParseDiagnostic) {
    let is_generic_parse_error = matches!(
        diagnostic.message.as_str(),
        "found '}' expected something else" | "found '}' expected something else, or end of input"
    );
    if !is_generic_parse_error {
        return;
    }

    let Some((expected, found, span)) =
        find_environment_name_mismatch(src, diagnostic.span.clone())
    else {
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
    kb: &KnowledgeBase,
    src: &str,
    diagnostic: &mut ParseDiagnostic,
) {
    let is_generic_begin_error = matches!(
        diagnostic.message.as_str(),
        "found '\\begin' expected something else"
            | "found '\\begin' expected something else, or end of input"
    );
    if !is_generic_begin_error {
        return;
    }

    let Some((name, span)) = find_unknown_environment_at_span(kb, src, diagnostic.span.clone())
    else {
        return;
    };

    diagnostic.message = format!("Unknown environment: {}", name);
    diagnostic.span = span;
    diagnostic.expected.clear();
    diagnostic.found = None;
}

fn supplement_argument_validation_span(src: &str, diagnostic: &mut ParseDiagnostic) {
    if !looks_like_argument_validation_message(diagnostic.message.as_str()) {
        return;
    }

    let Some(span_text) = src.get(diagnostic.span.start..diagnostic.span.end) else {
        return;
    };
    if !span_text.starts_with('\\') {
        return;
    }

    let Some(argument_span) = find_argument_surface_span(src, diagnostic.span.end) else {
        return;
    };
    diagnostic.span = argument_span;
}

fn looks_like_argument_validation_message(message: &str) -> bool {
    matches!(
        message,
        "failed to parse delimited argument content"
            | "invalid dimension argument"
            | "invalid integer argument"
            | "invalid delimiter argument"
            | "escape sequence should not appear in CSName"
            | "unbalanced brace in keyval"
            | "keyval missing key"
            | "keyval missing value"
            | "Too many column specifiers (perhaps looping column definitions?)"
            | "Missing close brace"
            | "First argument to * column specifier must be a number"
    ) || message.starts_with("Illegal pream-token (")
        || message.starts_with("Missing dimension or its units for ")
        || message.starts_with("Missing argument for ")
}

fn find_argument_surface_span(src: &str, after: usize) -> Option<Span> {
    let tokens: Vec<(Token, std::ops::Range<usize>)> = Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (token, span)
        })
        .collect();

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

fn find_invalid_left_context(kb: &KnowledgeBase, src: &str) -> Option<(Span, Option<Span>)> {
    let tokens: Vec<(Token, std::ops::Range<usize>)> = Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (token, span)
        })
        .collect();

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
                    Token::Char('.') => true,
                    Token::Char(c)
                        if matches!(c, '(' | ')' | '[' | ']' | '|' | '<' | '>' | '/' | '\\') =>
                    {
                        true
                    }
                    Token::LBracket | Token::RBracket => true,
                    Token::ControlSeq(name) => kb.lookup_delimiter_control(name.as_str()).is_some(),
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

fn find_environment_name_mismatch(src: &str, target_span: Span) -> Option<(String, String, Span)> {
    let tokens: Vec<(Token, std::ops::Range<usize>)> = Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (token, span)
        })
        .collect();

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
    kb: &KnowledgeBase,
    src: &str,
    target_span: Span,
) -> Option<(String, Span)> {
    let tokens: Vec<(Token, std::ops::Range<usize>)> = Token::lexer(src)
        .spanned()
        .map(|(token, span)| {
            let token = token.unwrap_or_else(|()| {
                panic!("Lexer error at byte offset {}..{}", span.start, span.end)
            });
            (token, span)
        })
        .collect();

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

        if parsed_name.is_empty() || kb.lookup_env(parsed_name.as_str()).is_some() {
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
        let debug = format!("{:?}", ParseContext::core_only());
        assert!(debug.contains("ParseContext"));
        assert!(debug.contains("kb"));
        assert!(!debug.contains("mutation_summary"));
    }
}
