use crate::argspec::parsed_arg_spec_slot;
use crate::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, Document, EditError,
    Error, FinalizeAstConfig, FinalizeAstReport, FlattenGroupsConfig, FlattenGroupsReport,
    FromSyntaxError, LowerAttributesConfig, LowerAttributesReport, ParseDiagnostic,
    ParsedArgSpecSlot, TransformConfig, TransformReport,
};
use texform_transform::{
    Attr, AttrValue, AttributeFormCounts, MathFontValue, SizeValue, StyleValue, TextFamily,
    TextSeries, TextShape,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ParseConfigInput {
    pub reject_unknown: Option<bool>,
    pub abort_on_error: Option<bool>,
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
#[serde(default, deny_unknown_fields)]
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
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RewriteConfigInput {
    pub enabled: Option<bool>,
    pub max_iterations: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(default, deny_unknown_fields)]
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct TransformConfigInput {
    pub lower_attributes: Option<LowerAttributesConfigInput>,
    pub rewrite: Option<RewriteConfigInput>,
    pub finalize_ast: Option<FinalizeAstConfigInput>,
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
        if let Some(finalize_ast) = self.finalize_ast {
            base.finalize_ast = finalize_ast.into_config(base.finalize_ast);
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
    pub rules: Vec<RewriteRuleDto>,
    pub finalize_ast: FinalizeAstReportDto,
    pub flatten_groups: FlattenGroupsReportDto,
    pub lower_attributes: LowerAttributesReportDto,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CommandInfoDto {
    pub name: String,
    pub kind: &'static str,
    pub allowed_mode: &'static str,
    pub spec_string: String,
    pub from_packages: Vec<String>,
    pub tags: Vec<String>,
    pub args: Vec<ParsedArgSpecSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct EnvInfoDto {
    pub name: String,
    pub allowed_mode: &'static str,
    pub body_mode: &'static str,
    pub spec_string: String,
    pub from_packages: Vec<String>,
    pub tags: Vec<String>,
    pub args: Vec<ParsedArgSpecSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CharacterAttributesInfoDto {
    pub mathvariant: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CharacterInfoDto {
    pub name: String,
    pub allowed_mode: &'static str,
    pub unicode_value: String,
    pub attributes: CharacterAttributesInfoDto,
    pub package: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct BindingErrorDto {
    pub kind: &'static str,
    pub message: String,
    pub diagnostics: Vec<ParseDiagnostic>,
}

pub struct BindingErrorParts {
    pub error: BindingErrorDto,
    pub document: Option<Document>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RewriteRuleDto {
    pub key: String,
    pub applied_count: usize,
    pub skipped_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct LowerAttributesReportDto {
    pub attributes: Vec<LowerAttributeDto>,
    pub eliminated_empty_segments: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct LowerAttributeDto {
    pub attr: String,
    pub value: String,
    pub consumed: AttributeFormCountsDto,
    pub redundant: AttributeFormCountsDto,
    pub emitted: AttributeFormCountsDto,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AttributeFormCountsDto {
    pub declaratives: usize,
    pub prefixes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FlattenGroupsReportDto {
    pub actions: FlattenGroupsActionCountsDto,
    pub guards: FlattenGroupsGuardCountsDto,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FlattenGroupsActionCountsDto {
    pub removed_empty: usize,
    pub replaced_single_child: usize,
    pub inlined_multi_child: usize,
    pub unwrapped_slot: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FlattenGroupsGuardCountsDto {
    pub preserve_group_containing_declarative_command: usize,
    pub preserve_group_in_script_base_slot: usize,
    pub preserve_group_inside_env_body: usize,
    pub preserve_group_containing_infix: usize,
    pub preserve_group_adjacent_to_command_like: usize,
    pub preserve_group_as_argument_of_command: usize,
    pub preserve_group_after_scripted_command_like: usize,
    pub preserve_empty_group: usize,
    pub preserve_group_with_lone_atom_spacing_char: usize,
    pub preserve_group_starting_with_atom_spacing_char: usize,
    pub preserve_group_containing_delimited_pair: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FinalizeAstReportDto {
    pub steps: FinalizeAstStepReportsDto,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FinalizeAstStepReportsDto {
    pub merge_adjacent_primes: FinalizeAstStepReportDto,
    pub normalize_text_sequences: FinalizeAstStepReportDto,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FinalizeAstStepReportDto {
    pub applied_count: usize,
}

pub fn transform_report_to_dto(report: &TransformReport) -> TransformReportDto {
    let mut rules: Vec<_> = report
        .rewrite
        .rules
        .iter()
        .map(|stat| RewriteRuleDto {
            key: stat.key.to_string(),
            applied_count: stat.applied_count,
            skipped_count: stat.skipped_count,
        })
        .collect();
    rules.sort_by(|left, right| left.key.cmp(&right.key));

    TransformReportDto {
        iterations: report.rewrite.iterations,
        rules,
        finalize_ast: finalize_ast_report_to_dto(&report.finalize_ast),
        flatten_groups: flatten_groups_report_to_dto(&report.flatten_groups),
        lower_attributes: lower_attributes_report_to_dto(&report.lower_attributes),
    }
}

pub fn command_info_to_dto(record: &ActiveCommandRecord) -> CommandInfoDto {
    CommandInfoDto {
        name: record.name.to_string(),
        kind: command_kind_to_dto_key(record.kind),
        allowed_mode: record.allowed_mode.as_str(),
        spec_string: record.argspec.source.to_string(),
        from_packages: record
            .from_packages
            .iter()
            .map(|package| (*package).to_string())
            .collect(),
        tags: record.tags.iter().map(|tag| (*tag).to_string()).collect(),
        args: record
            .argspec
            .args
            .iter()
            .map(parsed_arg_spec_slot)
            .collect(),
    }
}

pub fn env_info_to_dto(record: &ActiveEnvironmentRecord) -> EnvInfoDto {
    EnvInfoDto {
        name: record.name.to_string(),
        allowed_mode: record.allowed_mode.as_str(),
        body_mode: content_mode_to_dto_key(record.body_mode),
        spec_string: record.argspec.source.to_string(),
        from_packages: record
            .from_packages
            .iter()
            .map(|package| (*package).to_string())
            .collect(),
        tags: record.tags.iter().map(|tag| (*tag).to_string()).collect(),
        args: record
            .argspec
            .args
            .iter()
            .map(parsed_arg_spec_slot)
            .collect(),
    }
}

pub fn character_info_to_dto(record: &ActiveCharacterRecord) -> CharacterInfoDto {
    CharacterInfoDto {
        name: record.name.to_string(),
        allowed_mode: record.allowed_mode.as_str(),
        unicode_value: record.unicode_value.to_string(),
        attributes: CharacterAttributesInfoDto {
            mathvariant: record.attributes.mathvariant.clone(),
        },
        package: record.package.to_string(),
    }
}

pub fn normalize_error_to_parts(error: crate::NormalizeError) -> BindingErrorParts {
    match error {
        Error::Parse(error) => {
            let message = error.to_string();
            let (document, diagnostics) = error.into_parts();
            BindingErrorParts {
                error: BindingErrorDto {
                    kind: "parse",
                    message,
                    diagnostics,
                },
                document,
            }
        }
        Error::MissingProfile
        | Error::UnknownRule(_)
        | Error::ParserBuild(_)
        | Error::TransformBuild(_) => BindingErrorParts {
            error: BindingErrorDto {
                kind: "config",
                message: error.to_string(),
                diagnostics: Vec::new(),
            },
            document: None,
        },
        Error::ForeignDocument | Error::Transform(_) => BindingErrorParts {
            error: BindingErrorDto {
                kind: "transform",
                message: error.to_string(),
                diagnostics: Vec::new(),
            },
            document: None,
        },
        Error::IncompleteTree | Error::Serialize(_) => BindingErrorParts {
            error: BindingErrorDto {
                kind: "internal",
                message: error.to_string(),
                diagnostics: Vec::new(),
            },
            document: None,
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NodeSpanEntryDto {
    pub id: String,
    pub span: crate::Span,
}

pub fn node_spans_to_dto(document: &Document) -> Vec<NodeSpanEntryDto> {
    document
        .node_spans()
        .into_iter()
        .map(|entry| NodeSpanEntryDto {
            id: entry.id,
            span: entry.span,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PackageInfoDto {
    pub name: String,
    pub commands: usize,
    pub environments: usize,
}

pub fn list_packages_to_dto() -> Vec<PackageInfoDto> {
    crate::list_packages()
        .into_iter()
        .map(|info| PackageInfoDto {
            name: info.name,
            commands: info.commands,
            environments: info.environments,
        })
        .collect()
}

pub fn from_syntax_error_to_dto(error: FromSyntaxError) -> BindingErrorDto {
    BindingErrorDto {
        kind: "parse",
        message: error.to_string(),
        diagnostics: Vec::new(),
    }
}

pub fn edit_error_to_dto(error: EditError) -> BindingErrorDto {
    BindingErrorDto {
        kind: "edit",
        message: error.to_string(),
        diagnostics: Vec::new(),
    }
}

pub fn config_error_to_dto(message: impl Into<String>) -> BindingErrorDto {
    BindingErrorDto {
        kind: "config",
        message: message.into(),
        diagnostics: Vec::new(),
    }
}

fn command_kind_to_dto_key(kind: texform_core::parse::CommandKind) -> &'static str {
    match kind {
        texform_core::parse::CommandKind::Prefix => "prefix",
        texform_core::parse::CommandKind::Infix => "infix",
        texform_core::parse::CommandKind::Declarative => "declarative",
    }
}

fn content_mode_to_dto_key(mode: texform_interface::syntax_node::ContentMode) -> &'static str {
    match mode {
        texform_interface::syntax_node::ContentMode::Math => "math",
        texform_interface::syntax_node::ContentMode::Text => "text",
    }
}

fn finalize_ast_report_to_dto(report: &FinalizeAstReport) -> FinalizeAstReportDto {
    FinalizeAstReportDto {
        steps: FinalizeAstStepReportsDto {
            merge_adjacent_primes: FinalizeAstStepReportDto {
                applied_count: report.steps.merge_adjacent_primes.applied_count,
            },
            normalize_text_sequences: FinalizeAstStepReportDto {
                applied_count: report.steps.normalize_text_sequences.applied_count,
            },
        },
    }
}

fn lower_attributes_report_to_dto(report: &LowerAttributesReport) -> LowerAttributesReportDto {
    let mut attributes: Vec<_> = report
        .attributes
        .iter()
        .map(|(set, stat)| LowerAttributeDto {
            attr: attr_to_dto_key(set.attr()).to_string(),
            value: attr_value_to_dto_key(set.attr(), set.value()),
            consumed: attribute_form_counts_to_dto(&stat.consumed),
            redundant: attribute_form_counts_to_dto(&stat.redundant),
            emitted: attribute_form_counts_to_dto(&stat.emitted),
        })
        .collect();
    attributes.sort_by(|left, right| {
        left.attr
            .cmp(&right.attr)
            .then_with(|| left.value.cmp(&right.value))
    });

    LowerAttributesReportDto {
        attributes,
        eliminated_empty_segments: report.eliminated_empty_segments,
    }
}

fn attribute_form_counts_to_dto(counts: &AttributeFormCounts) -> AttributeFormCountsDto {
    AttributeFormCountsDto {
        declaratives: counts.declaratives,
        prefixes: counts.prefixes,
    }
}

fn flatten_groups_report_to_dto(report: &FlattenGroupsReport) -> FlattenGroupsReportDto {
    FlattenGroupsReportDto {
        actions: FlattenGroupsActionCountsDto {
            removed_empty: report.actions.removed_empty,
            replaced_single_child: report.actions.replaced_single_child,
            inlined_multi_child: report.actions.inlined_multi_child,
            unwrapped_slot: report.actions.unwrapped_slot,
        },
        guards: FlattenGroupsGuardCountsDto {
            preserve_group_containing_declarative_command: report
                .guards
                .preserve_group_containing_declarative_command,
            preserve_group_in_script_base_slot: report.guards.preserve_group_in_script_base_slot,
            preserve_group_inside_env_body: report.guards.preserve_group_inside_env_body,
            preserve_group_containing_infix: report.guards.preserve_group_containing_infix,
            preserve_group_adjacent_to_command_like: report
                .guards
                .preserve_group_adjacent_to_command_like,
            preserve_group_as_argument_of_command: report
                .guards
                .preserve_group_as_argument_of_command,
            preserve_group_after_scripted_command_like: report
                .guards
                .preserve_group_after_scripted_command_like,
            preserve_empty_group: report.guards.preserve_empty_group,
            preserve_group_with_lone_atom_spacing_char: report
                .guards
                .preserve_group_with_lone_atom_spacing_char,
            preserve_group_starting_with_atom_spacing_char: report
                .guards
                .preserve_group_starting_with_atom_spacing_char,
            preserve_group_containing_delimited_pair: report
                .guards
                .preserve_group_containing_delimited_pair,
        },
    }
}

fn attr_to_dto_key(attr: Attr) -> &'static str {
    match attr {
        Attr::MathFont => "math_font",
        Attr::MathSize => "math_size",
        Attr::MathStyle => "math_style",
        Attr::TextFamily => "text_family",
        Attr::TextSeries => "text_series",
        Attr::TextShape => "text_shape",
        Attr::TextSize => "text_size",
    }
}

fn attr_value_to_dto_key(attr: Attr, value: AttrValue) -> String {
    match (attr, value) {
        (Attr::MathFont, AttrValue::MathFont(value)) => math_font_value_to_dto_key(value),
        (Attr::MathSize | Attr::TextSize, AttrValue::Size(value)) => size_value_to_dto_key(value),
        (Attr::MathStyle, AttrValue::Style(value)) => style_value_to_dto_key(value),
        (Attr::TextFamily, AttrValue::TextFamily(value)) => text_family_to_dto_key(value),
        (Attr::TextSeries, AttrValue::TextSeries(value)) => text_series_to_dto_key(value),
        (Attr::TextShape, AttrValue::TextShape(value)) => text_shape_to_dto_key(value),
        (_, other) => fallback_attr_value_to_dto_key(other),
    }
}

fn math_font_value_to_dto_key(value: MathFontValue) -> String {
    match value.0 {
        "VARIANT.BOLD" => "bold".to_string(),
        "VARIANT.CALLIGRAPHIC" => "calligraphic".to_string(),
        "VARIANT.MATHITALIC" => "mathitalic".to_string(),
        "VARIANT.ITALIC" => "italic".to_string(),
        "VARIANT.NORMAL" => "normal".to_string(),
        "VARIANT.SANSSERIF" => "sans_serif".to_string(),
        "VARIANT.MONOSPACE" => "monospace".to_string(),
        "-tex-oldstyle" => "oldstyle".to_string(),
        other => string_to_dto_token(other),
    }
}

fn size_value_to_dto_key(value: SizeValue) -> String {
    let scaled = value.0;
    let sign = if scaled < 0 { "minus_" } else { "" };
    let absolute = scaled.abs();
    format!("{}scale_{}_{:02}", sign, absolute / 100, absolute % 100)
}

fn style_value_to_dto_key(value: StyleValue) -> String {
    match (value.letter, value.display, value.level) {
        ("D", true, 0) => "displaystyle".to_string(),
        ("T", false, 0) => "textstyle".to_string(),
        ("S", false, 1) => "scriptstyle".to_string(),
        ("SS", false, 2) => "scriptscriptstyle".to_string(),
        _ => format!(
            "style_{}_{}_{}",
            string_to_dto_token(value.letter),
            if value.display { "display" } else { "inline" },
            value.level
        ),
    }
}

fn text_family_to_dto_key(value: TextFamily) -> String {
    match value {
        TextFamily::Roman => "roman",
        TextFamily::SansSerif => "sans_serif",
        TextFamily::Typewriter => "typewriter",
        TextFamily::Calligraphic => "calligraphic",
        TextFamily::Italic => "italic",
        TextFamily::Oldstyle => "oldstyle",
    }
    .to_string()
}

fn text_series_to_dto_key(value: TextSeries) -> String {
    match value {
        TextSeries::Medium => "medium",
        TextSeries::Bold => "bold",
    }
    .to_string()
}

fn text_shape_to_dto_key(value: TextShape) -> String {
    match value {
        TextShape::Upright => "upright",
        TextShape::Italic => "italic",
        TextShape::Slanted => "slanted",
        TextShape::SmallCaps => "small_caps",
    }
    .to_string()
}

fn fallback_attr_value_to_dto_key(value: AttrValue) -> String {
    match value {
        AttrValue::MathFont(value) => math_font_value_to_dto_key(value),
        AttrValue::Size(value) => size_value_to_dto_key(value),
        AttrValue::Style(value) => style_value_to_dto_key(value),
        AttrValue::TextFamily(value) => text_family_to_dto_key(value),
        AttrValue::TextSeries(value) => text_series_to_dto_key(value),
        AttrValue::TextShape(value) => text_shape_to_dto_key(value),
    }
}

fn string_to_dto_token(value: &str) -> String {
    let mut token = String::new();
    let mut last_was_separator = false;
    let value = value.strip_prefix("VARIANT.").unwrap_or(value);

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            token.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !token.is_empty() {
            token.push('_');
            last_was_separator = true;
        }
    }

    if token.ends_with('_') {
        token.pop();
    }
    if token.is_empty() {
        "unknown".to_string()
    } else {
        token
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
            finalize_ast: None,
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
    fn transform_config_input_deserializes_camel_case_finalize_ast() {
        let input: TransformConfigInput = serde_json::from_value(serde_json::json!({
            "finalizeAst": {
                "enabled": false
            }
        }))
        .unwrap();

        let config = input.into_config();

        assert!(!config.finalize_ast.enabled);
    }

    #[test]
    fn transform_config_input_rejects_snake_case_finalize_ast() {
        let error = serde_json::from_value::<TransformConfigInput>(serde_json::json!({
            "finalize_ast": {
                "enabled": false
            }
        }))
        .expect_err("JS-facing transform config should reject snake_case fields");

        assert!(error.to_string().contains("unknown field"));
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
    fn transform_config_input_rejects_snake_case_flatten_groups() {
        let error = serde_json::from_value::<TransformConfigInput>(serde_json::json!({
            "flatten_groups": {
                "preserve_empty_group": false
            }
        }))
        .expect_err("JS-facing transform config should reject snake_case fields");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn transform_report_to_dto_reads_rewrite_report() {
        let mut report = crate::TransformReport::default();
        let key = texform_transform::rewrite::all_rules()[0].meta().key;
        report.rewrite.iterations = 3;
        report
            .rewrite
            .rules
            .push(texform_transform::rewrite::RewriteRuleStat {
                key,
                applied_count: 2,
                skipped_count: 1,
            });

        let dto = transform_report_to_dto(&report);

        assert_eq!(dto.iterations, 3);
        assert_eq!(dto.rules.len(), 1);
        assert_eq!(dto.rules[0].key, key.to_string());
        assert_eq!(dto.rules[0].applied_count, 2);
        assert_eq!(dto.rules[0].skipped_count, 1);
    }

    #[test]
    fn transform_report_to_dto_reads_finalize_ast_report() {
        let mut report = crate::TransformReport::default();
        report
            .finalize_ast
            .steps
            .merge_adjacent_primes
            .applied_count = 4;
        report
            .finalize_ast
            .steps
            .normalize_text_sequences
            .applied_count = 2;

        let dto = transform_report_to_dto(&report);
        let json = serde_json::to_value(&dto).unwrap();

        assert_eq!(
            dto.finalize_ast.steps.merge_adjacent_primes.applied_count,
            4
        );
        assert_eq!(
            dto.finalize_ast
                .steps
                .normalize_text_sequences
                .applied_count,
            2
        );
        assert_eq!(
            json["finalize_ast"]["steps"]["merge_adjacent_primes"]["applied_count"],
            4
        );
        assert_eq!(
            json["finalize_ast"]["steps"]["normalize_text_sequences"]["applied_count"],
            2
        );
    }

    #[test]
    fn transform_report_to_dto_groups_flatten_groups_report() {
        let mut report = crate::TransformReport::default();
        report.flatten_groups.actions = texform_transform::FlattenGroupsActionCounts {
            removed_empty: 1,
            replaced_single_child: 2,
            inlined_multi_child: 3,
            unwrapped_slot: 4,
        };
        report.flatten_groups.guards = texform_transform::FlattenGroupsGuardCounts {
            preserve_group_containing_declarative_command: 5,
            preserve_group_in_script_base_slot: 6,
            preserve_group_inside_env_body: 7,
            preserve_group_containing_infix: 8,
            preserve_group_adjacent_to_command_like: 9,
            preserve_group_as_argument_of_command: 10,
            preserve_group_after_scripted_command_like: 11,
            preserve_empty_group: 12,
            preserve_group_with_lone_atom_spacing_char: 13,
            preserve_group_starting_with_atom_spacing_char: 14,
            preserve_group_containing_delimited_pair: 15,
        };

        let dto = transform_report_to_dto(&report).flatten_groups;

        assert_eq!(dto.actions.removed_empty, 1);
        assert_eq!(dto.actions.replaced_single_child, 2);
        assert_eq!(dto.actions.inlined_multi_child, 3);
        assert_eq!(dto.actions.unwrapped_slot, 4);
        assert_eq!(dto.guards.preserve_group_containing_declarative_command, 5);
        assert_eq!(dto.guards.preserve_group_in_script_base_slot, 6);
        assert_eq!(dto.guards.preserve_group_inside_env_body, 7);
        assert_eq!(dto.guards.preserve_group_containing_infix, 8);
        assert_eq!(dto.guards.preserve_group_adjacent_to_command_like, 9);
        assert_eq!(dto.guards.preserve_group_as_argument_of_command, 10);
        assert_eq!(dto.guards.preserve_group_after_scripted_command_like, 11);
        assert_eq!(dto.guards.preserve_empty_group, 12);
        assert_eq!(dto.guards.preserve_group_with_lone_atom_spacing_char, 13);
        assert_eq!(
            dto.guards.preserve_group_starting_with_atom_spacing_char,
            14
        );
        assert_eq!(dto.guards.preserve_group_containing_delimited_pair, 15);
    }

    #[test]
    fn transform_report_to_dto_reads_lower_attributes_report_in_stable_order() {
        let mut report = crate::TransformReport::default();
        report.lower_attributes.eliminated_empty_segments = 2;
        report.lower_attributes.attributes.insert(
            texform_transform::AttributeSet::new(
                texform_transform::Attr::TextSize,
                texform_transform::AttrValue::Size(texform_transform::SizeValue(120)),
            ),
            texform_transform::AttributeStat {
                consumed: texform_transform::AttributeFormCounts {
                    declaratives: 3,
                    prefixes: 4,
                },
                redundant: texform_transform::AttributeFormCounts {
                    declaratives: 5,
                    prefixes: 6,
                },
                emitted: texform_transform::AttributeFormCounts {
                    declaratives: 7,
                    prefixes: 8,
                },
            },
        );
        report.lower_attributes.attributes.insert(
            texform_transform::AttributeSet::new(
                texform_transform::Attr::MathStyle,
                texform_transform::AttrValue::Style(texform_transform::StyleValue {
                    letter: "S",
                    display: false,
                    level: 1,
                }),
            ),
            texform_transform::AttributeStat {
                consumed: texform_transform::AttributeFormCounts {
                    declaratives: 1,
                    prefixes: 0,
                },
                redundant: texform_transform::AttributeFormCounts::default(),
                emitted: texform_transform::AttributeFormCounts {
                    declaratives: 1,
                    prefixes: 0,
                },
            },
        );

        let dto = transform_report_to_dto(&report).lower_attributes;

        assert_eq!(dto.eliminated_empty_segments, 2);
        assert_eq!(dto.attributes.len(), 2);
        assert_eq!(dto.attributes[0].attr, "math_style");
        assert_eq!(dto.attributes[0].value, "scriptstyle");
        assert_eq!(dto.attributes[0].consumed.declaratives, 1);
        assert_eq!(dto.attributes[1].attr, "text_size");
        assert_eq!(dto.attributes[1].value, "scale_1_20");
        assert_eq!(dto.attributes[1].consumed.prefixes, 4);
        assert_eq!(dto.attributes[1].redundant.declaratives, 5);
        assert_eq!(dto.attributes[1].emitted.prefixes, 8);
    }
}
