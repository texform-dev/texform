//! Compiled transform execution context.

use crate::ast::Ast;
use crate::config::TransformConfig;
use crate::engine;
use crate::error::{TransformBuildError, TransformError};
use crate::parse::ParseContext;
use crate::report::TransformReport;
use crate::rewrite;

pub struct TransformContext {
    config: TransformConfig,
    rewrite: Option<rewrite::Plan>,
}

impl TransformContext {
    pub fn from_config(
        config: TransformConfig,
        parse_ctx: &ParseContext,
    ) -> Result<Self, TransformBuildError> {
        let rewrite = if config.rewrite.enabled {
            Some(
                rewrite::Plan::build(&config.rewrite, parse_ctx)
                    .map_err(TransformBuildError::Rewrite)?,
            )
        } else {
            None
        };
        Ok(Self { config, rewrite })
    }

    pub fn run(
        &self,
        ast: &mut Ast,
        parse_ctx: &ParseContext,
    ) -> Result<TransformReport, TransformError> {
        engine::execute(self, ast, parse_ctx)
    }

    pub fn config(&self) -> &TransformConfig {
        &self.config
    }

    pub fn rewrite_plan(&self) -> Option<&rewrite::Plan> {
        self.rewrite.as_ref()
    }
}
