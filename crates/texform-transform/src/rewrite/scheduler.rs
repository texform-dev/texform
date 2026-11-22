//! Fixed-point scheduler for the rewrite phase.

use rustc_hash::FxHashMap;

use crate::ast::{Ast, NodeId, NodeKind};
use crate::knowledge::{lookup_command_node_name, lookup_environment_node_name};
use crate::parse::{ContentMode, ParseContext};
use crate::report::ReportRecorder;
use crate::rewrite::RewriteError;
use crate::rewrite::plan::Plan;
use crate::rewrite::rule::{RuleEffect, RuleMeta, RuleTarget};
use crate::rewrite::rule_context::RuleContext;

pub(super) fn drive_fixed_point(
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    plan: &Plan,
    max_iterations: usize,
    recorder: &mut ReportRecorder,
) -> Result<bool, RewriteError> {
    let rules = plan.rules();
    if rules.is_empty() {
        recorder.record_rewrite_iterations(0);
        return Ok(false);
    }

    // Every `RuleTarget` matches only when the node's command/environment name
    // equals the trigger name, so a rule can fire on a node only if that name is
    // one of its trigger names. Index rules by trigger name to skip the rules
    // that provably cannot match, instead of testing all rules against every
    // node. Indices stay in ascending rule order, preserving the fixed-point
    // loop's "first matching rule in plan order wins" semantics.
    let rules_by_name = &plan.rules_by_name;
    let mut candidate_nodes: Vec<NodeId> = Vec::new();

    for iteration in 0..max_iterations {
        let mut changed = false;
        candidate_nodes.clear();
        ast.visit_preorder(ast.root(), |id| {
            if node_target_name(ast, id).is_some_and(|name| rules_by_name.contains_key(name)) {
                candidate_nodes.push(id);
            }
        });

        if candidate_nodes.is_empty() {
            recorder.record_rewrite_iterations(iteration + 1);
            return Ok(iteration > 0);
        }

        {
            let mut cx = RuleContext::new(ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
            for &node_id in &candidate_nodes {
                if !cx.ast.contains(node_id) {
                    continue;
                }

                // Re-check the target name because an earlier rule in this
                // iteration may have replaced `node_id` in place.
                let Some(name) = node_target_name(cx.ast, node_id) else {
                    continue;
                };
                let Some(candidates) = rules_by_name.get(name) else {
                    continue;
                };

                for &rule_index in candidates {
                    let rule = rules[rule_index];
                    if !rule_matches(rule.meta(), node_id, &cx) {
                        continue;
                    }

                    let result =
                        rule.apply(&mut cx, node_id)
                            .map_err(|kind| RewriteError::Rule {
                                rule: rule.meta().key,
                                kind,
                            })?;
                    match result {
                        RuleEffect::Applied => {
                            recorder.record_rule_applied(rule.meta().key);
                            #[cfg(debug_assertions)]
                            cx.ast.assert_invariants();
                            changed = true;
                            break;
                        }
                        RuleEffect::Skipped => recorder.record_rule_skipped(rule.meta().key),
                    }
                }
            }
        }

        if !changed {
            recorder.record_rewrite_iterations(iteration + 1);
            return Ok(iteration > 0);
        }

        if iteration + 1 == max_iterations {
            return Err(RewriteError::MaxIterationsExceeded { max_iterations });
        }
    }

    unreachable!("loop body returns or errors on every iteration");
}

/// Map each trigger name to the indices of the rules that trigger on it, in
/// ascending rule order. A rule appears under each distinct name it triggers
/// on; repeated triggers with the same name are collapsed.
pub(super) fn index_rules_by_trigger_name(
    rules: &[&'static dyn crate::rewrite::rule::RewriteRule],
) -> FxHashMap<&'static str, Vec<usize>> {
    let mut by_name: FxHashMap<&'static str, Vec<usize>> = FxHashMap::default();
    for (index, rule) in rules.iter().enumerate() {
        for target in rule.meta().triggers {
            let bucket = by_name.entry(target.name()).or_default();
            if bucket.last() != Some(&index) {
                bucket.push(index);
            }
        }
    }
    by_name
}

/// The command/environment name a rule trigger would match on, or `None` for
/// nodes no rule can target. Borrows from the node to avoid an allocation in
/// the scheduler's hot loop.
fn node_target_name(ast: &Ast, node_id: NodeId) -> Option<&str> {
    match ast.kind(node_id) {
        NodeKind::Command | NodeKind::Infix | NodeKind::Declarative => {
            lookup_command_node_name(ast.node(node_id))
        }
        NodeKind::Environment => lookup_environment_node_name(ast.node(node_id)),
        _ => None,
    }
}

fn rule_matches(meta: &RuleMeta, node_id: NodeId, cx: &RuleContext<'_>) -> bool {
    meta.triggers
        .iter()
        .copied()
        .any(|target| target_matches(target, node_id, cx))
}

fn target_matches(target: RuleTarget, node_id: NodeId, cx: &RuleContext<'_>) -> bool {
    match target {
        RuleTarget::Command(record) => cx
            .active_command(node_id)
            .is_some_and(|active| active.name == record.name),
        RuleTarget::Environment(record) => cx
            .active_env(node_id)
            .is_some_and(|active| active.name == record.name),
        RuleTarget::Character(record) => lookup_command_node_name(cx.ast.node(node_id))
            .is_some_and(|name| {
                name == record.name
                    && (cx.lookup_character(name, ContentMode::Math).is_some()
                        || cx.lookup_character(name, ContentMode::Text).is_some())
            }),
    }
}
