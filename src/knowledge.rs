//! Knowledge base for LaTeX command metadata
//!
//! This module defines command metadata used during parsing.
//!
//! The knowledge base is loaded at runtime from YAML files under
//! `resources/specs/{package}/*.y{a,}ml` and cached globally.
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use crate::specs;
use crate::syntax_node::{ArgumentKind, ContentMode};

/// Command type in knowledge base (determines AST node type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Prefix command → creates Command node
    /// Arguments follow the command
    Prefix,

    /// Infix command → creates InfixCommand node
    /// Left and right operands collected from context
    Infix,

    /// Declarative command → creates DeclarativeCommand node
    /// Scope collected from context (command to end of group)
    Declarative,
}

/// Argument specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgSpec {
    /// Argument type (Mandatory or Optional)
    pub kind: ArgumentKind,

    /// Content mode for this argument (Math or Text)
    pub mode: ContentMode,
}

impl ArgSpec {
    /// Create a mandatory argument spec
    pub const fn mandatory(mode: ContentMode) -> Self {
        ArgSpec {
            kind: ArgumentKind::Mandatory,
            mode,
        }
    }

    /// Create an optional argument spec
    pub const fn optional(mode: ContentMode) -> Self {
        ArgSpec {
            kind: ArgumentKind::Optional,
            mode,
        }
    }
}

/// Command metadata in knowledge base
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMeta {
    /// Command name (without backslash)
    pub name: &'static str,

    /// Command type (determines which AST node type to create)
    pub kind: CommandKind,

    /// Whether command supports starred variant (e.g., \section*)
    pub has_star_variant: bool,

    /// Argument specifications
    /// - For Prefix: all arguments
    /// - For Infix: command's own args (usually empty), left/right collected separately
    /// - For Declarative: command's own args, scope collected separately
    pub args: &'static [ArgSpec],
}

/// Environment metadata in knowledge base
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvMeta {
    /// Environment name (without \begin/\end)
    pub name: &'static str,

    /// Whether environment supports starred variant
    pub has_star_variant: bool,

    /// Argument specifications
    pub args: &'static [ArgSpec],

    /// Content mode for environment body
    pub body_mode: ContentMode,
}

// ============ Runtime Knowledge Base (YAML-backed) ============

/// Lookup command metadata by name
///
/// Returns None if command is not in the knowledge base.
pub fn lookup_command(name: &str) -> Option<&'static CommandMeta> {
    let kb = kb();
    kb.command_idx_by_name
        .get(name)
        .copied()
        .map(|idx| &kb.commands[idx])
}

/// Lookup environment metadata by name
///
/// Returns None if environment is not in the knowledge base.
pub fn lookup_env(name: &str) -> Option<&'static EnvMeta> {
    let kb = kb();
    kb.env_idx_by_name
        .get(name)
        .copied()
        .map(|idx| &kb.envs[idx])
}

/// Check if command is blacklisted
///
/// Returns Some(reason) if blacklisted, None otherwise.
pub fn is_blacklisted(name: &str) -> Option<&'static str> {
    kb().blacklist.get(name).copied()
}

/// Check if control sequence acts as a delimiter usable by \left...\right
pub fn is_delimiter_control(name: &str) -> bool {
    kb().delimiter_controls.contains(name)
}

/// Initialize the global knowledge base from a `resources/specs/`-like root directory.
///
/// This function may only be called once. Subsequent calls will panic.
pub fn init_from_specs_root(specs_root: impl AsRef<Path>) {
    let specs_root = specs_root.as_ref();
    let loaded = KnowledgeBase::load_from_specs_root(specs_root, None);
    KB.set(loaded)
        .unwrap_or_else(|_| panic!("knowledge base already initialized"));
}

/// Initialize the global knowledge base from a `resources/specs/`-like root directory,
/// loading only the given packages (plus `base`).
///
/// Package load order determines override precedence: later packages override earlier ones.
pub fn init_from_packages(specs_root: impl AsRef<Path>, packages: &[&str]) {
    let specs_root = specs_root.as_ref();
    let loaded = KnowledgeBase::load_from_specs_root(specs_root, Some(packages));
    KB.set(loaded)
        .unwrap_or_else(|_| panic!("knowledge base already initialized"));
}

static KB: OnceLock<KnowledgeBase> = OnceLock::new();

fn kb() -> &'static KnowledgeBase {
    KB.get_or_init(KnowledgeBase::load_default)
}

#[derive(Debug)]
struct KnowledgeBase {
    commands: Vec<CommandMeta>,
    command_idx_by_name: HashMap<&'static str, usize>,
    envs: Vec<EnvMeta>,
    env_idx_by_name: HashMap<&'static str, usize>,
    blacklist: HashMap<&'static str, &'static str>,
    delimiter_controls: HashSet<&'static str>,
}

impl KnowledgeBase {
    fn load_default() -> Self {
        let specs_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/specs");
        match fs::read_dir(&specs_root) {
            Ok(_) => Self::load_from_specs_root(&specs_root, None),
            Err(_) => Self::load_from_embedded(),
        }
    }

    fn load_from_specs_root(specs_root: &Path, packages: Option<&[&str]>) -> Self {
        let package_names = compute_package_load_order(specs_root, packages);
        let mut package_specs = vec![];
        for package in package_names {
            let files = load_package_specs_files(specs_root, &package);
            let specs = files.parse(&format!("package {}", package));
            package_specs.push(specs);
        }
        Self::load_from_external(package_specs)
    }

    fn load_from_embedded() -> Self {
        Self::load_from_external(embedded_package_specs(&["base"]))
    }

    fn load_from_external(packages: Vec<specs::PackageSpecs>) -> Self {
        build_kb_from_packages(packages)
    }
}

#[derive(Default)]
struct KnowledgeBuilder {
    commands: Vec<CommandMeta>,
    command_idx_by_name: HashMap<&'static str, usize>,
    envs: Vec<EnvMeta>,
    env_idx_by_name: HashMap<&'static str, usize>,
    blacklist: HashMap<&'static str, &'static str>,
    delimiter_controls: HashSet<&'static str>,
}

impl KnowledgeBuilder {
    fn insert_character(&mut self, name: String) {
        let cmd = specs::Command {
            name,
            kind: specs::CommandKind::Prefix,
            has_star_variant: false,
            args: vec![],
        };
        self.insert_or_override_command(cmd);
    }

    fn insert_or_override_command(&mut self, cmd: specs::Command) {
        let name: &'static str = Box::leak(cmd.name.into_boxed_str());

        let args: Vec<ArgSpec> = cmd
            .args
            .into_iter()
            .map(|a| ArgSpec {
                kind: to_argument_kind(a.kind),
                mode: to_content_mode(a.mode),
            })
            .collect();
        let args: &'static [ArgSpec] = Box::leak(args.into_boxed_slice());

        let meta = CommandMeta {
            name,
            kind: to_command_kind(cmd.kind),
            has_star_variant: cmd.has_star_variant,
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

    fn insert_or_override_env(&mut self, env: specs::Environment) {
        let name: &'static str = Box::leak(env.name.into_boxed_str());

        let args: Vec<ArgSpec> = env
            .args
            .into_iter()
            .map(|a| ArgSpec {
                kind: to_argument_kind(a.kind),
                mode: to_content_mode(a.mode),
            })
            .collect();
        let args: &'static [ArgSpec] = Box::leak(args.into_boxed_slice());

        let meta = EnvMeta {
            name,
            has_star_variant: env.has_star_variant,
            args,
            body_mode: to_content_mode(env.body_mode),
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

    fn insert_or_override_blacklist(&mut self, name: String, reason: String) {
        let name: &'static str = Box::leak(name.into_boxed_str());
        let reason: &'static str = Box::leak(reason.into_boxed_str());
        self.blacklist.insert(name, reason);
    }

    fn insert_delimiter_control(&mut self, name: String) {
        let leaked: &'static str = Box::leak(name.into_boxed_str());
        self.delimiter_controls.insert(leaked);
    }

    fn build(self) -> KnowledgeBase {
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

fn build_kb_from_packages(packages: Vec<specs::PackageSpecs>) -> KnowledgeBase {
    let mut builder = KnowledgeBuilder::default();
    for package in packages {
        for name in package.characters {
            builder.insert_character(name);
        }
        for cmd in package.commands {
            builder.insert_or_override_command(cmd);
        }
        for env in package.environments {
            builder.insert_or_override_env(env);
        }
        for (name, reason) in package.blacklist {
            builder.insert_or_override_blacklist(name, reason);
        }
        for name in package.delimiter_controls {
            builder.insert_delimiter_control(name);
        }
    }
    builder.build()
}

fn discover_all_packages(specs_root: &Path) -> Vec<String> {
    let mut packages = vec![];
    for entry in fs::read_dir(specs_root).unwrap_or_else(|e| {
        panic!(
            "failed to read specs root directory {}: {}",
            specs_root.display(),
            e
        )
    }) {
        let entry = entry.unwrap_or_else(|e| {
            panic!(
                "failed to read specs root directory entry under {}: {}",
                specs_root.display(),
                e
            )
        });
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| {
                panic!(
                    "invalid package directory name under {}",
                    specs_root.display()
                )
            });
        packages.push(name.to_string());
    }

    packages.sort();
    packages
}

fn compute_package_load_order(specs_root: &Path, packages: Option<&[&str]>) -> Vec<String> {
    match packages {
        None => {
            let discovered = discover_all_packages(specs_root);
            let mut out = vec![];
            if discovered.iter().any(|p| p == "base") {
                out.push("base".to_string());
            }
            for p in discovered {
                if p == "base" {
                    continue;
                }
                out.push(p);
            }
            out
        }
        Some(selected) => {
            let mut out = vec![];
            let mut seen = HashSet::<String>::new();

            let has_base_dir = specs_root.join("base").is_dir();
            if has_base_dir {
                out.push("base".to_string());
                seen.insert("base".to_string());
            }

            for &p in selected {
                let p = p.trim();
                if p.is_empty() {
                    continue;
                }
                if p == "base" && seen.contains("base") {
                    continue;
                }
                let p = p.to_string();
                if seen.insert(p.clone()) {
                    out.push(p);
                }
            }
            out
        }
    }
}

fn embedded_package_specs(packages: &[&str]) -> Vec<specs::PackageSpecs> {
    let mut out = vec![];
    for &package in packages {
        match package {
            "base" => {
                let files = specs::PackageSpecsYamlSources {
                    characters: Some(
                        include_str!("../resources/specs/base/characters.yaml").to_string(),
                    ),
                    commands: Some(
                        include_str!("../resources/specs/base/commands.yaml").to_string(),
                    ),
                    environments: Some(
                        include_str!("../resources/specs/base/environments.yaml").to_string(),
                    ),
                    delimiters: Some(
                        include_str!("../resources/specs/base/delimiters.yaml").to_string(),
                    ),
                    lists: Some(include_str!("../resources/specs/base/lists.yaml").to_string()),
                };
                out.push(files.parse("<embedded>/base"));
            }
            other => panic!("unknown embedded package: {}", other),
        }
    }
    out
}

fn to_command_kind(kind: specs::CommandKind) -> CommandKind {
    match kind {
        specs::CommandKind::Prefix => CommandKind::Prefix,
        specs::CommandKind::Infix => CommandKind::Infix,
        specs::CommandKind::Declarative => CommandKind::Declarative,
    }
}

fn to_argument_kind(kind: specs::ArgumentKind) -> ArgumentKind {
    match kind {
        specs::ArgumentKind::Mandatory => ArgumentKind::Mandatory,
        specs::ArgumentKind::Optional => ArgumentKind::Optional,
    }
}

fn to_content_mode(mode: specs::ContentMode) -> ContentMode {
    match mode {
        specs::ContentMode::Math => ContentMode::Math,
        specs::ContentMode::Text => ContentMode::Text,
    }
}

fn load_package_specs_files(specs_root: &Path, package: &str) -> specs::PackageSpecsYamlSources {
    let package_dir = specs_root.join(package);
    if !package_dir.is_dir() {
        panic!("package directory not found: {}", package_dir.display());
    }

    specs::PackageSpecsYamlSources {
        characters: read_optional_yaml(&package_dir, "characters"),
        commands: read_optional_yaml(&package_dir, "commands"),
        environments: read_optional_yaml(&package_dir, "environments"),
        delimiters: read_optional_yaml(&package_dir, "delimiters"),
        lists: read_optional_yaml(&package_dir, "lists"),
    }
}

fn read_optional_yaml(dir: &Path, stem: &str) -> Option<String> {
    let yaml = dir.join(format!("{stem}.yaml"));
    let yml = dir.join(format!("{stem}.yml"));
    let path = if yaml.exists() {
        yaml
    } else if yml.exists() {
        yml
    } else {
        return None;
    };
    Some(
        fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read specs file {}: {}", path.display(), e)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_command() {
        // Test prefix commands
        let frac = lookup_command("frac").unwrap();
        assert_eq!(frac.name, "frac");
        assert_eq!(frac.kind, CommandKind::Prefix);
        assert!(!frac.has_star_variant);
        assert_eq!(frac.args.len(), 2);

        let sqrt = lookup_command("sqrt").unwrap();
        assert_eq!(sqrt.name, "sqrt");
        assert_eq!(sqrt.args.len(), 2);
        assert_eq!(sqrt.args[0].kind, ArgumentKind::Optional);
        assert_eq!(sqrt.args[1].kind, ArgumentKind::Mandatory);

        // Test infix commands
        let over = lookup_command("over").unwrap();
        assert_eq!(over.kind, CommandKind::Infix);
        assert!(over.args.is_empty());

        // Test declarative commands
        let color = lookup_command("color").unwrap();
        assert_eq!(color.kind, CommandKind::Declarative);
        assert_eq!(color.args.len(), 1);

        // Test unknown command
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
}
