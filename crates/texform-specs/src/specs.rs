//! Shared spec types.
//!
//! This crate hosts:
//! - `PackageSpecs`: parsed YAML package specs (owned, merge-ready)
//! - Knowledge metadata types (`CommandMeta`, `EnvMeta`, ...)
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use serde::Deserialize;
use texform_interface::syntax_node::ContentMode;

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

impl CommandKind {
    /// Return a human-readable label for error messages.
    pub const fn label(&self) -> &'static str {
        match self {
            CommandKind::Prefix => "prefix",
            CommandKind::Infix => "infix",
            CommandKind::Declarative => "declarative",
        }
    }
}

/// Argument value kind (parsing strategy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// Content argument parsed recursively (math/text mode)
    Content { mode: ContentMode },
    /// Single delimiter token (including '.' for empty)
    Delimiter,
    /// Dimension / length value (e.g., 1em, -2pt)
    Dimension,
    /// Integer value (e.g., 2, -10)
    Integer,
    /// Key=Value list (validated format, stored as raw string)
    KeyVal,
    /// Array column template argument
    Column,
}

impl ValueKind {
    pub const fn is_content(&self) -> bool {
        matches!(self, ValueKind::Content { .. })
    }

    pub const fn is_delimiter(&self) -> bool {
        matches!(self, ValueKind::Delimiter)
    }

    pub const fn is_dimension(&self) -> bool {
        matches!(self, ValueKind::Dimension)
    }

    pub const fn is_integer(&self) -> bool {
        matches!(self, ValueKind::Integer)
    }

    pub const fn is_keyval(&self) -> bool {
        matches!(self, ValueKind::KeyVal)
    }

    pub const fn is_column(&self) -> bool {
        matches!(self, ValueKind::Column)
    }

    pub const fn content_mode(&self) -> Option<ContentMode> {
        match self {
            ValueKind::Content { mode } => Some(*mode),
            _ => None,
        }
    }
}

/// Argument specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgSpec {
    /// Whether the argument is required (true) or optional (false)
    pub required: bool,

    /// Argument value kind (parsing strategy)
    pub kind: ValueKind,
}

impl ArgSpec {
    pub const fn new(required: bool, kind: ValueKind) -> Self {
        ArgSpec { required, kind }
    }

    /// Create a mandatory content argument spec
    pub const fn mandatory(mode: ContentMode) -> Self {
        ArgSpec {
            required: true,
            kind: ValueKind::Content { mode },
        }
    }

    /// Create an optional content argument spec
    pub const fn optional(mode: ContentMode) -> Self {
        ArgSpec {
            required: false,
            kind: ValueKind::Content { mode },
        }
    }

    pub const fn is_required(&self) -> bool {
        self.required
    }

    pub const fn is_optional(&self) -> bool {
        !self.required
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

    /// Metadata tags (kebab-case)
    pub tags: &'static [&'static str],
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

    /// Metadata tags (kebab-case)
    pub tags: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub kind: CommandKind,
    pub has_star_variant: bool,
    pub args: Vec<ArgSpec>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentSpec {
    pub name: String,
    pub has_star_variant: bool,
    pub args: Vec<ArgSpec>,
    pub body_mode: ContentMode,
    pub tags: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PackageSpecs {
    pub characters: Vec<String>,
    pub commands: Vec<CommandSpec>,
    pub environments: Vec<EnvironmentSpec>,
    pub delimiter_controls: Vec<String>,
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
}

impl PackageSpecsYaml {
    fn into_specs(self) -> PackageSpecs {
        PackageSpecs {
            characters: self.characters,
            commands: self.commands.into_iter().map(|c| c.into()).collect(),
            environments: self.environments.into_iter().map(|e| e.into()).collect(),
            delimiter_controls: self.delimiter_controls,
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
    #[serde(default)]
    tags: Vec<String>,
}

impl From<CommandSpecYaml> for CommandSpec {
    fn from(value: CommandSpecYaml) -> Self {
        CommandSpec {
            name: value.name,
            kind: value.kind.into(),
            has_star_variant: value.has_star_variant,
            args: value.args.into_iter().map(|a| a.into()).collect(),
            tags: value.tags,
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
    #[serde(default)]
    tags: Vec<String>,
}

impl From<EnvironmentSpecYaml> for EnvironmentSpec {
    fn from(value: EnvironmentSpecYaml) -> Self {
        EnvironmentSpec {
            name: value.name,
            has_star_variant: value.has_star_variant,
            args: value.args.into_iter().map(|a| a.into()).collect(),
            body_mode: value.body_mode.into(),
            tags: value.tags,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ArgSpecYaml {
    required: bool,
    kind: ValueKindYaml,
}

impl From<ArgSpecYaml> for ArgSpec {
    fn from(value: ArgSpecYaml) -> Self {
        ArgSpec {
            required: value.required,
            kind: value.kind.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ValueKindYaml {
    Math,
    Text,
    Delimiter,
    Dimension,
    Integer,
    KeyVal,
    Column,
}

impl From<ValueKindYaml> for ValueKind {
    fn from(value: ValueKindYaml) -> Self {
        match value {
            ValueKindYaml::Math => ValueKind::Content {
                mode: ContentMode::Math,
            },
            ValueKindYaml::Text => ValueKind::Content {
                mode: ContentMode::Text,
            },
            ValueKindYaml::Delimiter => ValueKind::Delimiter,
            ValueKindYaml::Dimension => ValueKind::Dimension,
            ValueKindYaml::Integer => ValueKind::Integer,
            ValueKindYaml::KeyVal => ValueKind::KeyVal,
            ValueKindYaml::Column => ValueKind::Column,
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
    tags: [discouraged]
    args:
      - required: true
        kind: math
      - required: true
        kind: delimiter
  - name: text
    kind: prefix
    args:
      - required: true
        kind: text
environments:
  - name: matrix
    body_mode: math
    tags: [matrix]
delimiter_controls: [langle]
"#;

        let specs = load_package_specs_from_str(yaml, "test");
        assert_eq!(specs.characters, vec!["alpha", "beta"]);
        assert_eq!(specs.commands.len(), 2);
        assert_eq!(specs.commands[0].name, "frac");
        assert!(!specs.commands[0].has_star_variant);
        assert_eq!(specs.commands[0].args.len(), 2);
        assert_eq!(specs.commands[0].tags, vec!["discouraged"]);
        assert_eq!(specs.commands[0].args[0].required, true);
        assert_eq!(
            specs.commands[0].args[0].kind,
            ValueKind::Content {
                mode: ContentMode::Math
            }
        );
        assert_eq!(specs.commands[0].args[1].kind, ValueKind::Delimiter);
        assert_eq!(specs.commands[1].name, "text");
        assert_eq!(specs.commands[1].args.len(), 1);
        assert!(specs.commands[1].tags.is_empty());
        assert_eq!(
            specs.commands[1].args[0].kind,
            ValueKind::Content {
                mode: ContentMode::Text
            }
        );
        assert_eq!(specs.environments.len(), 1);
        assert_eq!(specs.environments[0].name, "matrix");
        assert_eq!(specs.environments[0].tags, vec!["matrix"]);
        assert_eq!(specs.delimiter_controls, vec!["langle"]);
    }

    // Knowledge-base construction lives in texform-core.
}
