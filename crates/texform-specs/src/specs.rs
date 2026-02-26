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

/// Allowed invocation mode for commands/environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedMode {
    /// Can only be invoked in math mode.
    Math,
    /// Can only be invoked in text mode.
    Text,
    /// Can be invoked in both math and text mode.
    Both,
}

impl AllowedMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            AllowedMode::Math => "math",
            AllowedMode::Text => "text",
            AllowedMode::Both => "both",
        }
    }

    pub const fn allows(self, mode: ContentMode) -> bool {
        match self {
            AllowedMode::Both => true,
            AllowedMode::Math => matches!(mode, ContentMode::Math),
            AllowedMode::Text => matches!(mode, ContentMode::Text),
        }
    }
}

impl std::fmt::Display for AllowedMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
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

    /// Allowed invocation mode.
    pub allowed_mode: AllowedMode,

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

    /// Allowed invocation mode.
    pub allowed_mode: AllowedMode,

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
    pub allowed_mode: AllowedMode,
    pub args: Vec<ArgSpec>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentSpec {
    pub name: String,
    pub has_star_variant: bool,
    pub allowed_mode: AllowedMode,
    pub args: Vec<ArgSpec>,
    pub body_mode: ContentMode,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSpec {
    pub name: String,
    pub allowed_mode: AllowedMode,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PackageSpecs {
    pub characters: Vec<CharacterSpec>,
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
    characters: Vec<CharacterSpecYaml>,
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
            characters: self.characters.into_iter().map(|c| c.into()).collect(),
            commands: self.commands.into_iter().map(|c| c.into()).collect(),
            environments: self.environments.into_iter().map(|e| e.into()).collect(),
            delimiter_controls: self.delimiter_controls,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CharacterSpecYaml {
    name: String,
    allowed_mode: AllowedModeYaml,
}

impl From<CharacterSpecYaml> for CharacterSpec {
    fn from(value: CharacterSpecYaml) -> Self {
        CharacterSpec {
            name: value.name,
            allowed_mode: value.allowed_mode.into(),
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
    allowed_mode: AllowedModeYaml,
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
            allowed_mode: value.allowed_mode.into(),
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

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum AllowedModeYaml {
    Math,
    Text,
    #[default]
    Both,
}

impl From<AllowedModeYaml> for AllowedMode {
    fn from(value: AllowedModeYaml) -> Self {
        match value {
            AllowedModeYaml::Math => AllowedMode::Math,
            AllowedModeYaml::Text => AllowedMode::Text,
            AllowedModeYaml::Both => AllowedMode::Both,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EnvironmentSpecYaml {
    name: String,
    #[serde(default)]
    has_star_variant: bool,
    allowed_mode: AllowedModeYaml,
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
            allowed_mode: value.allowed_mode.into(),
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
characters:
  - name: alpha
    allowed_mode: math
  - name: beta
    allowed_mode: text
commands:
  - name: frac
    kind: prefix
    allowed_mode: math
    tags: [discouraged]
    args:
      - required: true
        kind: math
      - required: true
        kind: delimiter
  - name: text
    kind: prefix
    allowed_mode: both
    args:
      - required: true
        kind: text
environments:
  - name: matrix
    allowed_mode: math
    body_mode: math
    tags: [matrix]
delimiter_controls: [langle]
"#;

        let specs = load_package_specs_from_str(yaml, "test");
        assert_eq!(specs.characters.len(), 2);
        assert_eq!(specs.characters[0].name, "alpha");
        assert_eq!(specs.characters[0].allowed_mode, AllowedMode::Math);
        assert_eq!(specs.characters[1].name, "beta");
        assert_eq!(specs.characters[1].allowed_mode, AllowedMode::Text);
        assert_eq!(specs.commands.len(), 2);
        assert_eq!(specs.commands[0].name, "frac");
        assert!(!specs.commands[0].has_star_variant);
        assert_eq!(specs.commands[0].allowed_mode, AllowedMode::Math);
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
        assert_eq!(specs.commands[1].allowed_mode, AllowedMode::Both);
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
        assert_eq!(specs.environments[0].allowed_mode, AllowedMode::Math);
        assert_eq!(specs.environments[0].tags, vec!["matrix"]);
        assert_eq!(specs.delimiter_controls, vec!["langle"]);
    }

    #[test]
    fn test_command_allowed_mode_defaults_to_both() {
        let yaml = r#"
commands:
  - name: foo
    kind: prefix
"#;

        let specs = load_package_specs_from_str(yaml, "default-allowed-mode");
        assert_eq!(specs.commands.len(), 1);
        assert_eq!(specs.commands[0].allowed_mode, AllowedMode::Both);
    }

    #[test]
    #[should_panic(expected = "missing field `allowed_mode`")]
    fn test_character_allowed_mode_is_required() {
        let yaml = r#"
characters:
  - name: alpha
"#;

        let _ = load_package_specs_from_str(yaml, "character-allowed-mode-required");
    }

    #[test]
    fn test_environment_body_mode_can_be_text() {
        let yaml = r#"
environments:
  - name: textenv
    allowed_mode: math
    body_mode: text
"#;

        let specs = load_package_specs_from_str(yaml, "test");
        assert_eq!(specs.environments.len(), 1);
        assert_eq!(specs.environments[0].name, "textenv");
        assert_eq!(specs.environments[0].body_mode, ContentMode::Text);
    }

    #[test]
    #[should_panic(expected = "missing field `allowed_mode`")]
    fn test_environment_allowed_mode_is_required() {
        let yaml = r#"
environments:
  - name: matrix
    body_mode: math
"#;

        let _ = load_package_specs_from_str(yaml, "environment-allowed-mode-required");
    }

    #[test]
    fn test_allowed_mode_helpers() {
        assert!(AllowedMode::Math.allows(ContentMode::Math));
        assert!(!AllowedMode::Math.allows(ContentMode::Text));
        assert!(AllowedMode::Text.allows(ContentMode::Text));
        assert!(!AllowedMode::Text.allows(ContentMode::Math));
        assert!(AllowedMode::Both.allows(ContentMode::Math));
        assert!(AllowedMode::Both.allows(ContentMode::Text));

        assert_eq!(AllowedMode::Math.to_string(), "math");
        assert_eq!(AllowedMode::Text.to_string(), "text");
        assert_eq!(AllowedMode::Both.to_string(), "both");
    }

    // Knowledge-base construction lives in texform-core.
}
