//! Compiled transform execution context.

use crate::ast::Ast;
use crate::config::{BuildConfig, TransformConfig};
use crate::engine;
use crate::error::{TransformBuildError, TransformError};
use crate::parse::ParseContext;
use crate::report::{ReportRecorder, TransformReport};
use crate::rewrite;

pub struct TransformContext {
    default_config: TransformConfig,
    rewrite: rewrite::Plan,
}

impl TransformContext {
    pub fn from_build_config(
        config: BuildConfig,
        parse_ctx: &ParseContext,
    ) -> Result<Self, TransformBuildError> {
        let default_config = config.default_transform();
        let rewrite =
            rewrite::Plan::build(&config, parse_ctx).map_err(TransformBuildError::Rewrite)?;
        Ok(Self {
            default_config,
            rewrite,
        })
    }

    pub fn run(&self, ast: &mut Ast, parse_ctx: &ParseContext) -> Result<(), TransformError> {
        self.run_with(ast, parse_ctx, &self.default_config)
    }

    pub fn run_with(
        &self,
        ast: &mut Ast,
        parse_ctx: &ParseContext,
        config: &TransformConfig,
    ) -> Result<(), TransformError> {
        let mut recorder = ReportRecorder::disabled();
        engine::execute(self, ast, parse_ctx, config, None, &mut recorder)
    }

    /// Execute the pipeline and return the diagnostic report for this call.
    ///
    /// Pass [`Self::default_config`] when the profile defaults should apply.
    /// A failed run does not return a partial report.
    pub fn run_with_report(
        &self,
        ast: &mut Ast,
        parse_ctx: &ParseContext,
        config: &TransformConfig,
    ) -> Result<TransformReport, TransformError> {
        let mut recorder = ReportRecorder::collecting();
        engine::execute(self, ast, parse_ctx, config, None, &mut recorder)?;
        Ok(recorder.into_report())
    }

    /// Run with a sparse FlattenGroups research overlay.
    ///
    /// Unstable: this entry is for internal experiments and may change without
    /// notice. It always collects a report. `enabled=false` still skips
    /// FlattenGroups; the overlay cannot re-enable the phase.
    pub fn run_with_flatten_groups_guards(
        &self,
        ast: &mut Ast,
        parse_ctx: &ParseContext,
        config: &TransformConfig,
        overlay: &crate::flatten_groups::FlattenGroupsGuardsOverlay,
    ) -> Result<TransformReport, TransformError> {
        let mut recorder = ReportRecorder::collecting();
        engine::execute(self, ast, parse_ctx, config, Some(overlay), &mut recorder)?;
        Ok(recorder.into_report())
    }

    pub fn default_config(&self) -> &TransformConfig {
        &self.default_config
    }

    pub fn rewrite_plan(&self) -> &rewrite::Plan {
        &self.rewrite
    }

    #[cfg(test)]
    pub(crate) fn from_rewrite_plan_for_tests(
        default_config: TransformConfig,
        rewrite: rewrite::Plan,
    ) -> Self {
        Self {
            default_config,
            rewrite,
        }
    }
}
