use crate::{
    FlattenGroupsConfig, FlattenGroupsReport, LowerAttributesConfig, LowerAttributesReport,
    TransformConfig, TransformReport,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ParseConfigInput {
    #[serde(alias = "reject_unknown")]
    pub reject_unknown: Option<bool>,
    #[serde(alias = "abort_on_error")]
    pub abort_on_error: Option<bool>,
    #[serde(alias = "max_group_depth")]
    pub max_group_depth: Option<usize>,
}

impl ParseConfigInput {
    pub fn into_config(
        self,
        mut base: texform_core::parse::ParseConfig,
    ) -> texform_core::parse::ParseConfig {
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RewriteConfigInput {
    pub enabled: Option<bool>,
    #[serde(alias = "max_iterations")]
    pub max_iterations: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FlattenGroupsConfigInput {
    pub enabled: Option<bool>,
    #[serde(alias = "preserve_group_containing_declarative_command")]
    pub preserve_group_containing_declarative_command: Option<bool>,
    #[serde(alias = "preserve_group_in_script_base_slot")]
    pub preserve_group_in_script_base_slot: Option<bool>,
    #[serde(alias = "preserve_group_inside_env_body")]
    pub preserve_group_inside_env_body: Option<bool>,
    #[serde(alias = "preserve_group_containing_infix")]
    pub preserve_group_containing_infix: Option<bool>,
    #[serde(alias = "preserve_group_adjacent_to_command_like")]
    pub preserve_group_adjacent_to_command_like: Option<bool>,
    #[serde(alias = "preserve_group_as_argument_of_command")]
    pub preserve_group_as_argument_of_command: Option<bool>,
    #[serde(alias = "preserve_group_after_scripted_command_like")]
    pub preserve_group_after_scripted_command_like: Option<bool>,
    #[serde(alias = "preserve_empty_group")]
    pub preserve_empty_group: Option<bool>,
    #[serde(alias = "preserve_group_with_lone_atom_spacing_char")]
    pub preserve_group_with_lone_atom_spacing_char: Option<bool>,
    #[serde(alias = "preserve_group_starting_with_atom_spacing_char")]
    pub preserve_group_starting_with_atom_spacing_char: Option<bool>,
    #[serde(alias = "preserve_group_containing_delimited_pair")]
    pub preserve_group_containing_delimited_pair: Option<bool>,
}

impl FlattenGroupsConfigInput {
    pub fn into_config(self, mut base: FlattenGroupsConfig) -> FlattenGroupsConfig {
        if let Some(enabled) = self.enabled {
            base.enabled = enabled;
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TransformConfigInput {
    #[serde(alias = "lower_attributes")]
    pub lower_attributes: Option<LowerAttributesConfigInput>,
    pub rewrite: Option<RewriteConfigInput>,
    #[serde(alias = "flatten_groups")]
    pub flatten_groups: Option<FlattenGroupsConfigInput>,
}

impl TransformConfigInput {
    pub fn into_config(self) -> TransformConfig {
        self.into_config_with_base(crate::Profile::Authoring.default_transform_config())
    }

    pub fn into_config_with_base(self, mut base: TransformConfig) -> TransformConfig {
        if let Some(lower_attributes) = self.lower_attributes {
            base.lower_attributes_enabled = lower_attributes
                .into_config(LowerAttributesConfig {
                    enabled: base.lower_attributes_enabled,
                })
                .enabled;
        }
        if let Some(rewrite) = self.rewrite {
            if let Some(enabled) = rewrite.enabled {
                base.rewrite_enabled = enabled;
            }
            if let Some(max_iterations) = rewrite.max_iterations {
                base.max_iterations = max_iterations;
            }
        }
        if let Some(flatten_groups) = self.flatten_groups {
            base.flatten_groups = flatten_groups.into_config(base.flatten_groups);
        }
        base
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TransformReportDto {
    pub iterations: usize,
    pub applied: Vec<AppliedRuleDto>,
    pub lower_attributes: LowerAttributesReportDto,
    pub flatten_groups: FlattenGroupsReportDto,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AppliedRuleDto {
    pub key: String,
    pub count: usize,
    pub skipped_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct LowerAttributesReportDto {
    pub eliminated_empty_segments: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FlattenGroupsReportDto {
    pub removed_empty: usize,
    pub replaced_single_child: usize,
    pub inlined_multi_child: usize,
    pub unwrapped_slot: usize,
    pub preserved_group_containing_declarative_command: usize,
    pub preserved_group_in_script_base_slot: usize,
    pub preserved_group_inside_env_body: usize,
    pub preserved_group_containing_infix: usize,
    pub preserved_group_adjacent_to_command_like: usize,
    pub preserved_group_as_argument_of_command: usize,
    pub preserved_group_after_scripted_command_like: usize,
    pub preserved_empty_group: usize,
    pub preserved_group_with_lone_atom_spacing_char: usize,
    pub preserved_group_starting_with_atom_spacing_char: usize,
    pub preserved_group_containing_delimited_pair: usize,
}

pub fn transform_report_to_dto(report: &TransformReport) -> TransformReportDto {
    TransformReportDto {
        iterations: report.rewrite.iterations,
        applied: report
            .rewrite
            .applied
            .iter()
            .map(|stat| AppliedRuleDto {
                key: stat.key.to_string(),
                count: stat.count,
                skipped_count: stat.skipped_count,
            })
            .collect(),
        lower_attributes: lower_attributes_report_to_dto(&report.lower_attributes),
        flatten_groups: flatten_groups_report_to_dto(&report.flatten_groups),
    }
}

fn lower_attributes_report_to_dto(report: &LowerAttributesReport) -> LowerAttributesReportDto {
    LowerAttributesReportDto {
        eliminated_empty_segments: report.eliminated_empty_segments,
    }
}

fn flatten_groups_report_to_dto(report: &FlattenGroupsReport) -> FlattenGroupsReportDto {
    FlattenGroupsReportDto {
        removed_empty: report.removed_empty,
        replaced_single_child: report.replaced_single_child,
        inlined_multi_child: report.inlined_multi_child,
        unwrapped_slot: report.unwrapped_slot,
        preserved_group_containing_declarative_command: report
            .preserved_group_containing_declarative_command,
        preserved_group_in_script_base_slot: report.preserved_group_in_script_base_slot,
        preserved_group_inside_env_body: report.preserved_group_inside_env_body,
        preserved_group_containing_infix: report.preserved_group_containing_infix,
        preserved_group_adjacent_to_command_like: report.preserved_group_adjacent_to_command_like,
        preserved_group_as_argument_of_command: report.preserved_group_as_argument_of_command,
        preserved_group_after_scripted_command_like: report
            .preserved_group_after_scripted_command_like,
        preserved_empty_group: report.preserved_empty_group,
        preserved_group_with_lone_atom_spacing_char: report
            .preserved_group_with_lone_atom_spacing_char,
        preserved_group_starting_with_atom_spacing_char: report
            .preserved_group_starting_with_atom_spacing_char,
        preserved_group_containing_delimited_pair: report.preserved_group_containing_delimited_pair,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_input_overrides_default_fields() {
        let input = ParseConfigInput {
            reject_unknown: Some(true),
            abort_on_error: None,
            max_group_depth: Some(7),
        };

        let config = input.into_config(texform_core::parse::ParseConfig::default());

        assert!(config.reject_unknown);
        assert!(!config.abort_on_error);
        assert_eq!(config.max_group_depth, 7);
    }

    #[test]
    fn transform_config_input_fills_nested_defaults() {
        let input = TransformConfigInput {
            lower_attributes: Some(LowerAttributesConfigInput {
                enabled: Some(false),
            }),
            rewrite: None,
            flatten_groups: Some(FlattenGroupsConfigInput {
                enabled: Some(true),
                preserve_empty_group: Some(false),
                ..Default::default()
            }),
        };

        let config = input.into_config();

        assert!(!config.lower_attributes_enabled);
        assert!(config.rewrite_enabled);
        assert_eq!(config.max_iterations, 100);
        assert!(config.flatten_groups.enabled);
        assert!(!config.flatten_groups.preserve_empty_group);
    }

    #[test]
    fn transform_config_input_deserializes_camel_case_flatten_groups() {
        let input: TransformConfigInput = serde_json::from_value(serde_json::json!({
            "flattenGroups": {
                "preserveEmptyGroup": false
            }
        }))
        .unwrap();

        let config = input.into_config();

        assert!(!config.flatten_groups.preserve_empty_group);
    }

    #[test]
    fn transform_config_input_deserializes_snake_case_flatten_groups() {
        let input: TransformConfigInput = serde_json::from_value(serde_json::json!({
            "flatten_groups": {
                "preserve_empty_group": false
            }
        }))
        .unwrap();

        let config = input.into_config();

        assert!(!config.flatten_groups.preserve_empty_group);
    }

    #[test]
    fn transform_report_to_dto_reads_rewrite_report() {
        let mut report = crate::TransformReport::default();
        let key = texform_transform::rewrite::all_rules()[0].meta().key;
        report.rewrite.iterations = 3;
        report
            .rewrite
            .applied
            .push(texform_transform::rewrite::AppliedRuleStat {
                key,
                count: 2,
                skipped_count: 1,
            });

        let dto = transform_report_to_dto(&report);

        assert_eq!(dto.iterations, 3);
        assert_eq!(dto.applied.len(), 1);
        assert_eq!(dto.applied[0].key, key.to_string());
        assert_eq!(dto.applied[0].count, 2);
        assert_eq!(dto.applied[0].skipped_count, 1);
    }
}
