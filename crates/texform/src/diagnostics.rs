//! Diagnostic transform reports.
//!
//! Output text and errors follow the ordinary transform contract. Report
//! fields, phase divisions, and detailed counters are diagnostic: they are
//! not part of the stable compatibility promise. The recorder's accumulation
//! methods stay inside the transform crate.

pub use texform_transform::{
    Attr, AttrValue, AttributeFormCounts, AttributeSet, AttributeStat, FinalizeAstReport,
    FlattenGroupsActionCounts, FlattenGroupsGuardCounts, FlattenGroupsReport,
    LowerAttributesReport, MathFontValue, RewriteReport, RewriteRuleStat, SizeValue, StyleValue,
    TextFamily, TextSeries, TextShape, TransformReport,
};

/// Normalized text plus the diagnostic report from one explicit report call.
#[derive(Debug)]
pub struct NormalizeReportResult {
    /// Serialized LaTeX after parsing and normalization.
    pub normalized: String,
    /// Diagnostic report for this call. It is not an empty placeholder from a
    /// plain normalize.
    pub report: TransformReport,
}
