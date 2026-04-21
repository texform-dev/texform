//! Shared spec types.
//!
//! This crate hosts:
//! - `PackageSpecs`: parsed YAML package specs (owned, merge-ready)
//! - builtin records generated at compile time
//! - active records assembled by the knowledge base at runtime

use serde::Deserialize;

pub use texform_argspec::ContentMode;
pub use texform_argspec::{
    ArgForm, ArgSpec, ArgSpecParseError, DelimiterToken, OwnedArgSpec, ParsedArgSpec, ValueKind,
    parse_arg_specs,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Prefix,
    Infix,
    Declarative,
}

impl CommandKind {
    pub const fn label(&self) -> &'static str {
        match self {
            CommandKind::Prefix => "prefix",
            CommandKind::Infix => "infix",
            CommandKind::Declarative => "declarative",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedMode {
    Math,
    Text,
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

    pub const fn union(self, other: Self) -> Self {
        match (self, other) {
            (AllowedMode::Both, _) | (_, AllowedMode::Both) => AllowedMode::Both,
            (AllowedMode::Math, AllowedMode::Math) => AllowedMode::Math,
            (AllowedMode::Text, AllowedMode::Text) => AllowedMode::Text,
            (AllowedMode::Math, AllowedMode::Text) | (AllowedMode::Text, AllowedMode::Math) => {
                AllowedMode::Both
            }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinCommandRecord {
    pub name: &'static str,
    pub kind: CommandKind,
    pub allowed_mode: AllowedMode,
    pub argspec: ParsedArgSpec,
    pub tags: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinEnvironmentRecord {
    pub name: &'static str,
    pub allowed_mode: AllowedMode,
    pub argspec: ParsedArgSpec,
    pub body_mode: ContentMode,
    pub tags: &'static [&'static str],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinCharacterAttributes {
    pub mathvariant: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinCharacterRecord {
    pub name: &'static str,
    pub allowed_mode: AllowedMode,
    pub unicode_value: &'static str,
    pub attributes: BuiltinCharacterAttributes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCommandRecord {
    pub name: &'static str,
    pub kind: CommandKind,
    pub allowed_mode: AllowedMode,
    pub argspec: ParsedArgSpec,
    pub tags: &'static [&'static str],
    pub from_packages: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEnvironmentRecord {
    pub name: &'static str,
    pub allowed_mode: AllowedMode,
    pub argspec: ParsedArgSpec,
    pub body_mode: ContentMode,
    pub tags: &'static [&'static str],
    pub from_packages: &'static [&'static str],
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CharacterAttributes {
    pub mathvariant: Option<String>,
}

impl From<BuiltinCharacterAttributes> for CharacterAttributes {
    fn from(value: BuiltinCharacterAttributes) -> Self {
        CharacterAttributes {
            mathvariant: value.mathvariant.map(ToString::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCharacterRecord {
    pub name: String,
    pub allowed_mode: AllowedMode,
    pub unicode_value: String,
    pub attributes: CharacterAttributes,
    pub package: String,
}

pub type CommandMeta = ActiveCommandRecord;
pub type EnvMeta = ActiveEnvironmentRecord;
pub type CharacterMeta = ActiveCharacterRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub kind: CommandKind,
    pub allowed_mode: AllowedMode,
    pub argspec: OwnedArgSpec,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentSpec {
    pub name: String,
    pub allowed_mode: AllowedMode,
    pub argspec: OwnedArgSpec,
    pub body_mode: ContentMode,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSpec {
    pub name: String,
    pub allowed_mode: AllowedMode,
    pub unicode_value: String,
    pub attributes: CharacterAttributes,
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
            characters: self.characters.into_iter().map(Into::into).collect(),
            commands: self.commands.into_iter().map(Into::into).collect(),
            environments: self.environments.into_iter().map(Into::into).collect(),
            delimiter_controls: self.delimiter_controls,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CharacterSpecYaml {
    name: String,
    allowed_mode: AllowedModeYaml,
    unicode_value: String,
    attributes: CharacterAttributesYaml,
}

impl From<CharacterSpecYaml> for CharacterSpec {
    fn from(value: CharacterSpecYaml) -> Self {
        CharacterSpec {
            name: value.name,
            allowed_mode: value.allowed_mode.into(),
            unicode_value: value.unicode_value,
            attributes: value.attributes.into(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct CharacterAttributesYaml {
    #[serde(default)]
    mathvariant: Option<String>,
}

impl From<CharacterAttributesYaml> for CharacterAttributes {
    fn from(value: CharacterAttributesYaml) -> Self {
        CharacterAttributes {
            mathvariant: value.mathvariant,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CommandSpecYaml {
    name: String,
    kind: CommandKindYaml,
    #[serde(default)]
    allowed_mode: AllowedModeYaml,
    #[serde(default)]
    argspec: String,
    #[serde(default)]
    tags: Vec<String>,
}

impl From<CommandSpecYaml> for CommandSpec {
    fn from(value: CommandSpecYaml) -> Self {
        let context = format!("command {}", value.name);
        let args =
            parse_arg_specs(value.argspec.as_str(), context.as_str()).unwrap_or_else(|error| {
                panic!("{error}");
            });

        CommandSpec {
            name: value.name,
            kind: value.kind.into(),
            allowed_mode: value.allowed_mode.into(),
            argspec: OwnedArgSpec {
                args,
                source: value.argspec,
            },
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
    allowed_mode: AllowedModeYaml,
    #[serde(default)]
    argspec: String,
    body_mode: ContentModeYaml,
    #[serde(default)]
    tags: Vec<String>,
}

impl From<EnvironmentSpecYaml> for EnvironmentSpec {
    fn from(value: EnvironmentSpecYaml) -> Self {
        let context = format!("environment {}", value.name);
        let args =
            parse_arg_specs(value.argspec.as_str(), context.as_str()).unwrap_or_else(|error| {
                panic!("{error}");
            });

        EnvironmentSpec {
            name: value.name,
            allowed_mode: value.allowed_mode.into(),
            argspec: OwnedArgSpec {
                args,
                source: value.argspec,
            },
            body_mode: value.body_mode.into(),
            tags: value.tags,
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
