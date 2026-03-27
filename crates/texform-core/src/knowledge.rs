//! Knowledge base for LaTeX command metadata.
//!
//! The knowledge base is built from statically embedded package specs
//! provided by `texform-specs`.
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use texform_interface::syntax_node::ContentMode;

use crate::context::{
    CommandItem, ContextItem, DelimiterControlItem, EnvironmentItem, ParseContext,
};

pub use texform_specs::specs::{
    AllowedMode, ArgForm, ArgSpec, ArgSpecParseError, CharacterSpec, CommandKind, CommandMeta,
    CommandSpec, DelimiterToken, EnvMeta, EnvironmentSpec, PackageSpecs, ValueKind,
    load_package_specs_from_str,
};

const RUNTIME_PACKAGE_NAME: &str = "runtime";
const UNKNOWN_PACKAGE_NAME: &str = "unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLoadError {
    UnknownPackage { name: String },
}

impl std::fmt::Display for PackageLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageLoadError::UnknownPackage { name } => {
                write!(f, "unknown package: {name}")
            }
        }
    }
}

impl std::error::Error for PackageLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitError {
    PackageLoad(PackageLoadError),
    AlreadyInitialized,
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::PackageLoad(error) => write!(f, "{error}"),
            InitError::AlreadyInitialized => write!(f, "knowledge base already initialized"),
        }
    }
}

impl std::error::Error for InitError {}

#[derive(Debug, Clone)]
pub struct KnowledgeBase {
    commands: Vec<CommandMeta>,
    command_idx_by_name: HashMap<&'static str, usize>,
    envs: Vec<EnvMeta>,
    env_idx_by_name: HashMap<&'static str, usize>,
    delimiter_controls: HashSet<&'static str>,
}

impl KnowledgeBase {
    pub fn builder() -> KnowledgeBaseBuilder {
        KnowledgeBaseBuilder::default()
    }

    pub fn lookup_command(&self, name: &str) -> Option<&CommandMeta> {
        self.command_idx_by_name
            .get(name)
            .copied()
            .map(|idx| &self.commands[idx])
    }

    pub fn lookup_env(&self, name: &str) -> Option<&EnvMeta> {
        self.env_idx_by_name
            .get(name)
            .copied()
            .map(|idx| &self.envs[idx])
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.delimiter_controls.contains(name)
    }

    pub fn lookup_delimiter_control(&self, name: &str) -> Option<&'static str> {
        self.delimiter_controls.get(name).copied()
    }

    pub fn insert_item(&mut self, item: impl Into<ContextItem>) -> Result<(), ArgSpecParseError> {
        match item.into() {
            ContextItem::Command(item) => self.insert_command(item),
            ContextItem::Environment(item) => self.insert_environment(item),
            ContextItem::DelimiterControl(item) => {
                self.insert_delimiter_control(item);
                Ok(())
            }
        }
    }

    pub fn insert_command(&mut self, item: CommandItem) -> Result<(), ArgSpecParseError> {
        let meta = command_item_into_meta(item, RUNTIME_PACKAGE_NAME.to_string())?;
        self.upsert_command_meta(meta);
        Ok(())
    }

    pub fn remove_item(&mut self, item: impl Into<ContextItem>) -> bool {
        match item.into() {
            ContextItem::Command(item) => self
                .command_idx_by_name
                .remove(item.name.as_str())
                .is_some(),
            ContextItem::Environment(item) => {
                self.env_idx_by_name.remove(item.name.as_str()).is_some()
            }
            ContextItem::DelimiterControl(item) => {
                self.delimiter_controls.remove(item.name.as_str())
            }
        }
    }

    pub fn insert_environment(&mut self, item: EnvironmentItem) -> Result<(), ArgSpecParseError> {
        let meta = environment_item_into_meta(item, RUNTIME_PACKAGE_NAME.to_string())?;
        self.upsert_env_meta(meta);
        Ok(())
    }

    pub fn insert_delimiter_control(&mut self, item: DelimiterControlItem) {
        let name = item.name;
        if self.delimiter_controls.contains(name.as_str()) {
            return;
        }
        let name: &'static str = Box::leak(name.into_boxed_str());
        self.delimiter_controls.insert(name);
    }

    fn upsert_command_meta(&mut self, meta: CommandMeta) {
        let idx = self.commands.len();
        let name = meta.name;
        self.commands.push(meta);
        self.command_idx_by_name.insert(name, idx);
    }

    fn upsert_env_meta(&mut self, meta: EnvMeta) {
        let idx = self.envs.len();
        let name = meta.name;
        self.envs.push(meta);
        self.env_idx_by_name.insert(name, idx);
    }
}

#[derive(Debug, Default)]
pub struct KnowledgeBaseBuilder {
    commands: Vec<CommandMeta>,
    command_idx_by_name: HashMap<&'static str, usize>,
    envs: Vec<EnvMeta>,
    env_idx_by_name: HashMap<&'static str, usize>,
    delimiter_controls: HashSet<&'static str>,
}

impl KnowledgeBaseBuilder {
    pub fn insert_character(&mut self, character: CharacterSpec) {
        self.insert_character_with_package(character, UNKNOWN_PACKAGE_NAME);
    }

    fn insert_character_with_package(&mut self, character: CharacterSpec, package: &str) {
        self.insert_or_override_command_with_package(
            CommandSpec {
                name: character.name,
                kind: CommandKind::Prefix,
                allowed_mode: character.allowed_mode,
                args: vec![],
                tags: vec![],
                spec_string: String::new(),
            },
            package,
        );
    }

    pub fn insert_or_override_command(&mut self, spec: CommandSpec) {
        self.insert_or_override_command_with_package(spec, UNKNOWN_PACKAGE_NAME);
    }

    fn insert_or_override_command_with_package(&mut self, spec: CommandSpec, package: &str) {
        let meta = command_spec_into_meta(spec, package.to_string());
        let idx = self.commands.len();
        let name = meta.name;
        self.commands.push(meta);
        self.command_idx_by_name.insert(name, idx);
    }

    pub fn insert_or_override_environment(&mut self, spec: EnvironmentSpec) {
        self.insert_or_override_environment_with_package(spec, UNKNOWN_PACKAGE_NAME);
    }

    fn insert_or_override_environment_with_package(
        &mut self,
        spec: EnvironmentSpec,
        package: &str,
    ) {
        let meta = environment_spec_into_meta(spec, package.to_string());
        let idx = self.envs.len();
        let name = meta.name;
        self.envs.push(meta);
        self.env_idx_by_name.insert(name, idx);
    }

    pub fn insert_delimiter_control(&mut self, item: DelimiterControlItem) {
        let name: &'static str = Box::leak(item.name.into_boxed_str());
        self.delimiter_controls.insert(name);
    }

    pub fn import_package(&mut self, specs: PackageSpecs) {
        self.import_package_with_name(UNKNOWN_PACKAGE_NAME, specs);
    }

    pub fn import_package_with_name(&mut self, package: &str, specs: PackageSpecs) {
        for character in specs.characters {
            self.insert_character_with_package(character, package);
        }
        for cmd in specs.commands {
            self.insert_or_override_command_with_package(cmd, package);
        }
        for env in specs.environments {
            self.insert_or_override_environment_with_package(env, package);
        }
        for name in specs.delimiter_controls {
            self.insert_delimiter_control(DelimiterControlItem::new(name));
        }
    }

    pub fn build(self) -> KnowledgeBase {
        KnowledgeBase {
            commands: self.commands,
            command_idx_by_name: self.command_idx_by_name,
            envs: self.envs,
            env_idx_by_name: self.env_idx_by_name,
            delimiter_controls: self.delimiter_controls,
        }
    }
}

fn make_command_meta(
    name: String,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    args: Vec<ArgSpec>,
    tags: Vec<String>,
    spec_string: String,
    package: String,
) -> CommandMeta {
    CommandMeta {
        name: leak_string(name),
        kind,
        allowed_mode,
        args: leak_arg_specs(args),
        tags: leak_tags(tags),
        spec_string: leak_string(spec_string),
        package: leak_string(package),
    }
}

fn command_item_into_meta(
    item: CommandItem,
    package: String,
) -> Result<CommandMeta, ArgSpecParseError> {
    let context = format!("command {}", item.name);
    let args = texform_specs::specs::parse_arg_specs(item.spec.as_str(), context.as_str())?;
    Ok(make_command_meta(
        item.name,
        item.kind,
        item.allowed_mode,
        args,
        item.tags,
        item.spec,
        package,
    ))
}

fn make_env_meta(
    name: String,
    allowed_mode: AllowedMode,
    args: Vec<ArgSpec>,
    body_mode: ContentMode,
    tags: Vec<String>,
    spec_string: String,
    package: String,
) -> EnvMeta {
    EnvMeta {
        name: leak_string(name),
        allowed_mode,
        args: leak_arg_specs(args),
        body_mode,
        tags: leak_tags(tags),
        spec_string: leak_string(spec_string),
        package: leak_string(package),
    }
}

fn environment_item_into_meta(
    item: EnvironmentItem,
    package: String,
) -> Result<EnvMeta, ArgSpecParseError> {
    let context = format!("environment {}", item.name);
    let args = texform_specs::specs::parse_arg_specs(item.spec.as_str(), context.as_str())?;
    Ok(make_env_meta(
        item.name,
        item.allowed_mode,
        args,
        item.body_mode,
        item.tags,
        item.spec,
        package,
    ))
}

fn command_spec_into_meta(spec: CommandSpec, package: String) -> CommandMeta {
    make_command_meta(
        spec.name,
        spec.kind,
        spec.allowed_mode,
        spec.args,
        spec.tags,
        spec.spec_string,
        package,
    )
}

fn environment_spec_into_meta(spec: EnvironmentSpec, package: String) -> EnvMeta {
    make_env_meta(
        spec.name,
        spec.allowed_mode,
        spec.args,
        spec.body_mode,
        spec.tags,
        spec.spec_string,
        package,
    )
}

fn leak_string(value: impl Into<String>) -> &'static str {
    Box::leak(value.into().into_boxed_str())
}

fn leak_arg_specs(args: Vec<ArgSpec>) -> &'static [ArgSpec] {
    Box::leak(args.into_boxed_slice())
}

fn leak_tags(tags: Vec<String>) -> &'static [&'static str] {
    let tags: Vec<&'static str> = tags
        .into_iter()
        .map(|tag| Box::leak(tag.into_boxed_str()) as &'static str)
        .collect();
    Box::leak(tags.into_boxed_slice())
}

/// Initialize the global parse context.
///
/// - If `packages` is `None`, loads default packages.
///   - Runtime default: all embedded packages except `test` and `dev`
///   - Unit test default (`cfg(test)`): all embedded packages
/// - If `packages` is `Some`, loads exactly the given packages in order.
///
/// This function may only be called once. Subsequent calls will panic.
pub fn init(packages: Option<&[&str]>) {
    try_init(packages).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible variant of [`init`].
///
/// Returns [`InitError::AlreadyInitialized`] when called more than once.
pub fn try_init(packages: Option<&[&str]>) -> Result<(), InitError> {
    let requested = match packages {
        Some(list) => list,
        None => implicit_default_packages(),
    };
    let ctx = ParseContext::try_from_packages(requested).map_err(InitError::PackageLoad)?;
    DEFAULT_CTX
        .set(ctx)
        .map_err(|_| InitError::AlreadyInitialized)
}

/// Initialize the global parse context with runtime defaults
/// (all embedded packages except `test` and `dev`).
pub fn init_runtime_defaults() {
    init(Some(texform_specs::packages::runtime_default_packages()));
}

/// Initialize the global parse context with test defaults (all embedded packages).
pub fn init_test_defaults() {
    init(Some(texform_specs::packages::test_default_packages()));
}

/// Build a [`KnowledgeBaseBuilder`] pre-loaded with the test default packages
/// (`test` and `dev`).
///
/// Integration tests can call this to get a starting builder and inject
/// additional commands inline, without depending on extra YAML resource files.
pub fn test_kb_builder() -> KnowledgeBaseBuilder {
    let mut builder = KnowledgeBase::builder();
    for &name in &["test", "dev"] {
        if let Some(pkg) = texform_specs::packages::get(name) {
            builder.import_package_with_name(name, (pkg.load)());
        }
    }
    builder
}

/// Initialize the global parse context from a pre-built builder.
///
/// This is primarily useful for integration tests that need inline command
/// metadata without modifying package YAML files.
///
/// This function may only be called once. Subsequent calls will panic.
pub fn init_with_builder(builder: KnowledgeBaseBuilder) {
    try_init_with_builder(builder).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible variant of [`init_with_builder`].
///
/// Returns [`InitError::AlreadyInitialized`] when called more than once.
pub fn try_init_with_builder(builder: KnowledgeBaseBuilder) -> Result<(), InitError> {
    DEFAULT_CTX
        .set(ParseContext::from_kb(builder.build()))
        .map_err(|_| InitError::AlreadyInitialized)
}

static DEFAULT_CTX: OnceLock<ParseContext> = OnceLock::new();

pub(crate) fn default_ctx() -> &'static ParseContext {
    DEFAULT_CTX.get_or_init(|| ParseContext::from_packages(implicit_default_packages()))
}

pub(crate) fn kb() -> &'static KnowledgeBase {
    &default_ctx().kb
}

#[cfg(test)]
fn implicit_default_packages() -> &'static [&'static str] {
    texform_specs::packages::test_default_packages()
}

#[cfg(not(test))]
fn implicit_default_packages() -> &'static [&'static str] {
    texform_specs::packages::runtime_default_packages()
}

fn ordered_package_names<'a>(requested: &[&'a str]) -> Vec<&'a str> {
    let mut out = vec![];
    for &name in requested {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn import_package_names(
    builder: &mut KnowledgeBaseBuilder,
    requested: &[&str],
) -> Result<(), PackageLoadError> {
    for &name in requested {
        let pkg =
            texform_specs::packages::get(name).ok_or_else(|| PackageLoadError::UnknownPackage {
                name: name.to_string(),
            })?;
        builder.import_package_with_name(name, (pkg.load)());
    }
    Ok(())
}

pub(crate) fn build_kb_from_packages(requested: &[&str]) -> KnowledgeBase {
    try_build_kb_from_packages(requested).unwrap_or_else(|error| panic!("{error}"))
}

pub(crate) fn try_build_kb_from_packages(
    requested: &[&str],
) -> Result<KnowledgeBase, PackageLoadError> {
    let mut builder = KnowledgeBase::builder();
    let to_load = ordered_package_names(requested);
    import_package_names(&mut builder, to_load.as_slice())?;

    Ok(builder.build())
}

pub(crate) fn try_build_kb_from_exact_packages(
    requested: &[&str],
) -> Result<KnowledgeBase, PackageLoadError> {
    let mut builder = KnowledgeBase::builder();
    import_package_names(&mut builder, requested)?;

    Ok(builder.build())
}

#[cfg(test)]
fn build_default_kb(packages: Option<&[&str]>) -> KnowledgeBase {
    let requested = match packages {
        Some(list) => list,
        None => implicit_default_packages(),
    };
    build_kb_from_packages(requested)
}

/// Lookup command metadata by name.
///
/// Returns None if command is not in the knowledge base.
pub fn lookup_command(name: &str) -> Option<&'static CommandMeta> {
    kb().lookup_command(name)
}

/// Lookup environment metadata by name.
///
/// Returns None if environment is not in the knowledge base.
pub fn lookup_env(name: &str) -> Option<&'static EnvMeta> {
    kb().lookup_env(name)
}

/// Check if control sequence acts as a delimiter usable by \left...\right.
pub fn is_delimiter_control(name: &str) -> bool {
    kb().is_delimiter_control(name)
}

/// Lookup the canonical delimiter control name.
///
/// This returns the interned `&'static str` stored in the knowledge base.
/// Parser code should prefer this over allocating/leaking new strings.
pub fn lookup_delimiter_control(name: &str) -> Option<&'static str> {
    kb().lookup_delimiter_control(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use texform_interface::syntax_node::ContentMode;

    #[test]
    fn test_lookup_command() {
        let text = lookup_command("text").unwrap();
        assert_eq!(text.name, "text");
        assert_eq!(text.kind, CommandKind::Prefix);
        assert_eq!(text.args.len(), 1);
        assert_eq!(
            text.args[0].kind,
            ValueKind::Content {
                mode: ContentMode::Text
            }
        );

        let over = lookup_command("over").unwrap();
        assert_eq!(over.kind, CommandKind::Infix);
        assert!(over.args.is_empty());

        let color = lookup_command("color").unwrap();
        assert_eq!(color.kind, CommandKind::Declarative);
        assert_eq!(color.args.len(), 1);

        assert!(lookup_command("unknown").is_none());
    }

    #[test]
    fn test_lookup_env() {
        let align = lookup_env("align").unwrap();
        assert_eq!(align.name, "align");
        assert_eq!(align.allowed_mode, AllowedMode::Math);
        assert_eq!(align.body_mode, ContentMode::Math);

        assert!(lookup_env("unknown").is_none());
    }

    #[test]
    fn test_arg_spec_helpers() {
        let mandatory_math = ArgSpec::mandatory(ContentMode::Math);
        assert!(mandatory_math.required);
        assert_eq!(
            mandatory_math.kind,
            ValueKind::Content {
                mode: ContentMode::Math
            }
        );

        let optional_text = ArgSpec::optional(ContentMode::Text);
        assert!(!optional_text.required);
        assert_eq!(
            optional_text.kind,
            ValueKind::Content {
                mode: ContentMode::Text
            }
        );
    }

    #[test]
    fn test_delimiter_controls() {
        assert!(is_delimiter_control("langle"));
        assert!(is_delimiter_control("rvert"));
        assert!(!is_delimiter_control("notadelim"));
    }

    #[test]
    fn test_builder_import_overrides_by_order() {
        let mut builder = KnowledgeBase::builder();
        builder.insert_or_override_command(CommandSpec {
            name: "foo".to_string(),
            kind: CommandKind::Prefix,
            allowed_mode: AllowedMode::Math,
            args: vec![ArgSpec::mandatory(ContentMode::Math)],
            tags: vec![],
            spec_string: String::new(),
        });

        builder.import_package(texform_specs::specs::PackageSpecs {
            characters: vec![],
            commands: vec![texform_specs::specs::CommandSpec {
                name: "foo".to_string(),
                kind: CommandKind::Prefix,
                allowed_mode: AllowedMode::Text,
                args: vec![],
                tags: vec![],
                spec_string: "".to_string(),
            }],
            environments: vec![],
            delimiter_controls: vec![],
        });

        let kb = builder.build();
        let foo = kb.lookup_command("foo").unwrap();
        assert_eq!(foo.allowed_mode, AllowedMode::Text);
        assert!(foo.args.is_empty());
    }

    #[test]
    fn test_character_import_preserves_allowed_mode() {
        let mut builder = KnowledgeBase::builder();
        builder.import_package(texform_specs::specs::PackageSpecs {
            characters: vec![texform_specs::specs::CharacterSpec {
                name: "alpha".to_string(),
                allowed_mode: AllowedMode::Text,
            }],
            commands: vec![],
            environments: vec![],
            delimiter_controls: vec![],
        });

        let kb = builder.build();
        let alpha = kb.lookup_command("alpha").unwrap();
        assert_eq!(alpha.kind, CommandKind::Prefix);
        assert_eq!(alpha.allowed_mode, AllowedMode::Text);
        assert!(alpha.args.is_empty());
    }

    #[test]
    fn test_insert_env_accepts_text_body_mode() {
        let mut builder = KnowledgeBase::builder();
        builder.insert_or_override_environment(EnvironmentSpec {
            name: "textenv".to_string(),
            allowed_mode: AllowedMode::Text,
            args: vec![],
            body_mode: ContentMode::Text,
            tags: vec![],
            spec_string: String::new(),
        });

        let kb = builder.build();
        let env = kb.lookup_env("textenv").unwrap();
        assert_eq!(env.body_mode, ContentMode::Text);
        assert_eq!(env.allowed_mode, AllowedMode::Text);
    }

    #[test]
    fn test_runtime_defaults_exclude_test_and_dev_entries() {
        let kb = build_default_kb(Some(texform_specs::packages::runtime_default_packages()));
        assert!(kb.lookup_command("text").is_none());
        assert!(kb.lookup_command("over").is_none());
        assert!(kb.lookup_delimiter_control("langle").is_none());
    }

    #[test]
    fn test_test_defaults_include_dev_entries() {
        let kb = build_default_kb(Some(texform_specs::packages::test_default_packages()));
        assert!(kb.lookup_command("text").is_some());
        assert!(kb.lookup_command("over").is_some());
        assert!(kb.lookup_delimiter_control("langle").is_some());
    }
}
