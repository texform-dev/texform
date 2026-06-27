//! Aggregate transform report.

use crate::finalize_ast::FinalizeAstReport;
use crate::flatten_groups::FlattenGroupsReport;
use crate::lower_attributes::LowerAttributesReport;
use crate::rewrite::RewriteReport;

/// Phase-oriented summary of a single transform run.
///
/// Each field reports what its phase changed in the tree, in pipeline order:
/// pre/post LowerAttributes counts are aggregated into one bucket, then
/// Rewrite, FinalizeAst, and FlattenGroups. This is the Rust-native report;
/// the Python and WebAssembly bindings flatten the same data into a transport
/// DTO.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransformReport {
    /// Attribute canonicalization counts, summed over the pre- and
    /// post-Rewrite LowerAttributes passes.
    pub lower_attributes: LowerAttributesReport,
    /// Fixed-point iteration count and per-rule application counts.
    pub rewrite: RewriteReport,
    /// Local AST cleanup counts (currently adjacent-`Prime` merging).
    pub finalize_ast: FinalizeAstReport,
    /// Group-flattening action counts and per-guard hit counts.
    pub flatten_groups: FlattenGroupsReport,
}
