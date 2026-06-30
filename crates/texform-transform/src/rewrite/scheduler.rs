//! Fixed-point scheduler for the rewrite phase.

use std::collections::HashMap;

use crate::ast::{Ast, NodeId, NodeKind};
use crate::knowledge::{lookup_command_node_name, lookup_environment_node_name};
use crate::parse::{ContentMode, ParseContext};
use crate::rewrite::plan::Plan;
use crate::rewrite::rule::{RuleEffect, RuleMeta, RuleTarget, RuleTargetKey, RuleTargetKind};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{RewriteError, RewriteReport};

pub(super) fn drive_fixed_point(
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    plan: &Plan,
    max_iterations: usize,
    report: &mut RewriteReport,
) -> Result<(), RewriteError> {
    let rules = plan.rules();
    if rules.is_empty() {
        report.record_iteration(0);
        return Ok(());
    }

    // Every `RuleTarget` matches only when the node's command/environment name
    // equals the trigger name, so a rule can fire on a node only if that name is
    // one of its trigger names. Index rules by trigger name to skip the rules
    // that provably cannot match, instead of testing all rules against every
    // node. Indices stay in ascending rule order, preserving the fixed-point
    // loop's "first matching rule in plan order wins" semantics.
    let rules_by_name = index_rules_by_trigger_name(rules);

    for iteration in 0..max_iterations {
        let mut changed = false;
        let snapshot = preorder_snapshot(ast);

        {
            let mut cx = RuleContext::new(ast, parse_ctx.math_kb(), parse_ctx.text_kb(), report);
            for node_id in snapshot {
                if !cx.ast.contains(node_id) {
                    continue;
                }

                // A nameless node (char, group, ...) can never match a rule, and
                // a named node only needs to consider rules indexed under it.
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
                            cx.mark_rule_applied(rule.meta().key);
                            #[cfg(debug_assertions)]
                            cx.ast.assert_invariants();
                            changed = true;
                            break;
                        }
                        RuleEffect::Skipped => cx.mark_rule_skipped(rule.meta().key),
                    }
                }
            }
        }

        if !changed {
            report.record_iteration(iteration + 1);
            return Ok(());
        }

        if iteration + 1 == max_iterations {
            return Err(RewriteError::MaxIterationsExceeded { max_iterations });
        }
    }

    unreachable!("loop body returns or errors on every iteration");
}

fn preorder_snapshot(ast: &Ast) -> Vec<NodeId> {
    ast.find_all(ast.root(), |_| true)
}

/// Map each trigger name to the indices of the rules that trigger on it, in
/// ascending rule order. A rule appears under each distinct name it triggers
/// on; repeated triggers with the same name are collapsed.
fn index_rules_by_trigger_name(
    rules: &[&'static dyn crate::rewrite::rule::RewriteRule],
) -> HashMap<&'static str, Vec<usize>> {
    let mut by_name: HashMap<&'static str, Vec<usize>> = HashMap::new();
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

pub(super) fn target_present(
    ast: &Ast,
    node_id: NodeId,
    target: RuleTargetKey,
    parse_ctx: &ParseContext,
) -> bool {
    match target.kind {
        RuleTargetKind::Command => lookup_command_node_name(ast.node(node_id))
            .is_some_and(|name| name == target.name && parse_ctx.knows_command_name(name)),
        RuleTargetKind::Environment => lookup_environment_node_name(ast.node(node_id))
            .is_some_and(|name| name == target.name && parse_ctx.knows_env_name(name)),
        RuleTargetKind::Character => lookup_command_node_name(ast.node(node_id))
            .is_some_and(|name| name == target.name && parse_ctx.knows_character_name(name)),
    }
}

pub(super) fn node_name_for_target(ast: &Ast, node_id: NodeId) -> Option<String> {
    node_target_name(ast, node_id).map(ToString::to_string)
}
