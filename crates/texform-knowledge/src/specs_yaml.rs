#[derive(Debug, Default, serde::Deserialize)]
pub(crate) struct PackageSpecsYaml {
    #[serde(default)]
    pub(crate) characters: Vec<CharacterSpecYaml>,
    #[serde(default)]
    pub(crate) delimiters: Vec<DelimiterSpecYaml>,
    #[serde(default)]
    pub(crate) commands: Vec<CommandSpecYaml>,
    #[serde(default)]
    pub(crate) environments: Vec<EnvironmentSpecYaml>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CharacterSpecYaml {
    pub(crate) name: String,
    pub(crate) allowed_mode: AllowedModeYaml,
    pub(crate) unicode_value: String,
    pub(crate) attributes: CharacterAttributesYaml,
}

#[derive(Debug, Default, serde::Deserialize)]
pub(crate) struct CharacterAttributesYaml {
    #[serde(default)]
    pub(crate) mathvariant: Option<String>,
    #[serde(default)]
    pub(crate) tex_class: Option<String>,
    #[serde(default)]
    pub(crate) stretchy: Option<bool>,
    #[serde(default)]
    pub(crate) move_sup_sub: Option<bool>,
    #[serde(default)]
    pub(crate) large_op: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct DelimiterSpecYaml {
    pub(crate) name: String,
    pub(crate) is_control_sequence: bool,
    pub(crate) allowed_mode: AllowedModeYaml,
    pub(crate) unicode_value: String,
    pub(crate) attributes: CharacterAttributesYaml,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CommandSpecYaml {
    pub(crate) name: String,
    pub(crate) kind: CommandKindYaml,
    #[serde(default)]
    pub(crate) allowed_mode: AllowedModeYaml,
    #[serde(default)]
    pub(crate) argspec: String,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CommandKindYaml {
    Prefix,
    Infix,
    Declarative,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AllowedModeYaml {
    Math,
    Text,
    #[default]
    Both,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct EnvironmentSpecYaml {
    pub(crate) name: String,
    pub(crate) allowed_mode: AllowedModeYaml,
    #[serde(default)]
    pub(crate) argspec: String,
    pub(crate) body_mode: ContentModeYaml,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ContentModeYaml {
    Math,
    Text,
}
