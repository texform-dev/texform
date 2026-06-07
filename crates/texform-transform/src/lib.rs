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
pub mod finalize_ast;
pub mod flatten_groups;
pub mod lower_attributes;
pub mod report;
pub mod rewrite;

pub use config::{BuildConfig, NormalizeConfig, Profile, TransformConfig};
pub use context::TransformContext;
pub use error::{TransformBuildError, TransformError};
pub use finalize_ast::{
    FinalizeAstConfig, FinalizeAstReport, FinalizeAstStepReport, FinalizeAstStepReports,
};
pub use flatten_groups::{
    FlattenGroupsActionCounts, FlattenGroupsConfig, FlattenGroupsGuardCounts, FlattenGroupsReport,
};
pub use lower_attributes::{
    Attr, AttrValue, AttributeFormCounts, AttributeSet, AttributeStat, LowerAttributesConfig,
    LowerAttributesReport, MathFontValue, SizeValue, StyleValue, TextFamily, TextSeries, TextShape,
};
pub use report::TransformReport;
pub use rewrite::{
    ContractViolation, NormalizationLevel, NormalizationLevelSet, PackageName, Plan as RewritePlan,
    PlanBuildError, RewriteError, RewriteReport, RewriteRule, RewriteRuleStat,
    RuleAvailabilityFailure, RuleConsumes, RuleEffect, RuleError, RuleFidelity, RuleKey, RuleMeta,
    RuleProduces, RuleTarget, RuleTargetKey, RuleTargetKind, collect_eliminated_violations,
};

#[cfg(test)]
pub(crate) fn parse_to_ast_for_test(
    parse_ctx: &parse::ParseContext,
    src: &str,
    config: &parse::ParseConfig,
) -> ast::Ast {
    let document = parse_ctx
        .parse(src, config)
        .try_into_document()
        .expect("test input should parse")
        .0;
    ast::Ast::from_syntax_root(&document.to_syntax())
}
