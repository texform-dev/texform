//! Knowledge base for LaTeX command metadata.
//!
//! The knowledge base is built from statically embedded package specs
//! provided by `texform-specs`.
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use texform_interface::syntax_node::ContentMode;

pub use texform_specs::specs::{
    AllowedMode, ArgForm, ArgSpec, CommandKind, CommandMeta, DelimiterToken, EnvMeta, ValueKind,
};

#[derive(Debug)]
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
    pub fn insert_character(&mut self, name: impl Into<String>, allowed_mode: AllowedMode) {
        let name = name.into();
        self.insert_or_override_command(name, CommandKind::Prefix, allowed_mode, vec![], vec![]);
    }

    pub fn insert_or_override_command(
        &mut self,
        name: impl Into<String>,
        kind: CommandKind,
        allowed_mode: AllowedMode,
        args: Vec<ArgSpec>,
        tags: Vec<String>,
    ) {
        let name: &'static str = Box::leak(name.into().into_boxed_str());
        let args: &'static [ArgSpec] = Box::leak(args.into_boxed_slice());
        let tags: Vec<&'static str> = tags
            .into_iter()
            .map(|tag| Box::leak(tag.into_boxed_str()) as &'static str)
            .collect();
        let tags: &'static [&'static str] = Box::leak(tags.into_boxed_slice());

        let meta = CommandMeta {
            name,
            kind,
            allowed_mode,
            args,
            tags,
        };

        match self.command_idx_by_name.get(name).copied() {
            Some(idx) => self.commands[idx] = meta,
            None => {
                let idx = self.commands.len();
                self.commands.push(meta);
                self.command_idx_by_name.insert(name, idx);
            }
        }
    }

    pub fn insert_or_override_env(
        &mut self,
        name: impl Into<String>,
        has_star_variant: bool,
        allowed_mode: AllowedMode,
        args: Vec<ArgSpec>,
        body_mode: ContentMode,
        tags: Vec<String>,
    ) {
        let name: &'static str = Box::leak(name.into().into_boxed_str());
        let args: &'static [ArgSpec] = Box::leak(args.into_boxed_slice());
        let tags: Vec<&'static str> = tags
            .into_iter()
            .map(|tag| Box::leak(tag.into_boxed_str()) as &'static str)
            .collect();
        let tags: &'static [&'static str] = Box::leak(tags.into_boxed_slice());

        let meta = EnvMeta {
            name,
            has_star_variant,
            allowed_mode,
            args,
            body_mode,
            tags,
        };

        match self.env_idx_by_name.get(name).copied() {
            Some(idx) => self.envs[idx] = meta,
            None => {
                let idx = self.envs.len();
                self.envs.push(meta);
                self.env_idx_by_name.insert(name, idx);
            }
        }
    }

    pub fn insert_delimiter_control(&mut self, name: impl Into<String>) {
        let name: &'static str = Box::leak(name.into().into_boxed_str());
        self.delimiter_controls.insert(name);
    }

    pub fn import_package(&mut self, specs: texform_specs::specs::PackageSpecs) {
        for character in specs.characters {
            self.insert_character(character.name, character.allowed_mode);
        }
        for cmd in specs.commands {
            self.insert_or_override_command(
                cmd.name,
                cmd.kind,
                cmd.allowed_mode,
                cmd.args,
                cmd.tags,
            );
        }
        for env in specs.environments {
            self.insert_or_override_env(
                env.name,
                env.has_star_variant,
                env.allowed_mode,
                env.args,
                env.body_mode,
                env.tags,
            );
        }
        for name in specs.delimiter_controls {
            self.insert_delimiter_control(name);
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

/// Initialize the global knowledge base.
///
/// - If `packages` is `None`, loads default packages.
///   - Runtime default: `base`
///   - Unit test default (`cfg(test)`): `base + dev`
/// - If `packages` is `Some`, loads `base` (if available) then the given packages in order.
///
/// This function may only be called once. Subsequent calls will panic.
pub fn init(packages: Option<&[&str]>) {
    KB.set(build_default_kb(packages))
        .unwrap_or_else(|_| panic!("knowledge base already initialized"));
}

/// Initialize the global knowledge base with runtime defaults (`base` only).
pub fn init_runtime_defaults() {
    init(Some(texform_specs::packages::RUNTIME_DEFAULT_PACKAGES));
}

/// Initialize the global knowledge base with test defaults (`base + dev`).
pub fn init_test_defaults() {
    init(Some(texform_specs::packages::TEST_DEFAULT_PACKAGES));
}

/// Initialize the global knowledge base from a pre-built builder.
///
/// This is primarily useful for integration tests that need inline command
/// metadata without modifying package YAML files.
///
/// This function may only be called once. Subsequent calls will panic.
pub fn init_with_builder(builder: KnowledgeBaseBuilder) {
    KB.set(builder.build())
        .unwrap_or_else(|_| panic!("knowledge base already initialized"));
}

static KB: OnceLock<KnowledgeBase> = OnceLock::new();

fn kb() -> &'static KnowledgeBase {
    KB.get_or_init(|| build_default_kb(None))
}

#[cfg(test)]
fn implicit_default_packages() -> &'static [&'static str] {
    texform_specs::packages::TEST_DEFAULT_PACKAGES
}

#[cfg(not(test))]
fn implicit_default_packages() -> &'static [&'static str] {
    texform_specs::packages::RUNTIME_DEFAULT_PACKAGES
}

fn ordered_package_names<'a>(requested: &[&'a str]) -> Vec<&'a str> {
    let mut out = vec![];
    // Loading order is intentional:
    // - `base` always loads first (if present)
    // - later packages can override earlier definitions by name
    if texform_specs::packages::get("base").is_some() {
        out.push("base");
    }
    for &name in requested {
        if name == "base" {
            continue;
        }
        out.push(name);
    }
    out
}

fn build_default_kb(packages: Option<&[&str]>) -> KnowledgeBase {
    let mut builder = KnowledgeBase::builder();
    let requested = match packages {
        Some(list) => list,
        None => implicit_default_packages(),
    };
    let to_load = ordered_package_names(requested);

    for &name in &to_load {
        let pkg = texform_specs::packages::get(name)
            .unwrap_or_else(|| panic!("unknown package: {}", name));
        builder.import_package((pkg.load)());
    }

    builder.build()
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
        let frac = lookup_command("frac").unwrap();
        assert_eq!(frac.name, "frac");
        assert_eq!(frac.kind, CommandKind::Prefix);
        assert_eq!(frac.args.len(), 2);

        let sqrt = lookup_command("sqrt").unwrap();
        assert_eq!(sqrt.args.len(), 2);
        assert!(!sqrt.args[0].required);
        assert!(sqrt.args[1].required);
        assert_eq!(
            sqrt.args[0].kind,
            ValueKind::Content {
                mode: ContentMode::Math
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
        let matrix = lookup_env("matrix").unwrap();
        assert_eq!(matrix.name, "matrix");
        assert_eq!(matrix.allowed_mode, AllowedMode::Math);
        assert_eq!(matrix.body_mode, ContentMode::Math);

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
        builder.insert_or_override_command(
            "foo",
            CommandKind::Prefix,
            AllowedMode::Math,
            vec![ArgSpec::mandatory(ContentMode::Math)],
            vec![],
        );

        builder.import_package(texform_specs::specs::PackageSpecs {
            characters: vec![],
            commands: vec![texform_specs::specs::CommandSpec {
                name: "foo".to_string(),
                kind: CommandKind::Prefix,
                allowed_mode: AllowedMode::Text,
                args: vec![],
                tags: vec![],
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
        builder.insert_or_override_env(
            "textenv",
            false,
            AllowedMode::Text,
            vec![],
            ContentMode::Text,
            vec![],
        );

        let kb = builder.build();
        let env = kb.lookup_env("textenv").unwrap();
        assert_eq!(env.body_mode, ContentMode::Text);
        assert_eq!(env.allowed_mode, AllowedMode::Text);
    }

    #[test]
    fn test_runtime_defaults_exclude_dev_entries() {
        let kb = build_default_kb(Some(texform_specs::packages::RUNTIME_DEFAULT_PACKAGES));
        assert!(kb.lookup_command("frac").is_some());
        assert!(kb.lookup_command("over").is_none());
        assert!(kb.lookup_delimiter_control("langle").is_none());
    }

    #[test]
    fn test_test_defaults_include_dev_entries() {
        let kb = build_default_kb(Some(texform_specs::packages::TEST_DEFAULT_PACKAGES));
        assert!(kb.lookup_command("frac").is_some());
        assert!(kb.lookup_command("over").is_some());
        assert!(kb.lookup_delimiter_control("langle").is_some());
    }
}
