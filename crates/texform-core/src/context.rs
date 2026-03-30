//! Parse context for per-instance knowledge base isolation.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextItem {
    Command(CommandItem),
    Environment(EnvironmentItem),
    DelimiterControl(DelimiterControlItem),
}

impl ContextItem {
    pub fn name(&self) -> &str {
        match self {
            ContextItem::Command(item) => item.name.as_str(),
            ContextItem::Environment(item) => item.name.as_str(),
            ContextItem::DelimiterControl(item) => item.name.as_str(),
        }
    }

    pub const fn target_tag(&self) -> &'static str {
        match self {
            ContextItem::Command(_) => "command",
            ContextItem::Environment(_) => "environment",
            ContextItem::DelimiterControl(_) => "delimiter control",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    pub name: String,
    pub kind: CommandKind,
    pub allowed_mode: AllowedMode,
    pub spec: String,
    pub tags: Vec<String>,
}

impl CommandItem {
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

    pub fn with_tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentItem {
    pub name: String,
    pub allowed_mode: AllowedMode,
    pub body_mode: ContentMode,
    pub spec: String,
    pub tags: Vec<String>,
}

impl EnvironmentItem {
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

    pub fn with_tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelimiterControlItem {
    pub name: String,
}

impl DelimiterControlItem {
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

/// Byte-offset span.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Successful (possibly partial) parse result.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi))]
pub struct ParseResult {
    pub node: SyntaxNode,
    pub span: Span,
}

/// A single diagnostic produced during parsing.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseDiagnostic {
    pub message: String,
    pub span: Span,
    pub expected: Vec<String>,
    pub found: Option<String>,
}

/// Unified parse output carrying an optional result and zero or more diagnostics.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseOutput {
    pub result: Option<ParseResult>,
    pub diagnostics: Vec<ParseDiagnostic>,
}

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
    pub fn empty() -> Self {
        Self::from_kb(knowledge::build_empty_kb())
    }

    /// Build a context containing only core knowledge.
    pub fn core_only() -> Self {
        Self::from_kb(knowledge::build_core_only_kb())
    }

    /// Build a context from package names.
    pub fn from_packages(packages: &[&str]) -> Self {
        ParseContext {
            kb: knowledge::build_kb_from_packages(packages),
        }
    }

    /// Build a context from package names, returning an error for unknown package names.
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

    /// Borrow the cached all-packages context.
    pub fn all_packages_shared() -> &'static ParseContext {
        all_packages_ctx()
    }

    /// Clone the cached all-packages context.
    pub fn clone_all_packages() -> Self {
        Self::all_packages_shared().clone()
    }

    pub fn insert_item(&mut self, item: impl Into<ContextItem>) -> Result<(), ArgSpecParseError> {
        self.kb.insert_item(item.into())
    }

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

    pub fn remove_item(&mut self, item: impl Into<ContextItem>) -> bool {
        self.kb.remove_item(item.into())
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.kb.is_delimiter_control(name)
    }
    pub fn lookup_delimiter_control(&self, name: &str) -> Option<&'static str> {
        self.kb.lookup_delimiter_control(name)
    }

    /// Parse a LaTeX formula and return a unified output.
    ///
    /// Uses chumsky's output+errors semantics (equivalent to `.into_output_errors()`)
    /// so that a partial syntax tree can coexist with diagnostics.
    pub fn parse(&self, src: &str, strict: bool) -> ParseOutput {
        parse_with_kb(&self.kb, src, strict)
    }

    pub fn lookup_command(&self, name: &str) -> Option<&CommandMeta> {
        self.kb.lookup_command(name)
    }

    pub fn lookup_explicit_command(&self, name: &str) -> Option<&CommandMeta> {
        self.kb.lookup_explicit_command(name)
    }

    pub fn lookup_character(&self, name: &str) -> Option<&CharacterMeta> {
        self.kb.lookup_character(name)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_from_packages(actual: &[&str], expected: &[&str]) {
        assert_eq!(actual, expected);
    }

    #[test]
    fn core_only_context_includes_core_command() {
        let ctx = ParseContext::core_only();
        let linebreak = ctx
            .lookup_command("\\")
            .expect("expected core linebreak command");
        assert_from_packages(linebreak.from_packages, &["core"]);
    }

    #[test]
    fn context_can_insert_and_remove_delimiter_controls() {
        let mut ctx = ParseContext::empty();
        assert!(ctx.lookup_delimiter_control("langle").is_none());

        ctx.insert_item(DelimiterControlItem::new("langle"))
            .expect("delimiter control item should be valid");
        assert!(ctx.lookup_delimiter_control("langle").is_some());
        assert_eq!(ctx.lookup_delimiter_control("langle"), Some("langle"));

        assert!(ctx.remove_item(DelimiterControlItem::new("langle")));
        assert!(ctx.lookup_delimiter_control("langle").is_none());
        assert_eq!(ctx.lookup_delimiter_control("langle"), None);
    }

    #[test]
    fn context_exposes_raw_character_and_explicit_command_views() {
        let ctx = ParseContext::from_packages(&["base", "physics"]);

        let div = ctx
            .lookup_command("div")
            .expect("expected active div command");
        assert_from_packages(div.from_packages, &["physics"]);
        assert!(!div.args.is_empty());

        let explicit_div = ctx
            .lookup_explicit_command("div")
            .expect("expected explicit div command");
        assert_from_packages(explicit_div.from_packages, &["physics"]);
        assert!(!explicit_div.args.is_empty());

        let character_div = ctx
            .lookup_character("div")
            .expect("expected raw div character");
        assert_eq!(character_div.package, "base");
        assert_eq!(character_div.unicode_value, "÷");

        let aa = ctx
            .lookup_command("AA")
            .expect("expected active AA command");
        assert_from_packages(aa.from_packages, &["base"]);
        assert!(aa.args.is_empty());
        assert!(ctx.lookup_explicit_command("AA").is_none());

        let character_aa = ctx
            .lookup_character("AA")
            .expect("expected raw AA character");
        assert_eq!(character_aa.package, "base");
        assert_eq!(character_aa.unicode_value, "Å");
    }
}
