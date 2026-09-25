//! Aggregate transform report and the call-scoped collector.

use crate::finalize_ast::FinalizeAstReport;
use crate::flatten_groups::FlattenGroupsReport;
use crate::lower_attributes::LowerAttributesReport;
use crate::rewrite::{RewriteReport, RuleKey};

/// Phase-oriented summary of a single transform run.
///
/// Each field aggregates what its phase observed across all scheduling rounds.
/// The phase buckets remain LowerAttributes, Rewrite, FinalizeAst, and
/// FlattenGroups. Counters are not a single "number of output changes" and
/// must not be added together. This is the Rust-native report; bindings
/// transport the same hierarchy through a DTO. Field layout and detailed
/// statistics are diagnostic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransformReport {
    /// Attribute canonicalization counts, summed over all invocations.
    pub lower_attributes: LowerAttributesReport,
    /// Fixed-point iteration count and per-rule application counts.
    pub rewrite: RewriteReport,
    /// Counts for local AST canonicalization steps.
    pub finalize_ast: FinalizeAstReport,
    /// Group-flattening action counts and per-guard hit counts.
    pub flatten_groups: FlattenGroupsReport,
}

/// Call-scoped collector. `None` skips report-only scans, container inserts,
/// and report strings; the transform algorithm does not branch on it.
pub struct ReportRecorder {
    report: Option<TransformReport>,
}

impl ReportRecorder {
    /// Collector for a call that does not request a report.
    pub fn disabled() -> Self {
        Self { report: None }
    }

    /// Empty collector for a call that requests a report.
    pub fn collecting() -> Self {
        Self {
            report: Some(TransformReport::default()),
        }
    }

    /// Report accumulated by a collecting recorder.
    ///
    /// # Panics
    ///
    /// Panics if this recorder was created with [`Self::disabled`]. Callers
    /// extract a report only after a successful collecting run.
    pub fn into_report(self) -> TransformReport {
        self.report
            .expect("into_report requires a collecting recorder")
    }

    /// Run `body` only when this call is collecting a LowerAttributes report.
    ///
    /// `body` must not change the AST or other transform state. It is not
    /// stored or boxed.
    #[inline]
    pub(crate) fn lower_attributes(&mut self, body: impl FnOnce(&mut LowerAttributesReport)) {
        if let Some(report) = &mut self.report {
            body(&mut report.lower_attributes);
        }
    }

    /// Run `body` only when this call is collecting a Rewrite report.
    #[inline]
    pub(crate) fn rewrite(&mut self, body: impl FnOnce(&mut RewriteReport)) {
        if let Some(report) = &mut self.report {
            body(&mut report.rewrite);
        }
    }

    /// Run `body` only when this call is collecting a FinalizeAst report.
    #[inline]
    pub(crate) fn finalize_ast(&mut self, body: impl FnOnce(&mut FinalizeAstReport)) {
        if let Some(report) = &mut self.report {
            body(&mut report.finalize_ast);
        }
    }

    #[inline]
    pub(crate) fn flatten_groups(&mut self, body: impl FnOnce(&mut FlattenGroupsReport)) {
        if let Some(report) = &mut self.report {
            body(&mut report.flatten_groups);
        }
    }

    #[inline]
    pub(crate) fn record_rule_applied(&mut self, key: RuleKey) {
        self.rewrite(|report| report.mark_rule_applied(key));
    }

    #[inline]
    pub(crate) fn record_rule_skipped(&mut self, key: RuleKey) {
        self.rewrite(|report| report.mark_rule_skipped(key));
    }

    #[inline]
    pub(crate) fn record_rewrite_iterations(&mut self, iterations: usize) {
        self.rewrite(|report| report.record_iteration(iterations));
    }

    #[inline]
    pub(crate) fn record_prime_run_merge(&mut self) {
        self.finalize_ast(|report| report.prime_run_merges += 1);
    }

    #[inline]
    pub(crate) fn record_text_normalization(&mut self) {
        self.finalize_ast(|report| report.text_normalizations += 1);
    }
}
