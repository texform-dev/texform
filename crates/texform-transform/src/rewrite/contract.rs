//! Eliminated-form contract checked after the full transform pipeline.

use crate::ast::Ast;
use crate::parse::ParseContext;
use crate::rewrite::rule::RuleTargetKey;
use crate::rewrite::scheduler::{node_name_for_target, target_present};

/// A single eliminated-form contract violation found in an AST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractViolation {
    /// The eliminated form that is still present.
    pub target: RuleTargetKey,
    /// Best-effort node name for human-facing diagnostics.
    pub node_name: Option<String>,
}

impl std::fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "contract violation for {} `{}` (node {:?})",
            self.target.kind_label(),
            self.target.name,
            self.node_name
        )
    }
}

/// Collects all eliminated-form contract violations currently present in `ast`.
pub fn collect_eliminated_violations(
    ast: &Ast,
    parse_ctx: &ParseContext,
    eliminated_forms: &[RuleTargetKey],
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();

    for node_id in ast.find_all(ast.root(), |_| true) {
        for target in eliminated_forms {
            if target_present(ast, node_id, *target, parse_ctx) {
                let node_name = node_name_for_target(ast, node_id);
                violations.push(ContractViolation {
                    target: *target,
                    node_name,
                });
            }
        }
    }

    violations
}
