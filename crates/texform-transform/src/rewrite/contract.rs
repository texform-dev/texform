//! Eliminated-form contract checked after the full transform pipeline.

use std::ops::ControlFlow;

use rustc_hash::FxHashMap;

use crate::ast::{Ast, Node};
use crate::parse::ParseContext;
use crate::rewrite::rule::{RuleTargetKey, RuleTargetKind};

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

/// Groups eliminated targets by target name while preserving slice order within
/// each bucket.
pub(super) fn index_eliminated_by_name(
    eliminated_forms: &[RuleTargetKey],
) -> FxHashMap<&'static str, Vec<RuleTargetKey>> {
    let mut by_name: FxHashMap<&'static str, Vec<RuleTargetKey>> = FxHashMap::default();
    for &target in eliminated_forms {
        by_name.entry(target.name).or_default().push(target);
    }
    by_name
}

/// Returns the first eliminated-form contract violation in AST preorder,
/// short-circuiting as soon as a violation is found.
pub(crate) fn find_first_eliminated_violation(
    ast: &Ast,
    parse_ctx: &ParseContext,
    eliminated_by_name: &FxHashMap<&'static str, Vec<RuleTargetKey>>,
) -> Option<ContractViolation> {
    if eliminated_by_name.is_empty() {
        return None;
    }

    match ast.try_visit_preorder(ast.root(), &mut |id| {
        let node = ast.node(id);
        let Some(name) = node_target_name(node) else {
            return ControlFlow::Continue(());
        };
        if let Some(candidates) = eliminated_by_name.get(name) {
            for &target in candidates {
                if target_present_on_node(node, name, target, parse_ctx) {
                    return ControlFlow::Break(ContractViolation {
                        target,
                        node_name: Some(name.to_string()),
                    });
                }
            }
        }
        ControlFlow::Continue(())
    }) {
        ControlFlow::Break(violation) => Some(violation),
        ControlFlow::Continue(()) => None,
    }
}

/// Collects all eliminated-form contract violations currently present in `ast`.
pub fn collect_eliminated_violations(
    ast: &Ast,
    parse_ctx: &ParseContext,
    eliminated_forms: &[RuleTargetKey],
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    if eliminated_forms.is_empty() {
        return violations;
    }

    let indexed = (eliminated_forms.len() > 4).then(|| index_eliminated_by_name(eliminated_forms));
    ast.visit_preorder(ast.root(), |id| {
        let node = ast.node(id);
        let Some(name) = node_target_name(node) else {
            return;
        };
        let candidates = match &indexed {
            Some(by_name) => by_name.get(name).map_or(&[][..], Vec::as_slice),
            None => eliminated_forms,
        };
        for &target in candidates {
            if target.name == name && target_present_on_node(node, name, target, parse_ctx) {
                violations.push(ContractViolation {
                    target,
                    node_name: Some(name.to_string()),
                });
            }
        }
    });

    violations
}

#[inline]
fn node_target_name(node: &Node) -> Option<&str> {
    match node {
        Node::Command { name, .. }
        | Node::Infix { name, .. }
        | Node::Declarative { name, .. }
        | Node::Environment { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// Checks whether `target` (whose name already equals `name`) matches `node`
/// and is known in `parse_ctx`.
#[inline]
fn target_present_on_node(
    node: &Node,
    name: &str,
    target: RuleTargetKey,
    parse_ctx: &ParseContext,
) -> bool {
    debug_assert_eq!(name, target.name);
    match (node, target.kind) {
        (
            Node::Command { .. } | Node::Infix { .. } | Node::Declarative { .. },
            RuleTargetKind::Command,
        ) => parse_ctx.knows_command_name(name),
        (Node::Environment { .. }, RuleTargetKind::Environment) => parse_ctx.knows_env_name(name),
        (
            Node::Command { .. } | Node::Infix { .. } | Node::Declarative { .. },
            RuleTargetKind::Character,
        ) => parse_ctx.knows_character_name(name),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseConfig, ParseContext};

    fn parse_ast(parse_ctx: &ParseContext, src: &str) -> Ast {
        let document = parse_ctx
            .parse(src, &ParseConfig::default())
            .try_into_document()
            .expect("source should parse")
            .0;
        Ast::from_syntax_root(&document.to_syntax())
    }

    #[test]
    fn find_first_and_collect_match_preorder_across_node_kinds() {
        let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
        let ast = parse_ast(
            &parse_ctx,
            r"{a \over \frac{\alpha}{\beta}} + \begin{matrix} \gamma_1^{\delta} \end{matrix}",
        );
        let targets = [
            RuleTargetKey {
                kind: RuleTargetKind::Command,
                name: "over",
            },
            RuleTargetKey {
                kind: RuleTargetKind::Character,
                name: "alpha",
            },
            RuleTargetKey {
                kind: RuleTargetKind::Environment,
                name: "matrix",
            },
            RuleTargetKey {
                kind: RuleTargetKind::Character,
                name: "gamma",
            },
            RuleTargetKey {
                kind: RuleTargetKind::Character,
                name: "delta",
            },
        ];

        let indexed = index_eliminated_by_name(&targets);
        let all_indexed = collect_eliminated_violations(&ast, &parse_ctx, &targets);
        let first = find_first_eliminated_violation(&ast, &parse_ctx, &indexed);
        assert_eq!(first, all_indexed.first().cloned());
        assert_eq!(
            all_indexed
                .iter()
                .map(|v| (v.target, v.node_name.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                (targets[0], Some("over")),
                (targets[1], Some("alpha")),
                (targets[2], Some("matrix")),
                (targets[3], Some("gamma")),
                (targets[4], Some("delta")),
            ]
        );

        // Also check the small-slice path (<= 4 targets).
        let small = collect_eliminated_violations(&ast, &parse_ctx, &targets[..3]);
        assert_eq!(small, all_indexed[..3]);
    }
}
