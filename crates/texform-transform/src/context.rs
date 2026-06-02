//! Compiled transform execution context.

use crate::ast::Ast;
use crate::config::{BuildConfig, TransformConfig};
use crate::engine;
use crate::error::{TransformBuildError, TransformError};
use crate::parse::ParseContext;
use crate::report::TransformReport;
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

    pub fn run(
        &self,
        ast: &mut Ast,
        parse_ctx: &ParseContext,
    ) -> Result<TransformReport, TransformError> {
        self.run_with(ast, parse_ctx, &self.default_config)
    }

    pub fn run_with(
        &self,
        ast: &mut Ast,
        parse_ctx: &ParseContext,
        config: &TransformConfig,
    ) -> Result<TransformReport, TransformError> {
        engine::execute(self, ast, parse_ctx, config)
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
