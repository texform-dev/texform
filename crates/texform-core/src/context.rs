//! Parse context for per-instance knowledge base isolation.

use std::sync::OnceLock;

use texform_interface::syntax_node::ContentMode;
use texform_specs::specs::{AllowedMode, CommandKind};

use crate::api::{self, ParseOutput};
use crate::knowledge::{
    self, ArgSpecParseError, CommandMeta, EnvMeta, KnowledgeBase, PackageLoadError,
};

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

#[derive(Debug, Clone)]
pub struct ParseContext {
    pub(crate) kb: KnowledgeBase,
}

impl ParseContext {
    pub(crate) fn from_kb(kb: KnowledgeBase) -> Self {
        ParseContext { kb }
    }

    /// Build an empty context with no package specs loaded.
    pub fn empty() -> Self {
        Self::from_kb(KnowledgeBase::builder().build())
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

    /// Build runtime default context (all embedded packages except `test` and `dev`).
    pub fn runtime_default() -> Self {
        Self::from_packages(texform_specs::packages::runtime_default_packages())
    }

    /// Clone cached runtime default context.
    pub fn clone_runtime_default() -> Self {
        runtime_default_ctx().clone()
    }

    /// Build test default context (all embedded packages, including `test` and `dev`).
    pub fn test_default() -> Self {
        Self::from_packages(texform_specs::packages::test_default_packages())
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

    pub fn insert_command(&mut self, item: CommandItem) -> Result<(), ArgSpecParseError> {
        self.kb.insert_command(item)
    }

    pub fn remove_item(&mut self, item: impl Into<ContextItem>) -> bool {
        self.kb.remove_item(item.into())
    }

    pub fn insert_environment(&mut self, item: EnvironmentItem) -> Result<(), ArgSpecParseError> {
        self.kb.insert_environment(item)
    }

    pub fn insert_delimiter_control(&mut self, item: DelimiterControlItem) {
        self.kb.insert_delimiter_control(item);
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.kb.is_delimiter_control(name)
    }

    pub fn lookup_delimiter_control(&self, name: &str) -> Option<&'static str> {
        self.kb.lookup_delimiter_control(name)
    }

    pub fn parse(&self, src: &str, strict: bool) -> ParseOutput {
        api::parse_latex_with_kb(&self.kb, src, strict)
    }

    pub fn lookup_command(&self, name: &str) -> Option<&CommandMeta> {
        self.kb.lookup_command(name)
    }

    pub fn lookup_env(&self, name: &str) -> Option<&EnvMeta> {
        self.kb.lookup_env(name)
    }
}

fn runtime_default_ctx() -> &'static ParseContext {
    static DEFAULT: OnceLock<ParseContext> = OnceLock::new();
    DEFAULT.get_or_init(ParseContext::runtime_default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_can_insert_and_remove_delimiter_controls() {
        let mut ctx = ParseContext::empty();
        assert!(!ctx.is_delimiter_control("langle"));

        ctx.insert_delimiter_control(DelimiterControlItem::new("langle"));
        assert!(ctx.is_delimiter_control("langle"));
        assert_eq!(ctx.lookup_delimiter_control("langle"), Some("langle"));

        assert!(ctx.remove_item(DelimiterControlItem::new("langle")));
        assert!(!ctx.is_delimiter_control("langle"));
        assert_eq!(ctx.lookup_delimiter_control("langle"), None);
    }
}
