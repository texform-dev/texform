//! Parse context for per-instance knowledge base isolation.

use std::sync::OnceLock;

use texform_interface::syntax_node::ContentMode;

use crate::api::{self, ParseOutput};
use crate::knowledge::{
    self, AllowedMode, ArgSpecParseError, CommandKind, CommandMeta, EnvMeta, KnowledgeBase,
    PackageLoadError,
};

#[derive(Debug, Clone)]
pub struct ParseContext {
    pub(crate) kb: KnowledgeBase,
}

impl ParseContext {
    pub(crate) fn from_kb(kb: KnowledgeBase) -> Self {
        ParseContext { kb }
    }

    /// Build a context from package names.
    ///
    /// The loader always prepends `base` when available.
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

    /// Build runtime default context (`base` package).
    pub fn runtime_default() -> Self {
        Self::from_packages(texform_specs::packages::RUNTIME_DEFAULT_PACKAGES)
    }

    /// Clone cached runtime default context.
    pub fn clone_runtime_default() -> Self {
        runtime_default_ctx().clone()
    }

    /// Build test default context (`base + dev` packages).
    pub fn test_default() -> Self {
        Self::from_packages(texform_specs::packages::TEST_DEFAULT_PACKAGES)
    }

    pub fn insert_command(
        &mut self,
        name: &str,
        kind: CommandKind,
        allowed_mode: AllowedMode,
        spec_string: &str,
        tags: &[&str],
    ) -> Result<(), ArgSpecParseError> {
        let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_string()).collect();
        self.kb
            .insert_command(name, kind, allowed_mode, spec_string, tags.as_slice())
    }

    pub fn remove_command(&mut self, name: &str) -> bool {
        self.kb.remove_command(name)
    }

    pub fn insert_env(
        &mut self,
        name: &str,
        allowed_mode: AllowedMode,
        spec_string: &str,
        body_mode: ContentMode,
        tags: &[&str],
    ) -> Result<(), ArgSpecParseError> {
        let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_string()).collect();
        self.kb
            .insert_env(name, allowed_mode, spec_string, body_mode, tags.as_slice())
    }

    pub fn remove_env(&mut self, name: &str) -> bool {
        self.kb.remove_env(name)
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
