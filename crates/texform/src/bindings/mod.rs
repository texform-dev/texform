pub use crate::{ArgumentTable, ColumnarTree, NodeTable};

mod input;
mod read;

pub use input::{
    ContextItemInput, ContextTarget, FinalizeAstConfigInput, FlattenGroupsConfigInput,
    KnowledgeBaseInput, LowerAttributesConfigInput, NormalizeConfigInput, ParseConfigInput,
    RewriteConfigInput, SerializeOptionsInput, TransformConfigInput,
};
pub use read::{ReadError, format_read_error, read, snake_to_camel};
pub use texform_core::document::parse_char;

use crate::argspec::parsed_arg_spec_slot;
use crate::diagnostics::{
    FinalizeAstReport, FlattenGroupsReport, LowerAttributesReport, TransformReport,
};
use crate::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, ArgKindRef, ArgRef,
    Document, EditError, Error, FromSyntaxError, GroupKindRef, NodeId, NodeRef, ParseDiagnostic,
    ParsedArgSpecSlot, SerializationTokenKind, SerializeOptions, SyntaxNode, TokenizedLatex,
};
use texform_transform::{
    Attr, AttrValue, AttributeFormCounts, MathFontValue, SizeValue, StyleValue, TextFamily,
    TextSeries, TextShape,
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TokenizedLatexDto {
    pub latex: String,
    pub tokens: Vec<SerializationTokenDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SerializationTokenDto {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: &'static str,
    pub mode: &'static str,
}

pub fn tokenized_latex_to_dto(result: TokenizedLatex) -> TokenizedLatexDto {
    TokenizedLatexDto {
        latex: result.latex,
        tokens: result
            .tokens
            .into_iter()
            .map(|token| SerializationTokenDto {
                text: token.text,
                start_byte: token.span.start,
                end_byte: token.span.end,
                kind: match token.kind {
                    SerializationTokenKind::ControlSequence => "control_sequence",
                    SerializationTokenKind::Character => "character",
                    SerializationTokenKind::Delimiter => "delimiter",
                    SerializationTokenKind::Text => "text",
                    SerializationTokenKind::Raw => "raw",
                    SerializationTokenKind::Error => "error",
                },
                mode: token.mode.as_str(),
            })
            .collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TransformReportDto {
    pub warnings: Vec<TransformWarningDto>,
    pub lower_attributes: LowerAttributesReportDto,
    pub rewrite: RewriteReportDto,
    pub finalize_ast: FinalizeAstReportDto,
    pub flatten_groups: FlattenGroupsReportDto,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TransformWarningDto {
    pub kind: String,
    pub name: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RewriteReportDto {
    pub iterations: usize,
    pub rules: Vec<RewriteRuleDto>,
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DelimiterInfoDto {
    pub name: String,
    pub is_control_sequence: bool,
    pub allowed_mode: &'static str,
    pub unicode_value: String,
    pub attributes: CharacterAttributesInfoDto,
    pub package: String,
}

pub fn delimiter_info_to_dto(record: &crate::ActiveDelimiterRecord) -> DelimiterInfoDto {
    DelimiterInfoDto {
        name: record.name.to_string(),
        is_control_sequence: record.is_control_sequence,
        allowed_mode: record.allowed_mode.as_str(),
        unicode_value: record.unicode_value.clone(),
        attributes: CharacterAttributesInfoDto {
            mathvariant: record.attributes.mathvariant.clone(),
        },
        package: record.package.clone(),
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct BindingErrorDto {
    pub kind: &'static str,
    pub message: String,
    pub diagnostics: Vec<ParseDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance: Option<ConformanceErrorDto>,
}

impl BindingErrorDto {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            diagnostics: Vec::new(),
            conformance: None,
        }
    }
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
    pub guard_hits: FlattenGroupsGuardCountsDto,
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
    pub unknown_command_arguments: usize,
    pub declarative_scope: usize,
    pub script_base: usize,
    pub env_body: usize,
    pub infix_scope: usize,
    pub command_contact: usize,
    pub command_contact_via_scripted_base: usize,
    pub empty_group: usize,
    pub lone_atom_spacing_char: usize,
    pub leading_atom_spacing_char: usize,
    pub delimited_pair: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FinalizeAstReportDto {
    pub prime_run_merges: usize,
    pub text_normalizations: usize,
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
        warnings: report.warnings.iter().map(|warning| {
            let (kind, name, spelling) = match warning {
                texform_transform::TransformWarning::UnknownCommand { name } => ("unknown-command", name, format!("\\{name}")),
                texform_transform::TransformWarning::UnknownEnvironment { name } => ("unknown-environment", name, format!("environment `{name}`")),
            };
            TransformWarningDto {
                kind: kind.to_owned(),
                name: name.clone(),
                message: format!("Unknown {spelling}: its arguments and scope are not known; normalization may change its meaning."),
            }
        }).collect(),
        lower_attributes: lower_attributes_report_to_dto(&report.lower_attributes),
        rewrite: RewriteReportDto {
            iterations: report.rewrite.iterations,
            rules,
        },
        finalize_ast: finalize_ast_report_to_dto(&report.finalize_ast),
        flatten_groups: flatten_groups_report_to_dto(&report.flatten_groups),
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
        body_mode: record.body_mode.as_str(),
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
    let kind = match error {
        Error::Parse(error) => {
            let message = error.to_string();
            let (document, diagnostics) = error.into_parts();
            return BindingErrorParts {
                error: BindingErrorDto {
                    diagnostics,
                    ..BindingErrorDto::new("parse", message)
                },
                document,
            };
        }
        Error::MissingProfile | Error::UnknownRule(_) | Error::TransformBuild(_) => "config",
        Error::KnowledgeBaseMismatch | Error::IncompleteTree | Error::Transform(_) => "transform",
        Error::Serialize(_) => "internal",
    };
    BindingErrorParts {
        error: BindingErrorDto::new(kind, error.to_string()),
        document: None,
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RuleInfoDto {
    pub key: String,
    pub level: &'static str,
    pub fidelity: &'static str,
    pub summary: String,
    pub enabled_by_packages: Vec<String>,
}

pub fn list_rules_to_dto() -> Vec<RuleInfoDto> {
    crate::list_rules()
        .into_iter()
        .map(|info| RuleInfoDto {
            key: info.key.to_string(),
            level: info.level.as_str(),
            fidelity: info.fidelity.as_str(),
            summary: info.summary,
            enabled_by_packages: info.enabled_by_packages,
        })
        .collect()
}

/// The structured part of a conformance failure; the message stays on [`BindingErrorDto`].
#[derive(Clone, Debug, serde::Serialize)]
pub struct ConformanceErrorDto {
    pub path: String,
    pub rule: &'static str,
}

fn conformance_error_to_dto(error: crate::ConformanceError) -> BindingErrorDto {
    let message = error.to_string();
    BindingErrorDto {
        conformance: Some(ConformanceErrorDto {
            path: error.path,
            rule: error.rule.as_str(),
        }),
        ..BindingErrorDto::new("conformance", message)
    }
}

pub fn from_syntax_error_to_dto(error: FromSyntaxError) -> BindingErrorDto {
    match error {
        FromSyntaxError::Conformance(error) => conformance_error_to_dto(error),
        _ => BindingErrorDto::new("parse", error.to_string()),
    }
}

pub fn edit_error_to_dto(error: EditError) -> BindingErrorDto {
    match error {
        EditError::Conformance(error) => conformance_error_to_dto(error),
        EditError::InvalidSource(diagnostics) => BindingErrorDto {
            diagnostics,
            ..BindingErrorDto::new("parse", "source fragment contains parse errors")
        },
        _ => BindingErrorDto::new("edit", error.to_string()),
    }
}

/// Serialize a syntax root after structural checks only, for the bindings' `serialize`.
///
/// It needs no knowledge base and does not check knowledge conformance.
pub fn serialize_syntax(
    node: &SyntaxNode,
    options: &SerializeOptions,
) -> Result<String, BindingErrorDto> {
    texform_core::document::Document::serialize_syntax(node, options)
        .map_err(from_syntax_error_to_dto)
}

/// The syntax form attached to a present argument value.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ArgKindDto {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close: Option<String>,
}

impl From<ArgKindRef<'_>> for ArgKindDto {
    fn from(kind: ArgKindRef<'_>) -> Self {
        let (open, close) = match kind {
            ArgKindRef::Until { close } => (None, Some(close.to_string())),
            ArgKindRef::Delimited { open, close } | ArgKindRef::Paired { open, close } => {
                (Some(open.to_string()), Some(close.to_string()))
            }
            _ => (None, None),
        };
        Self {
            kind: kind.as_str(),
            open,
            close,
        }
    }
}

/// A present argument with its form.
///
/// Content arguments carry `node`, which each binding exposes as a live node
/// handle; leaf arguments carry `value`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ArgRefDto {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    pub form: ArgKindDto,
    #[serde(skip)]
    pub node: Option<NodeId>,
}

/// Read argument `index` of `node`; `None` for an absent slot.
pub fn arg_ref_to_dto(node: NodeRef<'_>, index: usize) -> Option<ArgRefDto> {
    let form = node.arg_kind(index)?.into();
    let (kind, value, node) = match node.arg(index)? {
        ArgRef::Math(node) => ("Math", None, Some(node.id())),
        ArgRef::Text(node) => ("Text", None, Some(node.id())),
        ArgRef::OperatorName(node) => ("OperatorName", None, Some(node.id())),
        ArgRef::Delimiter(value) => ("Delimiter", Some(value.to_string().into()), None),
        ArgRef::CSName(value) => ("CSName", Some(value.into()), None),
        ArgRef::Dimension(value) => ("Dimension", Some(value.into()), None),
        ArgRef::Integer(value) => ("Integer", Some(value.into()), None),
        ArgRef::KeyVal(value) => ("KeyVal", Some(value.into()), None),
        ArgRef::Column(value) => ("Column", Some(value.into()), None),
        ArgRef::Boolean(value) => ("Boolean", Some(value.into()), None),
    };
    Some(ArgRefDto {
        kind,
        value,
        form,
        node,
    })
}

/// A group's kind; delimited groups carry their delimiter strings.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct GroupKindDto {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
}

impl From<GroupKindRef<'_>> for GroupKindDto {
    fn from(kind: GroupKindRef<'_>) -> Self {
        let (kind, delimiters) = match kind {
            GroupKindRef::Explicit => ("Explicit", None),
            GroupKindRef::Implicit => ("Implicit", None),
            GroupKindRef::Delimited { left, right } => ("Delimited", Some((left, right))),
            GroupKindRef::InlineMath => ("InlineMath", None),
        };
        Self {
            kind,
            left: delimiters.map(|(left, _)| left.to_string()),
            right: delimiters.map(|(_, right)| right.to_string()),
        }
    }
}

fn command_kind_to_dto_key(kind: texform_core::parse::CommandKind) -> &'static str {
    match kind {
        texform_core::parse::CommandKind::Prefix => "prefix",
        texform_core::parse::CommandKind::Infix => "infix",
        texform_core::parse::CommandKind::Declarative => "declarative",
    }
}

fn finalize_ast_report_to_dto(report: &FinalizeAstReport) -> FinalizeAstReportDto {
    FinalizeAstReportDto {
        prime_run_merges: report.prime_run_merges,
        text_normalizations: report.text_normalizations,
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
        guard_hits: FlattenGroupsGuardCountsDto {
            unknown_command_arguments: report.guard_hits.unknown_command_arguments,
            declarative_scope: report.guard_hits.declarative_scope,
            script_base: report.guard_hits.script_base,
            env_body: report.guard_hits.env_body,
            infix_scope: report.guard_hits.infix_scope,
            command_contact: report.guard_hits.command_contact,
            command_contact_via_scripted_base: report.guard_hits.command_contact_via_scripted_base,
            empty_group: report.guard_hits.empty_group,
            lone_atom_spacing_char: report.guard_hits.lone_atom_spacing_char,
            leading_atom_spacing_char: report.guard_hits.leading_atom_spacing_char,
            delimited_pair: report.guard_hits.delimited_pair,
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
    fn transform_report_to_dto_reads_rewrite_report() {
        let mut report = crate::diagnostics::TransformReport::default();
        let rules = texform_transform::rewrite::all_rules();
        let later = rules[0].meta().key;
        let earlier = rules[1].meta().key;
        report.rewrite.iterations = 3;
        report
            .rewrite
            .rules
            .push(texform_transform::rewrite::RewriteRuleStat {
                key: later,
                applied_count: 2,
                skipped_count: 1,
            });
        report
            .rewrite
            .rules
            .push(texform_transform::rewrite::RewriteRuleStat {
                key: earlier,
                applied_count: 4,
                skipped_count: 0,
            });

        let dto = transform_report_to_dto(&report);
        let mut expected = [later.to_string(), earlier.to_string()];
        expected.sort();

        assert_eq!(dto.rewrite.iterations, 3);
        assert_eq!(dto.rewrite.rules.len(), 2);
        assert_eq!(dto.rewrite.rules[0].key, expected[0]);
        assert_eq!(dto.rewrite.rules[1].key, expected[1]);
        let applied = dto
            .rewrite
            .rules
            .iter()
            .find(|rule| rule.key == later.to_string())
            .expect("later rule");
        assert_eq!(applied.applied_count, 2);
        assert_eq!(applied.skipped_count, 1);
    }

    #[test]
    fn transform_report_to_dto_reads_finalize_ast_report() {
        let mut report = crate::diagnostics::TransformReport::default();
        report.finalize_ast.prime_run_merges = 4;
        report.finalize_ast.text_normalizations = 2;

        let dto = transform_report_to_dto(&report);
        let json = serde_json::to_value(&dto).unwrap();

        assert_eq!(dto.finalize_ast.prime_run_merges, 4);
        assert_eq!(dto.finalize_ast.text_normalizations, 2);
        assert_eq!(json["finalize_ast"]["prime_run_merges"], 4);
        assert_eq!(json["finalize_ast"]["text_normalizations"], 2);
        assert!(json["finalize_ast"].get("steps").is_none());
    }

    #[test]
    fn transform_report_to_dto_groups_flatten_groups_report() {
        let mut report = crate::diagnostics::TransformReport::default();
        report.flatten_groups.actions = texform_transform::FlattenGroupsActionCounts {
            removed_empty: 1,
            replaced_single_child: 2,
            inlined_multi_child: 3,
            unwrapped_slot: 4,
        };
        report.flatten_groups.guard_hits = texform_transform::FlattenGroupsGuardCounts {
            unknown_command_arguments: 3,
            declarative_scope: 5,
            script_base: 6,
            env_body: 7,
            infix_scope: 8,
            command_contact: 9,
            command_contact_via_scripted_base: 11,
            empty_group: 12,
            lone_atom_spacing_char: 13,
            leading_atom_spacing_char: 14,
            delimited_pair: 15,
        };

        let dto = transform_report_to_dto(&report).flatten_groups;

        assert_eq!(dto.actions.removed_empty, 1);
        assert_eq!(dto.actions.replaced_single_child, 2);
        assert_eq!(dto.actions.inlined_multi_child, 3);
        assert_eq!(dto.actions.unwrapped_slot, 4);
        assert_eq!(dto.guard_hits.declarative_scope, 5);
        assert_eq!(dto.guard_hits.script_base, 6);
        assert_eq!(dto.guard_hits.env_body, 7);
        assert_eq!(dto.guard_hits.infix_scope, 8);
        assert_eq!(dto.guard_hits.command_contact, 9);
        assert_eq!(dto.guard_hits.command_contact_via_scripted_base, 11);
        assert_eq!(dto.guard_hits.empty_group, 12);
        assert_eq!(dto.guard_hits.lone_atom_spacing_char, 13);
        assert_eq!(dto.guard_hits.leading_atom_spacing_char, 14);
        assert_eq!(dto.guard_hits.delimited_pair, 15);
    }

    #[test]
    fn transform_report_to_dto_reads_lower_attributes_report_in_stable_order() {
        let mut report = crate::diagnostics::TransformReport::default();
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

/// Host-language description of a node's immediate parent slot.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NodeSlotDto {
    pub kind: &'static str,
    pub index: Option<usize>,
}

impl From<crate::NodeSlot> for NodeSlotDto {
    fn from(slot: crate::NodeSlot) -> Self {
        Self {
            kind: slot.as_str(),
            index: slot.index(),
        }
    }
}
