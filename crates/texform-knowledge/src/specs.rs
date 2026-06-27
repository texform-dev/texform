//! Shared spec types.
//!
//! This crate hosts:
//! - `PackageSpecs`: parsed YAML package specs (owned, merge-ready)
//! - builtin records generated at compile time
//! - active records assembled by the knowledge base at runtime

#[path = "specs_yaml.rs"]
mod specs_yaml;

use specs_yaml::{
    AllowedModeYaml, CharacterAttributesYaml, CharacterSpecYaml, CommandKindYaml, CommandSpecYaml,
    ContentModeYaml, DelimiterSpecYaml, EnvironmentSpecYaml, PackageSpecsYaml,
};

pub use texform_argspec::ContentMode;
pub use texform_argspec::{
    ArgForm, ArgSpec, ArgSpecParseError, DelimiterToken, OwnedArgSpec, ParsedArgSpec, ValueKind,
    parse_arg_specs,
};

/// How a command consumes its surroundings during parsing.
///
/// The kind determines where the command's arguments come from: from the tokens
/// that follow it, from the material on both sides of it, or from the scope it
/// opens until the enclosing group ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// An ordinary command that takes its arguments from the tokens following it,
    /// per its argspec (e.g. `\frac`, `\sqrt`).
    Prefix,
    /// An infix operator that splits the surrounding material into numerator and
    /// denominator, like `\over` or `\atop`.
    Infix,
    /// A declaration that changes style for the rest of the current group rather
    /// than taking explicit arguments, like the font switch `\bf`.
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

/// The content mode(s) in which a knowledge-base name is valid.
///
/// Constrains whether a command, environment, character, or delimiter may appear
/// in math mode, text mode, or either; the parser uses it to reject a name used
/// in a mode it does not support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedMode {
    /// Valid only in math mode.
    Math,
    /// Valid only in text mode.
    Text,
    /// Valid in both math and text mode.
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
    pub tex_class: Option<&'static str>,
    pub stretchy: Option<bool>,
    pub move_sup_sub: Option<bool>,
    pub large_op: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinCharacterRecord {
    pub name: &'static str,
    pub allowed_mode: AllowedMode,
    pub unicode_value: &'static str,
    pub attributes: BuiltinCharacterAttributes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinDelimiterRecord {
    pub name: &'static str,
    pub is_control_sequence: bool,
    pub allowed_mode: AllowedMode,
    pub unicode_value: &'static str,
    pub attributes: BuiltinCharacterAttributes,
}

/// A command entry in the active knowledge base the parser consults.
///
/// "Active" means it is part of the resolved set the parser sees for the current
/// package configuration, merged from builtin and package specs. It records what
/// a command name means and what argument shape it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCommandRecord {
    /// The command name, without the leading backslash (e.g. `frac`).
    pub name: &'static str,
    /// How the command consumes its surroundings (prefix, infix, or declarative).
    pub kind: CommandKind,
    /// The content mode(s) in which the command is valid.
    pub allowed_mode: AllowedMode,
    /// The command's argument signature, in the xparse-style spec language.
    pub argspec: ParsedArgSpec,
    /// Free-form classification tags carried from the source spec.
    pub tags: &'static [&'static str],
    /// The packages that contribute this record, in resolution order.
    pub from_packages: &'static [&'static str],
}

/// An environment entry in the active knowledge base the parser consults.
///
/// "Active" means it is part of the resolved set the parser sees for the current
/// package configuration. It records the argument shape an environment takes
/// after its `\begin{...}` and the content mode its body is parsed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEnvironmentRecord {
    /// The environment name, as written between the braces of `\begin{...}`.
    pub name: &'static str,
    /// The content mode(s) in which the environment may be opened.
    pub allowed_mode: AllowedMode,
    /// The argument signature taken after `\begin{name}`, in the xparse-style spec language.
    pub argspec: ParsedArgSpec,
    /// The content mode the environment body is parsed in.
    pub body_mode: ContentMode,
    /// Free-form classification tags carried from the source spec.
    pub tags: &'static [&'static str],
    /// The packages that contribute this record, in resolution order.
    pub from_packages: &'static [&'static str],
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CharacterAttributes {
    pub mathvariant: Option<String>,
    pub tex_class: Option<String>,
    pub stretchy: Option<bool>,
    pub move_sup_sub: Option<bool>,
    pub large_op: Option<bool>,
}

impl From<BuiltinCharacterAttributes> for CharacterAttributes {
    fn from(value: BuiltinCharacterAttributes) -> Self {
        CharacterAttributes {
            mathvariant: value.mathvariant.map(ToString::to_string),
            tex_class: value.tex_class.map(ToString::to_string),
            stretchy: value.stretchy,
            move_sup_sub: value.move_sup_sub,
            large_op: value.large_op,
        }
    }
}

/// A character command entry in the active knowledge base the parser consults.
///
/// Maps a named character command (such as a Greek letter or symbol) to the
/// Unicode it stands for, together with the rendering attributes that describe
/// how it behaves in a formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCharacterRecord {
    /// The character command name, without the leading backslash (e.g. `alpha`).
    pub name: String,
    /// The content mode(s) in which the character is valid.
    pub allowed_mode: AllowedMode,
    /// The Unicode character this command represents.
    pub unicode_value: String,
    /// Rendering attributes (math variant, TeX class, stretchiness, ...) for the character.
    pub attributes: CharacterAttributes,
    /// The package this record comes from.
    pub package: String,
}

/// A delimiter entry in the active knowledge base the parser consults.
///
/// Describes a name usable as a delimiter (for example after `\left`/`\right`),
/// the Unicode it stands for, and its rendering attributes. A delimiter may be a
/// control sequence (like `\langle`) or a single literal character (like `(`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveDelimiterRecord {
    /// The delimiter name: a control-sequence name without the backslash, or the literal character.
    pub name: &'static str,
    /// Whether the name is a control sequence (`true`) rather than a literal character (`false`).
    pub is_control_sequence: bool,
    /// The content mode(s) in which the delimiter is valid.
    pub allowed_mode: AllowedMode,
    /// The Unicode character this delimiter represents.
    pub unicode_value: String,
    /// Rendering attributes (TeX class, stretchiness, ...) for the delimiter.
    pub attributes: CharacterAttributes,
    /// The package this record comes from.
    pub package: String,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelimiterSpec {
    pub name: String,
    pub is_control_sequence: bool,
    pub allowed_mode: AllowedMode,
    pub unicode_value: String,
    pub attributes: CharacterAttributes,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PackageSpecs {
    pub characters: Vec<CharacterSpec>,
    pub delimiters: Vec<DelimiterSpec>,
    pub commands: Vec<CommandSpec>,
    pub environments: Vec<EnvironmentSpec>,
}

pub fn load_package_specs_from_str(yaml: &str, context: &str) -> PackageSpecs {
    let parsed: PackageSpecsYaml = serde_yaml::from_str(yaml)
        .unwrap_or_else(|e| panic!("failed to parse package specs ({context}): {e}"));
    parsed.into_specs()
}

impl PackageSpecsYaml {
    fn into_specs(self) -> PackageSpecs {
        PackageSpecs {
            characters: self.characters.into_iter().map(Into::into).collect(),
            delimiters: self.delimiters.into_iter().map(Into::into).collect(),
            commands: self.commands.into_iter().map(Into::into).collect(),
            environments: self.environments.into_iter().map(Into::into).collect(),
        }
    }
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

impl From<CharacterAttributesYaml> for CharacterAttributes {
    fn from(value: CharacterAttributesYaml) -> Self {
        CharacterAttributes {
            mathvariant: value.mathvariant,
            tex_class: value.tex_class,
            stretchy: value.stretchy,
            move_sup_sub: value.move_sup_sub,
            large_op: value.large_op,
        }
    }
}

impl From<DelimiterSpecYaml> for DelimiterSpec {
    fn from(value: DelimiterSpecYaml) -> Self {
        DelimiterSpec {
            name: value.name,
            is_control_sequence: value.is_control_sequence,
            allowed_mode: value.allowed_mode.into(),
            unicode_value: value.unicode_value,
            attributes: value.attributes.into(),
        }
    }
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

impl From<CommandKindYaml> for CommandKind {
    fn from(value: CommandKindYaml) -> Self {
        match value {
            CommandKindYaml::Prefix => CommandKind::Prefix,
            CommandKindYaml::Infix => CommandKind::Infix,
            CommandKindYaml::Declarative => CommandKind::Declarative,
        }
    }
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

impl From<ContentModeYaml> for ContentMode {
    fn from(value: ContentModeYaml) -> Self {
        match value {
            ContentModeYaml::Math => ContentMode::Math,
            ContentModeYaml::Text => ContentMode::Text,
        }
    }
}
