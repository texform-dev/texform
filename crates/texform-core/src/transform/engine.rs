//! Transform engine that applies compiled transformation rules to an AST.
//!
//! The engine executes in two phases:
//!
//! 1. **Normalize** — runs in a fixed-point loop, repeatedly applying normalization
//!    rules until the AST stabilizes (no rule fires) or the iteration limit is reached.
//! 2. **Cleanup** — runs a single pass of cleanup rules after normalization is complete.
//!
//! After both phases, the engine validates the resulting AST against the
//! [`NormalFormContract`] to ensure all expected forms have been eliminated.

use crate::ast::{Ast, NodeKind};
use crate::knowledge::{KnowledgeBase, lookup_command_node_name, lookup_environment_node_name};
use crate::transform::compile::{CompiledProfile, NormalFormContract, ProfileCompileError};
use crate::transform::context::TransformContext;
use crate::transform::rule::{RuleEffect, RuleKey, RuleMeta, RuleTarget, RuleTrigger};

/// Tracks how often a specific rule changed the AST or skipped after a trigger match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedRuleStat {
    /// The identity of the rule.
    pub key: RuleKey,
    /// The total number of times this rule fired.
    pub count: usize,
    /// The total number of times this rule's trigger matched but `apply()` returned `Skipped`.
    pub skipped_count: usize,
}

/// Accumulates statistics across an entire transformation pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformReport {
    /// Per-rule execution counts for rules that were attempted at least once.
    pub applied: Vec<AppliedRuleStat>,
    /// The number of fixed-point iterations the normalize phase completed.
    pub iterations: usize,
}

impl TransformReport {
    fn stat_mut(&mut self, key: RuleKey) -> &mut AppliedRuleStat {
        if let Some(index) = self.applied.iter().position(|entry| entry.key == key) {
            return &mut self.applied[index];
        }

        self.applied.push(AppliedRuleStat {
            key,
            count: 0,
            skipped_count: 0,
        });
        self.applied
            .last_mut()
            .expect("newly inserted rule stat must exist")
    }

    pub fn mark_rule_applied(&mut self, key: RuleKey) {
        self.stat_mut(key).count += 1;
    }

    pub fn mark_rule_skipped(&mut self, key: RuleKey) {
        self.stat_mut(key).skipped_count += 1;
    }

    pub fn record_iteration(&mut self, iterations: usize) {
        self.iterations = iterations;
    }
}

/// Errors reported by individual rules during application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformError {
    /// The rule encountered a node whose structure does not match its expectations.
    InvalidNodeShape { rule: RuleKey, message: String },
    /// The rule requires knowledge-base metadata that is not present.
    MissingMetadata { rule: RuleKey, name: String },
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformError::InvalidNodeShape { rule, message } => {
                write!(f, "{rule}: {message}")
            }
            TransformError::MissingMetadata { rule, name } => {
                write!(f, "{rule}: missing metadata for {name}")
            }
        }
    }
}

impl std::error::Error for TransformError {}

/// Top-level errors produced by the transform engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformEngineError {
    /// The profile failed to compile.
    Profile(ProfileCompileError),
    /// An individual rule returned an error during application.
    Rule(TransformError),
    /// The output AST still contains a form that the contract requires to be eliminated.
    ContractViolation {
        target: RuleTarget,
        node_name: Option<String>,
    },
    /// The normalize phase did not converge within the allowed iteration budget.
    MaxIterationsExceeded { max_iterations: usize },
}

impl std::fmt::Display for TransformEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformEngineError::Profile(error) => error.fmt(f),
            TransformEngineError::Rule(error) => error.fmt(f),
            TransformEngineError::ContractViolation { target, node_name } => write!(
                f,
                "transform contract violated for {} `{}` (node {:?})",
                target.kind_label(),
                target.name(),
                node_name
            ),
            TransformEngineError::MaxIterationsExceeded { max_iterations } => {
                write!(f, "transform exceeded max iterations: {max_iterations}")
            }
        }
    }
}

impl std::error::Error for TransformEngineError {}

/// Applies compiled transformation rules to an AST and returns a report of what changed.
///
/// Execution proceeds in two phases:
/// 1. **Normalize** — loops over the AST repeatedly, applying normalization rules until
///    no rule fires (fixed-point) or `max_iterations` is reached.
/// 2. **Cleanup** — makes a single pass with cleanup rules. Cleanup runs after
///    normalization is complete and is not expected to trigger further normalization.
///
/// After both phases, the output AST is validated against the [`NormalFormContract`].
pub fn transform_ast(
    ast: &mut Ast,
    kb: &KnowledgeBase,
    compiled: &CompiledProfile,
) -> Result<TransformReport, TransformEngineError> {
    let mut report = TransformReport {
        applied: Vec::new(),
        iterations: 0,
    };

    // --- Phase 1: Normalize (fixed-point loop) ---
    for iteration in 0..compiled.max_iterations {
        let mut changed = false;
        // Snapshot node IDs before this iteration so we iterate over a stable
        // list while rules mutate the AST (avoids iterator invalidation).
        let snapshot = preorder_snapshot(ast);

        {
            let mut cx = TransformContext::new(ast, kb, compiled, &mut report);
            for node_id in snapshot {
                // A node may have been removed by an earlier rule application
                // within this same iteration, so check that it still exists.
                if !cx.ast.contains(node_id) {
                    continue;
                }

                for rule in &compiled.normalize_phase.ordered_rules {
                    if !rule_matches(rule.meta(), node_id, &cx) {
                        continue;
                    }

                    match rule
                        .apply(&mut cx, node_id)
                        .map_err(TransformEngineError::Rule)?
                    {
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
            break;
        }

        if iteration + 1 == compiled.max_iterations {
            return Err(TransformEngineError::MaxIterationsExceeded {
                max_iterations: compiled.max_iterations,
            });
        }
    }

    // --- Phase 2: Cleanup (single pass) ---
    // Cleanup rules run once after normalization converges. They are not expected
    // to produce forms that would re-trigger normalization rules.
    if let Some(cleanup_phase) = &compiled.cleanup_phase {
        let snapshot = preorder_snapshot(ast);
        let mut cx = TransformContext::new(ast, kb, compiled, &mut report);
        for node_id in snapshot {
            if !cx.ast.contains(node_id) {
                continue;
            }

            for rule in &cleanup_phase.ordered_rules {
                if !rule_matches(rule.meta(), node_id, &cx) {
                    continue;
                }

                match rule
                    .apply(&mut cx, node_id)
                    .map_err(TransformEngineError::Rule)?
                {
                    RuleEffect::Applied => {
                        cx.mark_rule_applied(rule.meta().key);
                        #[cfg(debug_assertions)]
                        cx.ast.assert_invariants();
                        break;
                    }
                    RuleEffect::Skipped => cx.mark_rule_skipped(rule.meta().key),
                }
            }
        }
    }

    validate_contract(ast, &compiled.contract)?;
    Ok(report)
}

/// Collects all node IDs in pre-order traversal to produce a stable snapshot.
///
/// The snapshot is taken before each iteration so that rule applications can
/// safely mutate the AST without invalidating the traversal order.
fn preorder_snapshot(ast: &Ast) -> Vec<crate::ast::NodeId> {
    ast.find_all(ast.root(), |_| true)
}

/// Returns `true` if the rule's trigger conditions match the given node.
///
/// When a rule declares no triggers, it matches every node (convention: an
/// empty trigger list means universal applicability).
fn rule_matches(meta: &RuleMeta, node_id: crate::ast::NodeId, cx: &TransformContext<'_>) -> bool {
    if meta.triggers.is_empty() {
        return true;
    }

    meta.triggers
        .iter()
        .copied()
        .any(|trigger| trigger_matches(trigger, node_id, cx))
}

/// Checks whether a single trigger condition is satisfied for the given node.
fn trigger_matches(
    trigger: RuleTrigger,
    node_id: crate::ast::NodeId,
    cx: &TransformContext<'_>,
) -> bool {
    match trigger {
        RuleTrigger::NodeKind(kind) => cx.ast.kind(node_id) == kind,
        RuleTrigger::Command(record) => cx
            .active_command(node_id)
            .is_some_and(|active| active.name == record.name),
        RuleTrigger::Environment(record) => cx
            .active_env(node_id)
            .is_some_and(|active| active.name == record.name),
        RuleTrigger::CommandTag(tag) => cx
            .active_command(node_id)
            .is_some_and(|active| active.tags.contains(&tag)),
        RuleTrigger::EnvironmentTag(tag) => cx
            .active_env(node_id)
            .is_some_and(|active| active.tags.contains(&tag)),
    }
}

/// Walks the AST and verifies that no eliminated form from the contract is still present.
fn validate_contract(ast: &Ast, contract: &NormalFormContract) -> Result<(), TransformEngineError> {
    for node_id in ast.find_all(ast.root(), |_| true) {
        for target in &contract.eliminated_forms {
            if target_present(ast, node_id, *target) {
                let node_name = match ast.kind(node_id) {
                    NodeKind::Command | NodeKind::Infix | NodeKind::Declarative => {
                        lookup_command_node_name(ast.node(node_id)).map(ToString::to_string)
                    }
                    NodeKind::Environment => {
                        lookup_environment_node_name(ast.node(node_id)).map(ToString::to_string)
                    }
                    _ => None,
                };
                return Err(TransformEngineError::ContractViolation {
                    target: *target,
                    node_name,
                });
            }
        }
    }
    Ok(())
}

/// Returns `true` if the given node matches the specified rule target form.
fn target_present(ast: &Ast, node_id: crate::ast::NodeId, target: RuleTarget) -> bool {
    match target {
        RuleTarget::Command(record) => {
            lookup_command_node_name(ast.node(node_id)).is_some_and(|name| name == record.name)
        }
        RuleTarget::Environment(record) => {
            lookup_environment_node_name(ast.node(node_id)).is_some_and(|name| name == record.name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Ast, Node, NodeId};
    use crate::knowledge::KnowledgeBase;
    use crate::transform::compile::{CompiledPhase, CompiledProfile};
    use crate::transform::context::TransformContext;
    use crate::transform::rule::{
        RuleConsumes, RuleEffect, RuleGroup, RuleMeta, RulePhase, RuleProduces, RuleSafety,
        RuleTrigger, TransformRule,
    };
    use texform_specs::argspec;
    use texform_specs::specs::{AllowedMode, BuiltinCommandRecord, CommandKind, CommandSpec};

    struct SkipRule;

    impl TransformRule for SkipRule {
        fn meta(&self) -> &'static RuleMeta {
            &SKIP_RULE_META
        }

        fn apply(
            &self,
            _cx: &mut TransformContext<'_>,
            _node_id: NodeId,
        ) -> Result<RuleEffect, TransformError> {
            Ok(RuleEffect::Skipped)
        }
    }

    static SKIP_COMMAND: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "skip-me",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!(""),
        tags: &[],
    };

    static SKIP_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Physics,
            name: "skip-me",
        },
        summary: "mock skip rule",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[RuleTrigger::Command(&SKIP_COMMAND)],
        consumes: RuleConsumes {
            eliminates: &[],
            requires: &[],
        },
        produces: RuleProduces { targets: &[] },
    };

    static SKIP_RULE: SkipRule = SkipRule;

    fn compiled_profile_with(rule: &'static dyn TransformRule) -> CompiledProfile {
        CompiledProfile {
            normalize_phase: CompiledPhase {
                phase: RulePhase::Normalize,
                ordered_rules: vec![rule],
            },
            cleanup_phase: None,
            statuses: Vec::new(),
            contract: NormalFormContract {
                eliminated_forms: Vec::new(),
            },
            max_iterations: 4,
        }
    }

    #[test]
    fn report_tracks_skipped_rule_attempts_after_trigger_match() {
        let mut kb = KnowledgeBase::empty();
        kb.insert_or_override_command(CommandSpec {
            name: "skip-me".to_string(),
            kind: CommandKind::Prefix,
            allowed_mode: AllowedMode::Math,
            argspec: argspec!("").into(),
            tags: vec![],
        });

        let mut ast = Ast::new();
        let node_id = ast.new_node(Node::Command {
            name: "skip-me".to_string(),
            args: Vec::new(),
        });
        ast.append_child(ast.root(), node_id);

        let report = transform_ast(&mut ast, &kb, &compiled_profile_with(&SKIP_RULE))
            .expect("transform with skip-only rule should succeed");

        assert_eq!(report.iterations, 1);
        assert_eq!(
            report.applied,
            vec![AppliedRuleStat {
                key: SKIP_RULE_META.key,
                count: 0,
                skipped_count: 1,
            }]
        );
    }
}
