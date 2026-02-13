use serde::{Deserialize, Serialize};

/// Parsed column template.
///
/// This structure mirrors the output-oriented state from MathJax ColumnParser.
/// It stores both the raw template and parsed structural data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ColumnSpec {
    pub template_raw: String,
    pub template_normalized: String,
    pub column_align: Vec<ColumnAlign>,
    pub column_width: Vec<String>,
    pub column_spacing: Vec<String>,
    pub column_lines: Vec<LineStyle>,
    pub frame: Vec<FrameLine>,
    pub column_start: Vec<String>,
    pub column_end: Vec<String>,
    pub column_extra: Vec<bool>,
    pub row_align: Vec<Option<RowAlign>>,
    pub array_padding: Option<ArrayPadding>,
}

impl ColumnSpec {
    pub fn new(template_raw: String, template_normalized: String) -> Self {
        ColumnSpec {
            template_raw,
            template_normalized,
            column_align: Vec::new(),
            column_width: Vec::new(),
            column_spacing: Vec::new(),
            column_lines: Vec::new(),
            frame: Vec::new(),
            column_start: Vec::new(),
            column_end: Vec::new(),
            column_extra: Vec::new(),
            row_align: Vec::new(),
            array_padding: None,
        }
    }
}

impl std::fmt::Display for ColumnSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.template_normalized)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum ColumnAlign {
    Left,
    Center,
    Right,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum LineStyle {
    None,
    Solid,
    Dashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum FrameSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct FrameLine {
    pub side: FrameSide,
    pub style: LineStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct RowAlign {
    pub vertical: VerticalAlign,
    pub width: String,
    pub align: ColumnAlign,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ArrayPadding {
    pub left: String,
    pub right: String,
}
