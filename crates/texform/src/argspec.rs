use texform_argspec::{ArgForm, ArgSpec, DelimiterToken, ValueKind};
use texform_interface::syntax_node::ContentMode;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ValidateArgspecResult {
    pub valid: bool,
    pub error: Option<String>,
    pub arg_count: Option<usize>,
    pub parsed: Option<Vec<ParsedArgSpecSlot>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ParsedArgSpecSlot {
    pub required: bool,
    pub no_leading_space: bool,
    pub nullable: bool,
    pub kind: ArgSpecKindInfo,
    pub form: ArgSpecFormInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ArgSpecKindInfo {
    Content {
        mode: RuntimeContentModeInfo,
    },
    Delimiter,
    #[serde(rename = "csname")]
    CsName,
    Dimension,
    Integer,
    #[serde(rename = "keyval")]
    KeyVal,
    Column,
    Star,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeContentModeInfo {
    Math,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ArgSpecFormInfo {
    Standard,
    Star,
    Group,
    Delimited {
        open: DelimiterTokenInfo,
        close: DelimiterTokenInfo,
    },
    Paired {
        pairs: Vec<DelimiterTokenPairInfo>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DelimiterTokenInfo {
    Char { value: char },
    ControlSeq { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DelimiterTokenPairInfo {
    pub open: DelimiterTokenInfo,
    pub close: DelimiterTokenInfo,
}

pub fn validate_argspec(spec: &str) -> ValidateArgspecResult {
    match texform_argspec::parse_arg_specs(spec, "validate_argspec") {
        Ok(parsed) => ValidateArgspecResult {
            valid: true,
            error: None,
            arg_count: Some(parsed.len()),
            parsed: Some(parsed.iter().map(parsed_arg_spec_slot).collect()),
        },
        Err(error) => ValidateArgspecResult {
            valid: false,
            error: Some(error.to_string()),
            arg_count: None,
            parsed: None,
        },
    }
}

pub fn parsed_arg_spec_slot(spec: &ArgSpec) -> ParsedArgSpecSlot {
    ParsedArgSpecSlot {
        required: spec.required,
        no_leading_space: spec.no_leading_space,
        nullable: spec.nullable,
        kind: arg_spec_kind_info(spec.kind),
        form: arg_spec_form_info(&spec.form),
    }
}

fn arg_spec_kind_info(kind: ValueKind) -> ArgSpecKindInfo {
    match kind {
        ValueKind::Content { mode } => ArgSpecKindInfo::Content {
            mode: runtime_content_mode_info(mode),
        },
        ValueKind::Delimiter => ArgSpecKindInfo::Delimiter,
        ValueKind::CSName => ArgSpecKindInfo::CsName,
        ValueKind::Dimension => ArgSpecKindInfo::Dimension,
        ValueKind::Integer => ArgSpecKindInfo::Integer,
        ValueKind::KeyVal => ArgSpecKindInfo::KeyVal,
        ValueKind::Column => ArgSpecKindInfo::Column,
        ValueKind::Star => ArgSpecKindInfo::Star,
    }
}

fn runtime_content_mode_info(mode: ContentMode) -> RuntimeContentModeInfo {
    match mode {
        ContentMode::Math => RuntimeContentModeInfo::Math,
        ContentMode::Text => RuntimeContentModeInfo::Text,
    }
}

fn arg_spec_form_info(form: &ArgForm) -> ArgSpecFormInfo {
    match form {
        ArgForm::Standard => ArgSpecFormInfo::Standard,
        ArgForm::Star => ArgSpecFormInfo::Star,
        ArgForm::Group => ArgSpecFormInfo::Group,
        ArgForm::Delimited { open, close } => ArgSpecFormInfo::Delimited {
            open: delimiter_token_info(open),
            close: delimiter_token_info(close),
        },
        ArgForm::Paired { pairs } => ArgSpecFormInfo::Paired {
            pairs: pairs
                .iter()
                .map(|(open, close)| DelimiterTokenPairInfo {
                    open: delimiter_token_info(open),
                    close: delimiter_token_info(close),
                })
                .collect(),
        },
    }
}

fn delimiter_token_info(token: &DelimiterToken) -> DelimiterTokenInfo {
    match token {
        DelimiterToken::Char(value) => DelimiterTokenInfo::Char { value: *value },
        DelimiterToken::ControlSeq(value) => DelimiterTokenInfo::ControlSeq {
            value: value.to_string(),
        },
    }
}
