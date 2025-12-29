//! Knowledge base for LaTeX command metadata.
//!
//! The knowledge base is built from statically embedded package specs
//! provided by `texform-specs`.
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

pub use texform_specs::specs::{ArgSpec, CommandKind, CommandMeta, EnvMeta};

#[derive(Debug)]
pub struct KnowledgeBase {
    commands: Vec<CommandMeta>,
    command_idx_by_name: HashMap<&'static str, usize>,
    envs: Vec<EnvMeta>,
    env_idx_by_name: HashMap<&'static str, usize>,
    blacklist: HashMap<&'static str, &'static str>,
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

    pub fn is_blacklisted(&self, name: &str) -> Option<&'static str> {
        self.blacklist.get(name).copied()
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
    blacklist: HashMap<&'static str, &'static str>,
    delimiter_controls: HashSet<&'static str>,
}

impl KnowledgeBaseBuilder {
    pub fn insert_character(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.insert_or_override_command(name, CommandKind::Prefix, false, vec![]);
    }

    pub fn insert_or_override_command(
        &mut self,
        name: impl Into<String>,
        kind: CommandKind,
        has_star_variant: bool,
        args: Vec<ArgSpec>,
    ) {
        let name: &'static str = Box::leak(name.into().into_boxed_str());
        let args: &'static [ArgSpec] = Box::leak(args.into_boxed_slice());

        let meta = CommandMeta {
            name,
            kind,
            has_star_variant,
            args,
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
        args: Vec<ArgSpec>,
        body_mode: texform_interface::syntax_node::ContentMode,
    ) {
        let name: &'static str = Box::leak(name.into().into_boxed_str());
        let args: &'static [ArgSpec] = Box::leak(args.into_boxed_slice());

        let meta = EnvMeta {
            name,
            has_star_variant,
            args,
            body_mode,
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

    pub fn insert_or_override_blacklist(
        &mut self,
        name: impl Into<String>,
        reason: impl Into<String>,
    ) {
        let name: &'static str = Box::leak(name.into().into_boxed_str());
        let reason: &'static str = Box::leak(reason.into().into_boxed_str());
        self.blacklist.insert(name, reason);
    }

    pub fn insert_delimiter_control(&mut self, name: impl Into<String>) {
        let name: &'static str = Box::leak(name.into().into_boxed_str());
        self.delimiter_controls.insert(name);
    }

    pub fn import_package(&mut self, specs: texform_specs::specs::PackageSpecs) {
        for name in specs.characters {
            self.insert_character(name);
        }
        for cmd in specs.commands {
            self.insert_or_override_command(cmd.name, cmd.kind, cmd.has_star_variant, cmd.args);
        }
        for env in specs.environments {
            self.insert_or_override_env(env.name, env.has_star_variant, env.args, env.body_mode);
        }
        for name in specs.delimiter_controls {
            self.insert_delimiter_control(name);
        }
        for (name, reason) in specs.blacklist {
            self.insert_or_override_blacklist(name, reason);
        }
    }

    pub fn build(self) -> KnowledgeBase {
        KnowledgeBase {
            commands: self.commands,
            command_idx_by_name: self.command_idx_by_name,
            envs: self.envs,
            env_idx_by_name: self.env_idx_by_name,
            blacklist: self.blacklist,
            delimiter_controls: self.delimiter_controls,
        }
    }
}

/// Initialize the global knowledge base.
///
/// - If `packages` is `None`, loads all packages.
/// - If `packages` is `Some`, loads `base` (if available) then the given packages in order.
///
/// This function may only be called once. Subsequent calls will panic.
pub fn init(packages: Option<&[&str]>) {
    KB.set(build_default_kb(packages))
        .unwrap_or_else(|_| panic!("knowledge base already initialized"));
}

static KB: OnceLock<KnowledgeBase> = OnceLock::new();

fn kb() -> &'static KnowledgeBase {
    KB.get_or_init(|| build_default_kb(None))
}

fn build_default_kb(packages: Option<&[&str]>) -> KnowledgeBase {
    let mut builder = KnowledgeBase::builder();

    let to_load = match packages {
        None => texform_specs::packages::ALL_PACKAGES
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>(),
        Some(list) => {
            let mut out = vec![];
            // Loading order is intentional:
            // - `base` always loads first (if present)
            // - later packages can override earlier definitions by name
            if texform_specs::packages::get("base").is_some() {
                out.push("base");
            }
            for &name in list {
                if name == "base" {
                    continue;
                }
                out.push(name);
            }
            out
        }
    };

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

/// Check if command is blacklisted.
///
/// Returns Some(reason) if blacklisted, None otherwise.
pub fn is_blacklisted(name: &str) -> Option<&'static str> {
    kb().is_blacklisted(name)
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
    use texform_interface::syntax_node::ArgumentKind;
    use texform_interface::syntax_node::ContentMode;

    #[test]
    fn test_lookup_command() {
        let frac = lookup_command("frac").unwrap();
        assert_eq!(frac.name, "frac");
        assert_eq!(frac.kind, CommandKind::Prefix);
        assert_eq!(frac.args.len(), 2);

        let sqrt = lookup_command("sqrt").unwrap();
        assert_eq!(sqrt.args.len(), 2);
        assert_eq!(sqrt.args[0].kind, ArgumentKind::Optional);
        assert_eq!(sqrt.args[1].kind, ArgumentKind::Mandatory);

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
        assert_eq!(matrix.body_mode, ContentMode::Math);

        assert!(lookup_env("unknown").is_none());
    }

    #[test]
    fn test_blacklist() {
        assert_eq!(is_blacklisted("ifnum"), Some("Control flow not supported"));
        assert_eq!(
            is_blacklisted("csname"),
            Some("Dynamic command names not supported")
        );
        assert_eq!(is_blacklisted("frac"), None);
    }

    #[test]
    fn test_arg_spec_helpers() {
        let mandatory_math = ArgSpec::mandatory(ContentMode::Math);
        assert_eq!(mandatory_math.kind, ArgumentKind::Mandatory);
        assert_eq!(mandatory_math.mode, ContentMode::Math);

        let optional_text = ArgSpec::optional(ContentMode::Text);
        assert_eq!(optional_text.kind, ArgumentKind::Optional);
        assert_eq!(optional_text.mode, ContentMode::Text);
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
            false,
            vec![ArgSpec::mandatory(ContentMode::Math)],
        );

        builder.import_package(texform_specs::specs::PackageSpecs {
            characters: vec![],
            commands: vec![texform_specs::specs::CommandSpec {
                name: "foo".to_string(),
                kind: CommandKind::Prefix,
                has_star_variant: true,
                args: vec![],
            }],
            environments: vec![],
            delimiter_controls: vec![],
            blacklist: std::collections::HashMap::new(),
        });

        let kb = builder.build();
        let foo = kb.lookup_command("foo").unwrap();
        assert!(foo.has_star_variant);
        assert!(foo.args.is_empty());
    }
}
