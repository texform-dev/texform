//! TeXForm transform crate: phase-oriented AST rewrite pipeline.

pub(crate) use texform_core::ast;
pub(crate) use texform_core::knowledge;
pub(crate) use texform_core::parse;
#[cfg(test)]
pub(crate) use texform_core::serialize;

pub mod config;
pub mod context;
mod engine;
pub mod error;
pub mod flatten_groups;
pub mod lower_attributes;
pub mod report;
pub mod rewrite;

pub use config::{RewriteConfig, TransformConfig};
pub use context::TransformContext;
pub use error::{TransformBuildError, TransformError};
pub use flatten_groups::{FlattenGroupsConfig, FlattenGroupsReport};
pub use lower_attributes::{
    Attr, AttrValue, LowerAttributesConfig, LowerAttributesReport, SizeValue, StyleValue,
};
pub use report::TransformReport;
pub use rewrite::{
    AppliedRuleStat, PackageName, Plan as RewritePlan, PlanBuildError, RewriteError, RewriteReport,
    RewriteRule, RuleAvailabilityFailure, RuleClass, RuleClassSet, RuleConsumes, RuleEffect,
    RuleError, RuleKey, RuleMeta, RuleProduces, RuleSafety, RuleSelection, RuleTarget,
    RuleTargetKey, RuleTargetKind,
};

use crate::ast::Ast;
use crate::parse::ParseContext;

/// One-shot transform that builds a context and runs it once.
pub fn run(
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    config: &TransformConfig,
) -> Result<TransformReport, TransformError> {
    let context =
        TransformContext::from_config(config.clone(), parse_ctx).map_err(TransformError::Build)?;
    context.run(ast, parse_ctx)
}
