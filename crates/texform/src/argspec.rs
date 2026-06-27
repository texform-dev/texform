//! Validation of xparse-style argument specifications.
//!
//! TeXForm describes every command's argument structure with an xparse-style
//! signature (mandatory, optional, delimited, starred, and similar argument
//! kinds). [`validate_argspec`] parses such a signature string and reports
//! whether it is well-formed, together with a structured, serde-serializable
//! description of each parsed slot. The `*Info` types here are a stable,
//! presentation-oriented view of the internal `texform-argspec` model, so that
//! tooling and the language bindings can consume argspecs without depending on
//! internal crates.

use texform_argspec::{ArgForm, ArgSpec, DelimiterToken, ValueKind};
use texform_interface::syntax_node::ContentMode;

/// Outcome of validating an argument-specification string.
///
/// On success, `valid` is `true` and `arg_count` / `parsed` describe the slots;
/// on failure, `valid` is `false` and `error` carries the parse error message.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ValidateArgspecResult {
    /// Whether the specification parsed successfully.
    pub valid: bool,
    /// Human-readable error message when `valid` is `false`, otherwise `None`.
    pub error: Option<String>,
    /// Number of argument slots when valid, otherwise `None`.
    pub arg_count: Option<usize>,
    /// Per-slot breakdown when valid, otherwise `None`.
    pub parsed: Option<Vec<ParsedArgSpecSlot>>,
}

/// One parsed argument slot of a specification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ParsedArgSpecSlot {
    /// Whether the argument is mandatory (`true`) or optional (`false`).
    pub required: bool,
    /// Whether the argument may not be preceded by whitespace.
    pub no_leading_space: bool,
    /// Whether an absent optional argument is reported as a null value.
    pub nullable: bool,
    /// The value kind the slot accepts (content, delimiter, integer, ...).
    pub kind: ArgSpecKindInfo,
    /// The syntactic form the slot takes (group, delimited, starred, ...).
    pub form: ArgSpecFormInfo,
}

/// The kind of value an argument slot accepts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ArgSpecKindInfo {
    /// Arbitrary content parsed in a given mode (math or text).
    Content {
        /// The content mode the argument body is parsed in.
        mode: RuntimeContentModeInfo,
    },
    /// A single delimiter token.
    Delimiter,
    /// A control-sequence name.
    #[serde(rename = "csname")]
    CsName,
    /// A TeX dimension (such as `2pt`).
    Dimension,
    /// An integer literal.
    Integer,
    /// A `key=value` list.
    #[serde(rename = "keyval")]
    KeyVal,
    /// A tabular column specification.
    Column,
    /// A star flag (`*`).
    Star,
}

/// Content mode an argument body is parsed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeContentModeInfo {
    /// Math mode.
    Math,
    /// Text mode.
    Text,
}

/// The syntactic form an argument slot takes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ArgSpecFormInfo {
    /// A standard mandatory/optional argument (`m`, `o`, ...).
    Standard,
    /// A star flag.
    Star,
    /// A brace-group argument.
    Group,
    /// An argument bounded by an explicit open/close delimiter pair.
    Delimited {
        /// The opening delimiter token.
        open: DelimiterTokenInfo,
        /// The closing delimiter token.
        close: DelimiterTokenInfo,
    },
    /// An argument accepting any of several interchangeable delimiter pairs.
    Paired {
        /// The accepted open/close delimiter pairs.
        pairs: Vec<DelimiterTokenPairInfo>,
    },
}

/// A single delimiter token in an argument form.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DelimiterTokenInfo {
    /// A literal character delimiter (such as `(` or `[`).
    Char {
        /// The delimiter character.
        value: char,
    },
    /// A control-sequence delimiter (such as `\langle`).
    ControlSeq {
        /// The control-sequence name, without the leading backslash.
        value: String,
    },
}

/// An open/close delimiter pair accepted by a [`Paired`](ArgSpecFormInfo::Paired) form.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DelimiterTokenPairInfo {
    /// The opening delimiter token.
    pub open: DelimiterTokenInfo,
    /// The closing delimiter token.
    pub close: DelimiterTokenInfo,
}

/// Validate an xparse-style argument-specification string.
///
/// Returns a [`ValidateArgspecResult`] describing whether `spec` is well-formed
/// and, when it is, the structured breakdown of each argument slot. This never
/// fails by returning `Err`: a malformed specification is reported through the
/// `valid` / `error` fields of the result.
///
/// # Examples
///
/// ```
/// let result = texform::validate_argspec("m o");
/// assert!(result.valid);
/// assert_eq!(result.arg_count, Some(2));
///
/// let bad = texform::validate_argspec("m {");
/// assert!(!bad.valid);
/// assert!(bad.error.is_some());
/// ```
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

/// Convert one internal [`ArgSpec`] into its presentation-oriented
/// [`ParsedArgSpecSlot`] view.
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
