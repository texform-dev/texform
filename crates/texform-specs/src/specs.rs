//! Shared spec types.
//!
//! This crate hosts:
//! - `PackageSpecs`: parsed YAML package specs (owned, merge-ready)
//! - Knowledge metadata types (`CommandMeta`, `EnvMeta`, ...)
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use std::collections::HashMap;

use serde::Deserialize;
use texform_interface::syntax_node::{ArgumentKind, ContentMode};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub kind: CommandKind,
    pub has_star_variant: bool,
    pub args: Vec<ArgSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentSpec {
    pub name: String,
    pub has_star_variant: bool,
    pub args: Vec<ArgSpec>,
    pub body_mode: ContentMode,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PackageSpecs {
    pub characters: Vec<String>,
    pub commands: Vec<CommandSpec>,
    pub environments: Vec<EnvironmentSpec>,
    pub delimiter_controls: Vec<String>,
    pub blacklist: HashMap<String, String>,
}

pub fn load_package_specs_from_str(yaml: &str, context: &str) -> PackageSpecs {
    let parsed: PackageSpecsYaml = serde_yaml::from_str(yaml)
        .unwrap_or_else(|e| panic!("failed to parse package specs ({context}): {e}"));
    parsed.into_specs()
}

#[derive(Debug, Default, Deserialize)]
struct PackageSpecsYaml {
    #[serde(default)]
    characters: Vec<String>,
    #[serde(default)]
    commands: Vec<CommandSpecYaml>,
    #[serde(default)]
    environments: Vec<EnvironmentSpecYaml>,
    #[serde(default)]
    delimiter_controls: Vec<String>,
    #[serde(default)]
    blacklist: HashMap<String, String>,
}

impl PackageSpecsYaml {
    fn into_specs(self) -> PackageSpecs {
        PackageSpecs {
            characters: self.characters,
            commands: self.commands.into_iter().map(|c| c.into()).collect(),
            environments: self.environments.into_iter().map(|e| e.into()).collect(),
            delimiter_controls: self.delimiter_controls,
            blacklist: self.blacklist,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CommandSpecYaml {
    name: String,
    kind: CommandKindYaml,
    #[serde(default)]
    has_star_variant: bool,
    #[serde(default)]
    args: Vec<ArgSpecYaml>,
}

impl From<CommandSpecYaml> for CommandSpec {
    fn from(value: CommandSpecYaml) -> Self {
        CommandSpec {
            name: value.name,
            kind: value.kind.into(),
            has_star_variant: value.has_star_variant,
            args: value.args.into_iter().map(|a| a.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CommandKindYaml {
    Prefix,
    Infix,
    Declarative,
}

impl From<CommandKindYaml> for CommandKind {
    fn from(value: CommandKindYaml) -> Self {
        match value {
            CommandKindYaml::Prefix => CommandKind::Prefix,
            CommandKindYaml::Infix => CommandKind::Infix,
            CommandKindYaml::Declarative => CommandKind::Declarative,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EnvironmentSpecYaml {
    name: String,
    #[serde(default)]
    has_star_variant: bool,
    #[serde(default)]
    args: Vec<ArgSpecYaml>,
    body_mode: ContentModeYaml,
}

impl From<EnvironmentSpecYaml> for EnvironmentSpec {
    fn from(value: EnvironmentSpecYaml) -> Self {
        EnvironmentSpec {
            name: value.name,
            has_star_variant: value.has_star_variant,
            args: value.args.into_iter().map(|a| a.into()).collect(),
            body_mode: value.body_mode.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ArgSpecYaml {
    kind: ArgumentKindYaml,
    mode: ContentModeYaml,
}

impl From<ArgSpecYaml> for ArgSpec {
    fn from(value: ArgSpecYaml) -> Self {
        ArgSpec {
            kind: value.kind.into(),
            mode: value.mode.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ArgumentKindYaml {
    Mandatory,
    Optional,
}

impl From<ArgumentKindYaml> for ArgumentKind {
    fn from(value: ArgumentKindYaml) -> Self {
        match value {
            ArgumentKindYaml::Mandatory => ArgumentKind::Mandatory,
            ArgumentKindYaml::Optional => ArgumentKind::Optional,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ContentModeYaml {
    Math,
    Text,
}

impl From<ContentModeYaml> for ContentMode {
    fn from(value: ContentModeYaml) -> Self {
        match value {
            ContentModeYaml::Math => ContentMode::Math,
            ContentModeYaml::Text => ContentMode::Text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_package_specs_from_str() {
        let yaml = r#"
characters: [alpha, beta]
commands:
  - name: frac
    kind: prefix
    args:
      - kind: mandatory
        mode: math
environments:
  - name: matrix
    body_mode: math
delimiter_controls: [langle]
blacklist:
  ifnum: nope
"#;

        let specs = load_package_specs_from_str(yaml, "test");
        assert_eq!(specs.characters, vec!["alpha", "beta"]);
        assert_eq!(specs.commands.len(), 1);
        assert_eq!(specs.commands[0].name, "frac");
        assert!(!specs.commands[0].has_star_variant);
        assert_eq!(specs.commands[0].args.len(), 1);
        assert_eq!(specs.environments.len(), 1);
        assert_eq!(specs.environments[0].name, "matrix");
        assert_eq!(specs.delimiter_controls, vec!["langle"]);
        assert_eq!(specs.blacklist.get("ifnum").unwrap(), "nope");
    }

    // Knowledge-base construction lives in texform-core.
}
