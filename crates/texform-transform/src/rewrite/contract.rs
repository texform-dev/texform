//! Post-rewrite contract: forms that should no longer appear in the AST.

use crate::ast::Ast;
use crate::parse::Parser;
use crate::rewrite::RewriteError;
use crate::rewrite::rule::RuleTargetKey;
use crate::rewrite::scheduler::{node_name_for_target, target_present};

pub(super) fn assert_eliminated_forms(
    ast: &Ast,
    parse_ctx: &Parser,
    eliminated_forms: &[RuleTargetKey],
) -> Result<(), RewriteError> {
    for node_id in ast.find_all(ast.root(), |_| true) {
        for target in eliminated_forms {
            if target_present(ast, node_id, *target, parse_ctx) {
                let node_name = node_name_for_target(ast, node_id);
                return Err(RewriteError::ContractViolation {
                    target: *target,
                    node_name,
                });
            }
        }
    }
    Ok(())
}
