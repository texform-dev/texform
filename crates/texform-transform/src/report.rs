//! Aggregate transform report.

use crate::flatten_groups::FlattenGroupsReport;
use crate::lower_attributes::LowerAttributesReport;
use crate::rewrite::RewriteReport;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransformReport {
    pub lower_attributes: LowerAttributesReport,
    pub rewrite: RewriteReport,
    pub flatten_groups: FlattenGroupsReport,
}
