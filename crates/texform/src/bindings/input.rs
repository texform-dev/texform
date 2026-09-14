use crate::{
    FinalizeAstConfig, FlattenGroupsConfig, LowerAttributesConfig, NormalizeConfig, ParseConfig,
    RewriteConfig, TransformConfig,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(
    default,
    rename_all = "camelCase",
    deny_unknown_fields,
    expecting = "an object"
)]
pub struct ParseConfigInput {
    pub reject_unknown: Option<bool>,
    pub abort_on_error: Option<bool>,
    pub max_group_depth: Option<usize>,
}

impl ParseConfigInput {
    pub fn into_config(self, mut base: ParseConfig) -> ParseConfig {
        if let Some(reject_unknown) = self.reject_unknown {
            base.reject_unknown = reject_unknown;
        }
        if let Some(abort_on_error) = self.abort_on_error {
            base.abort_on_error = abort_on_error;
        }
        if let Some(max_group_depth) = self.max_group_depth {
            base.max_group_depth = max_group_depth;
        }
        base
    }

    pub fn from_config(config: ParseConfig) -> Self {
        Self {
            reject_unknown: Some(config.reject_unknown),
            abort_on_error: Some(config.abort_on_error),
            max_group_depth: Some(config.max_group_depth),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields, expecting = "an object")]
pub struct LowerAttributesConfigInput {
    pub enabled: Option<bool>,
}

impl LowerAttributesConfigInput {
    pub fn into_config(self, mut base: LowerAttributesConfig) -> LowerAttributesConfig {
        if let Some(enabled) = self.enabled {
            base.enabled = enabled;
        }
        base
    }

    pub fn from_config(config: LowerAttributesConfig) -> Self {
        Self {
            enabled: Some(config.enabled),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(
    default,
    rename_all = "camelCase",
    deny_unknown_fields,
    expecting = "an object"
)]
pub struct RewriteConfigInput {
    pub enabled: Option<bool>,
    pub max_iterations: Option<usize>,
}

impl RewriteConfigInput {
    pub fn into_config(self, mut base: RewriteConfig) -> RewriteConfig {
        if let Some(enabled) = self.enabled {
            base.enabled = enabled;
        }
        if let Some(max_iterations) = self.max_iterations {
            base.max_iterations = max_iterations;
        }
        base
    }

    pub fn from_config(config: RewriteConfig) -> Self {
        Self {
            enabled: Some(config.enabled),
            max_iterations: Some(config.max_iterations),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(
    default,
    rename_all = "camelCase",
    deny_unknown_fields,
    expecting = "an object"
)]
pub struct FlattenGroupsConfigInput {
    pub enabled: Option<bool>,
    pub preserve_group_containing_declarative_command: Option<bool>,
    pub preserve_group_in_script_base_slot: Option<bool>,
    pub preserve_group_inside_env_body: Option<bool>,
    pub preserve_group_containing_infix: Option<bool>,
    pub preserve_group_adjacent_to_command_like: Option<bool>,
    pub preserve_group_as_argument_of_command: Option<bool>,
    pub preserve_group_after_scripted_command_like: Option<bool>,
    pub preserve_empty_group: Option<bool>,
    pub preserve_group_with_lone_atom_spacing_char: Option<bool>,
    pub preserve_group_starting_with_atom_spacing_char: Option<bool>,
    pub preserve_group_containing_delimited_pair: Option<bool>,
}

impl FlattenGroupsConfigInput {
    pub fn into_config(self, mut base: FlattenGroupsConfig) -> FlattenGroupsConfig {
        if let Some(value) = self.enabled {
            base.enabled = value;
        }
        if let Some(value) = self.preserve_group_containing_declarative_command {
            base.preserve_group_containing_declarative_command = value;
        }
        if let Some(value) = self.preserve_group_in_script_base_slot {
            base.preserve_group_in_script_base_slot = value;
        }
        if let Some(value) = self.preserve_group_inside_env_body {
            base.preserve_group_inside_env_body = value;
        }
        if let Some(value) = self.preserve_group_containing_infix {
            base.preserve_group_containing_infix = value;
        }
        if let Some(value) = self.preserve_group_adjacent_to_command_like {
            base.preserve_group_adjacent_to_command_like = value;
        }
        if let Some(value) = self.preserve_group_as_argument_of_command {
            base.preserve_group_as_argument_of_command = value;
        }
        if let Some(value) = self.preserve_group_after_scripted_command_like {
            base.preserve_group_after_scripted_command_like = value;
        }
        if let Some(value) = self.preserve_empty_group {
            base.preserve_empty_group = value;
        }
        if let Some(value) = self.preserve_group_with_lone_atom_spacing_char {
            base.preserve_group_with_lone_atom_spacing_char = value;
        }
        if let Some(value) = self.preserve_group_starting_with_atom_spacing_char {
            base.preserve_group_starting_with_atom_spacing_char = value;
        }
        if let Some(value) = self.preserve_group_containing_delimited_pair {
            base.preserve_group_containing_delimited_pair = value;
        }
        base
    }

    pub fn from_config(config: FlattenGroupsConfig) -> Self {
        Self {
            enabled: Some(config.enabled),
            preserve_group_containing_declarative_command: Some(
                config.preserve_group_containing_declarative_command,
            ),
            preserve_group_in_script_base_slot: Some(config.preserve_group_in_script_base_slot),
            preserve_group_inside_env_body: Some(config.preserve_group_inside_env_body),
            preserve_group_containing_infix: Some(config.preserve_group_containing_infix),
            preserve_group_adjacent_to_command_like: Some(
                config.preserve_group_adjacent_to_command_like,
            ),
            preserve_group_as_argument_of_command: Some(
                config.preserve_group_as_argument_of_command,
            ),
            preserve_group_after_scripted_command_like: Some(
                config.preserve_group_after_scripted_command_like,
            ),
            preserve_empty_group: Some(config.preserve_empty_group),
            preserve_group_with_lone_atom_spacing_char: Some(
                config.preserve_group_with_lone_atom_spacing_char,
            ),
            preserve_group_starting_with_atom_spacing_char: Some(
                config.preserve_group_starting_with_atom_spacing_char,
            ),
            preserve_group_containing_delimited_pair: Some(
                config.preserve_group_containing_delimited_pair,
            ),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields, expecting = "an object")]
pub struct FinalizeAstConfigInput {
    pub enabled: Option<bool>,
}

impl FinalizeAstConfigInput {
    pub fn into_config(self, mut base: FinalizeAstConfig) -> FinalizeAstConfig {
        if let Some(enabled) = self.enabled {
            base.enabled = enabled;
        }
        base
    }

    pub fn from_config(config: FinalizeAstConfig) -> Self {
        Self {
            enabled: Some(config.enabled),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(
    default,
    rename_all = "camelCase",
    deny_unknown_fields,
    expecting = "an object"
)]
pub struct TransformConfigInput {
    pub lower_attributes: Option<LowerAttributesConfigInput>,
    pub rewrite: Option<RewriteConfigInput>,
    pub finalize_ast: Option<FinalizeAstConfigInput>,
    pub flatten_groups: Option<FlattenGroupsConfigInput>,
}

impl TransformConfigInput {
    pub fn into_config(self, base: TransformConfig) -> TransformConfig {
        TransformConfig {
            lower_attributes: self
                .lower_attributes
                .unwrap_or_default()
                .into_config(base.lower_attributes),
            rewrite: self.rewrite.unwrap_or_default().into_config(base.rewrite),
            finalize_ast: self
                .finalize_ast
                .unwrap_or_default()
                .into_config(base.finalize_ast),
            flatten_groups: self
                .flatten_groups
                .unwrap_or_default()
                .into_config(base.flatten_groups),
        }
    }

    pub fn from_config(config: TransformConfig) -> Self {
        Self {
            lower_attributes: Some(LowerAttributesConfigInput::from_config(
                config.lower_attributes,
            )),
            rewrite: Some(RewriteConfigInput::from_config(config.rewrite)),
            finalize_ast: Some(FinalizeAstConfigInput::from_config(config.finalize_ast)),
            flatten_groups: Some(FlattenGroupsConfigInput::from_config(config.flatten_groups)),
        }
    }
}

/// Flat union of parse and transform overlays: the `normalize` shape in both bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(
    default,
    rename_all = "camelCase",
    deny_unknown_fields,
    expecting = "an object"
)]
pub struct NormalizeConfigInput {
    pub reject_unknown: Option<bool>,
    pub abort_on_error: Option<bool>,
    pub max_group_depth: Option<usize>,
    pub lower_attributes: Option<LowerAttributesConfigInput>,
    pub rewrite: Option<RewriteConfigInput>,
    pub finalize_ast: Option<FinalizeAstConfigInput>,
    pub flatten_groups: Option<FlattenGroupsConfigInput>,
}

impl NormalizeConfigInput {
    pub fn into_config(self, base: NormalizeConfig) -> NormalizeConfig {
        let parse = ParseConfigInput {
            reject_unknown: self.reject_unknown,
            abort_on_error: self.abort_on_error,
            max_group_depth: self.max_group_depth,
        };
        let transform = TransformConfigInput {
            lower_attributes: self.lower_attributes,
            rewrite: self.rewrite,
            finalize_ast: self.finalize_ast,
            flatten_groups: self.flatten_groups,
        };
        NormalizeConfig {
            parse: parse.into_config(base.parse),
            transform: transform.into_config(base.transform),
        }
    }

    pub fn from_config(config: NormalizeConfig) -> Self {
        let parse = ParseConfigInput::from_config(config.parse);
        let transform = TransformConfigInput::from_config(config.transform);
        Self {
            reject_unknown: parse.reject_unknown,
            abort_on_error: parse.abort_on_error,
            max_group_depth: parse.max_group_depth,
            lower_attributes: transform.lower_attributes,
            rewrite: transform.rewrite,
            finalize_ast: transform.finalize_ast,
            flatten_groups: transform.flatten_groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Profile;

    fn authoring() -> TransformConfig {
        Profile::Authoring.default_transform_config()
    }

    fn corpus() -> TransformConfig {
        Profile::Corpus.default_transform_config()
    }

    fn disabled_rewrite() -> RewriteConfig {
        RewriteConfig {
            enabled: false,
            max_iterations: 7,
        }
    }

    #[test]
    fn parse_config_input_default_leaves_base_unchanged() {
        let base = ParseConfig::STRICT;
        assert_eq!(ParseConfigInput::default().into_config(base.clone()), base);
    }

    #[test]
    fn parse_config_input_overrides_default_fields() {
        let input = ParseConfigInput {
            reject_unknown: Some(true),
            abort_on_error: None,
            max_group_depth: Some(7),
        };

        let config = input.into_config(ParseConfig::default());

        assert!(config.reject_unknown);
        assert!(!config.abort_on_error);
        assert_eq!(config.max_group_depth, 7);
    }

    #[test]
    fn parse_config_input_from_config_into_config_is_identity() {
        let cfg = ParseConfig::STRICT;
        assert_eq!(
            ParseConfigInput::from_config(cfg.clone()).into_config(ParseConfig::LENIENT),
            cfg
        );
    }

    #[test]
    fn lower_attributes_input_default_leaves_base_unchanged() {
        let base = LowerAttributesConfig::DISABLED;
        assert_eq!(
            LowerAttributesConfigInput::default().into_config(base),
            base
        );
    }

    #[test]
    fn lower_attributes_input_partial_overlay() {
        let out = LowerAttributesConfigInput {
            enabled: Some(false),
        }
        .into_config(LowerAttributesConfig::ENABLED);
        assert!(!out.enabled);
    }

    #[test]
    fn lower_attributes_input_from_config_into_config_is_identity() {
        let cfg = LowerAttributesConfig::DISABLED;
        assert_eq!(
            LowerAttributesConfigInput::from_config(cfg)
                .into_config(LowerAttributesConfig::ENABLED),
            cfg
        );
    }

    #[test]
    fn rewrite_input_default_leaves_base_unchanged() {
        let base = disabled_rewrite();
        assert_eq!(RewriteConfigInput::default().into_config(base), base);
    }

    #[test]
    fn rewrite_input_partial_overlay() {
        let out = RewriteConfigInput {
            enabled: Some(false),
            max_iterations: None,
        }
        .into_config(RewriteConfig::DEFAULT);
        assert!(!out.enabled);
        assert_eq!(out.max_iterations, 100);
    }

    #[test]
    fn rewrite_input_from_config_into_config_is_identity() {
        let cfg = disabled_rewrite();
        assert_eq!(
            RewriteConfigInput::from_config(cfg).into_config(RewriteConfig::DEFAULT),
            cfg
        );
    }

    #[test]
    fn finalize_ast_input_default_leaves_base_unchanged() {
        let base = FinalizeAstConfig::DISABLED;
        assert_eq!(FinalizeAstConfigInput::default().into_config(base), base);
    }

    #[test]
    fn finalize_ast_input_partial_overlay() {
        let out = FinalizeAstConfigInput {
            enabled: Some(false),
        }
        .into_config(FinalizeAstConfig::ENABLED);
        assert!(!out.enabled);
    }

    #[test]
    fn finalize_ast_input_from_config_into_config_is_identity() {
        let cfg = FinalizeAstConfig::DISABLED;
        assert_eq!(
            FinalizeAstConfigInput::from_config(cfg).into_config(FinalizeAstConfig::ENABLED),
            cfg
        );
    }

    #[test]
    fn flatten_groups_input_default_leaves_base_unchanged() {
        let base = FlattenGroupsConfig::STRUCTURAL_ONLY;
        assert_eq!(FlattenGroupsConfigInput::default().into_config(base), base);
    }

    #[test]
    fn flatten_groups_input_partial_overlay() {
        let out = FlattenGroupsConfigInput {
            enabled: Some(false),
            preserve_empty_group: Some(true),
            ..Default::default()
        }
        .into_config(FlattenGroupsConfig::STRUCTURAL_ONLY);
        assert!(!out.enabled);
        assert!(out.preserve_empty_group);
        assert!(out.preserve_group_containing_declarative_command);
        assert!(!out.preserve_group_adjacent_to_command_like);
    }

    #[test]
    fn flatten_groups_overlay_enabled_on_corpus_preserves_guards() {
        let base = corpus().flatten_groups;
        assert_eq!(base, FlattenGroupsConfig::STRUCTURAL_ONLY);

        let out = FlattenGroupsConfigInput {
            enabled: Some(true),
            ..Default::default()
        }
        .into_config(base);
        assert_eq!(
            out,
            FlattenGroupsConfig {
                enabled: true,
                ..base
            }
        );
    }

    #[test]
    fn flatten_groups_input_from_config_into_config_is_identity() {
        let cfg = FlattenGroupsConfig::STRUCTURAL_ONLY;
        assert_eq!(
            FlattenGroupsConfigInput::from_config(cfg).into_config(FlattenGroupsConfig::STRICT),
            cfg
        );
    }

    #[test]
    fn transform_config_input_default_leaves_base_unchanged() {
        let base = corpus();
        assert_eq!(TransformConfigInput::default().into_config(base), base);
    }

    #[test]
    fn transform_config_input_fills_nested_defaults() {
        let input = TransformConfigInput {
            lower_attributes: Some(LowerAttributesConfigInput {
                enabled: Some(false),
            }),
            rewrite: None,
            finalize_ast: None,
            flatten_groups: Some(FlattenGroupsConfigInput {
                enabled: Some(true),
                preserve_empty_group: Some(false),
                ..Default::default()
            }),
        };

        let config = input.into_config(authoring());

        assert!(!config.lower_attributes.enabled);
        assert!(config.rewrite.enabled);
        assert_eq!(config.rewrite.max_iterations, 100);
        assert!(config.flatten_groups.enabled);
        assert!(!config.flatten_groups.preserve_empty_group);
    }

    #[test]
    fn transform_config_input_overlay_enabled_on_corpus_preserves_guards() {
        let base = corpus();
        let input = TransformConfigInput {
            flatten_groups: Some(FlattenGroupsConfigInput {
                enabled: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = input.into_config(base);
        assert_eq!(
            out.flatten_groups,
            FlattenGroupsConfig {
                enabled: true,
                ..base.flatten_groups
            }
        );
        assert_eq!(out.lower_attributes, base.lower_attributes);
        assert_eq!(out.rewrite, base.rewrite);
        assert_eq!(out.finalize_ast, base.finalize_ast);
    }

    #[test]
    fn transform_config_input_from_config_into_config_is_identity() {
        let cfg = corpus();
        assert_eq!(
            TransformConfigInput::from_config(cfg).into_config(authoring()),
            cfg
        );
    }

    #[test]
    fn transform_config_input_deserializes_camel_case_finalize_ast() {
        let input: TransformConfigInput = serde_json::from_value(serde_json::json!({
            "finalizeAst": {
                "enabled": false
            }
        }))
        .unwrap();

        let config = input.into_config(authoring());

        assert!(!config.finalize_ast.enabled);
    }

    #[test]
    fn transform_config_input_deserializes_camel_case_flatten_groups() {
        let input: TransformConfigInput = serde_json::from_value(serde_json::json!({
            "flattenGroups": {
                "preserveEmptyGroup": false
            }
        }))
        .unwrap();

        let config = input.into_config(authoring());

        assert!(!config.flatten_groups.preserve_empty_group);
    }

    #[test]
    fn normalize_config_input_default_leaves_base_unchanged() {
        let base = NormalizeConfig {
            parse: ParseConfig::STRICT,
            transform: corpus(),
        };
        assert_eq!(
            NormalizeConfigInput::default().into_config(base.clone()),
            base
        );
    }

    #[test]
    fn normalize_config_input_splits_parse_and_transform_fields() {
        let base = NormalizeConfig {
            parse: ParseConfig::LENIENT,
            transform: corpus(),
        };
        let input = NormalizeConfigInput {
            reject_unknown: Some(true),
            rewrite: Some(RewriteConfigInput {
                enabled: Some(false),
                max_iterations: None,
            }),
            ..Default::default()
        };
        let out = input.into_config(base.clone());
        assert!(out.parse.reject_unknown);
        assert!(!out.parse.abort_on_error);
        assert_eq!(out.parse.max_group_depth, base.parse.max_group_depth);
        assert!(!out.transform.rewrite.enabled);
        assert_eq!(out.transform.rewrite.max_iterations, 100);
        assert_eq!(out.transform.flatten_groups, base.transform.flatten_groups);
        assert_eq!(
            out.transform.lower_attributes,
            base.transform.lower_attributes
        );
        assert_eq!(out.transform.finalize_ast, base.transform.finalize_ast);
    }

    #[test]
    fn normalize_config_input_from_config_into_config_is_identity() {
        let cfg = NormalizeConfig {
            parse: ParseConfig::STRICT,
            transform: corpus(),
        };
        let other = NormalizeConfig {
            parse: ParseConfig::LENIENT,
            transform: authoring(),
        };
        assert_eq!(
            NormalizeConfigInput::from_config(cfg.clone()).into_config(other),
            cfg
        );
    }
}
