//! Transform engine that applies configured phases to an AST.
//!
//! The engine executes in four ordered steps:
//!
//! 1. **LowerAttributes** rewrites registered declarative-scope commands before
//!    ordinary rewrite execution.
//! 2. **Rewrite** runs transform rules in a fixed-point loop until the AST
//!    stabilizes (no rule fires) or the iteration limit is reached.
//! 3. **LowerAttributes** normalizes attribute prefixes created by Rewrite.
//! 4. **FlattenGroups** removes redundant grouping once earlier phases are complete.
//!
//! After these steps, the Rewrite phase validates the resulting AST against the
//! eliminated-form contract derived into [`TransformContext`].

use crate::ast::Ast;
use crate::config::TransformConfig;
use crate::context::TransformContext;
use crate::error::TransformError;
use crate::lower_attributes::LowerAttributesConfig;
use crate::parse::Parser;
use crate::report::TransformReport;
use crate::{flatten_groups, lower_attributes, rewrite};

pub(crate) fn execute(
    tctx: &TransformContext,
    ast: &mut Ast,
    parse_ctx: &Parser,
    cfg: &TransformConfig,
) -> Result<TransformReport, TransformError> {
    let mut report = TransformReport::default();

    if cfg.lower_attributes_enabled {
        lower_attributes::run(
            ast,
            &LowerAttributesConfig::ENABLED,
            &mut report.lower_attributes,
        );
    }

    if cfg.rewrite_enabled {
        rewrite::run(
            ast,
            parse_ctx,
            tctx.rewrite_plan(),
            cfg.max_iterations,
            &mut report.rewrite,
        )
        .map_err(TransformError::Rewrite)?;
    }

    if cfg.lower_attributes_enabled {
        lower_attributes::run(
            ast,
            &LowerAttributesConfig::ENABLED,
            &mut report.lower_attributes,
        );
    }

    if cfg.flatten_groups.enabled {
        flatten_groups::run(ast, &cfg.flatten_groups, &mut report.flatten_groups);
    }

    Ok(report)
}
