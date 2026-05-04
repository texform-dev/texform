//! Transform engine that applies transformation rules to an AST.
//!
//! The engine executes in two phases:
//!
//! 1. **Normalize** — runs in a fixed-point loop, repeatedly applying normalization
//!    rules until the AST stabilizes (no rule fires) or the iteration limit is reached.
//! 2. **Cleanup** — runs a single pass of cleanup rules after normalization is complete.
//!
//! After both phases, the engine validates the resulting AST against the
//! eliminated-form contract derived into [`TransformContext`].

use crate::ast::{Ast, NodeId, NodeKind};
use crate::knowledge::{lookup_command_node_name, lookup_environment_node_name};
use crate::parse::ParseContext;
use crate::transform::context::TransformContext;
use crate::transform::rule::{RuleEffect, RuleKey, RuleMeta, RuleTarget, RuleTargetKey};
use crate::transform::rule_context::RuleContext;

/// Tracks how often a specific rule changed the AST or skipped after a consumed target match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedRuleStat {
    /// The identity of the rule.
    pub key: RuleKey,
    /// The total number of times this rule fired.
    pub count: usize,
    /// The total number of times this rule's consumed target matched but `apply()` returned `Skipped`.
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
    /// An individual rule returned an error during application.
    Rule(TransformError),
    /// The output AST still contains a form that the contract requires to be eliminated.
    ContractViolation {
        target: RuleTargetKey,
        node_name: Option<String>,
    },
    /// The normalize phase did not converge within the allowed iteration budget.
    MaxIterationsExceeded { max_iterations: usize },
}

impl std::fmt::Display for TransformEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformEngineError::Rule(error) => error.fmt(f),
            TransformEngineError::ContractViolation { target, node_name } => write!(
                f,
                "transform contract violated for {} `{}` (node {:?})",
                target.kind_label(),
                target.name,
                node_name
            ),
            TransformEngineError::MaxIterationsExceeded { max_iterations } => {
                write!(f, "transform exceeded max iterations: {max_iterations}")
            }
        }
    }
}

impl std::error::Error for TransformEngineError {}

/// Applies transformation rules to an AST and returns a report of what changed.
pub fn transform_ast(
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    transform_ctx: &TransformContext,
) -> Result<TransformReport, TransformEngineError> {
    let mut report = TransformReport {
        applied: Vec::new(),
        iterations: 0,
    };

    for iteration in 0..transform_ctx.max_iterations() {
        let mut changed = false;
        let snapshot = preorder_snapshot(ast);

        {
            let mut cx =
                RuleContext::new(ast, parse_ctx.math_kb(), parse_ctx.text_kb(), &mut report);
            for node_id in snapshot {
                if !cx.ast.contains(node_id) {
                    continue;
                }

                for rule in transform_ctx.normalize_rules() {
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

        if iteration + 1 == transform_ctx.max_iterations() {
            return Err(TransformEngineError::MaxIterationsExceeded {
                max_iterations: transform_ctx.max_iterations(),
            });
        }
    }

    if !transform_ctx.cleanup_rules().is_empty() {
        let snapshot = preorder_snapshot(ast);
        let mut cx = RuleContext::new(ast, parse_ctx.math_kb(), parse_ctx.text_kb(), &mut report);
        for node_id in snapshot {
            if !cx.ast.contains(node_id) {
                continue;
            }

            for rule in transform_ctx.cleanup_rules() {
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

    assert_eliminated_forms(ast, parse_ctx, transform_ctx.eliminated_forms())?;
    Ok(report)
}

fn preorder_snapshot(ast: &Ast) -> Vec<NodeId> {
    ast.find_all(ast.root(), |_| true)
}

fn rule_matches(meta: &RuleMeta, node_id: NodeId, cx: &RuleContext<'_>) -> bool {
    meta.consumes
        .eliminates
        .iter()
        .chain(meta.consumes.touches.iter())
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
    }
}

fn assert_eliminated_forms(
    ast: &Ast,
    parse_ctx: &ParseContext,
    eliminated_forms: &[RuleTargetKey],
) -> Result<(), TransformEngineError> {
    for node_id in ast.find_all(ast.root(), |_| true) {
        for target in eliminated_forms {
            if target_present(ast, node_id, *target, parse_ctx) {
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

fn target_present(
    ast: &Ast,
    node_id: NodeId,
    target: RuleTargetKey,
    parse_ctx: &ParseContext,
) -> bool {
    match target.kind {
        crate::transform::rule::RuleTargetKind::Command => {
            lookup_command_node_name(ast.node(node_id))
                .is_some_and(|name| name == target.name && parse_ctx.knows_command_name(name))
        }
        crate::transform::rule::RuleTargetKind::Environment => {
            lookup_environment_node_name(ast.node(node_id))
                .is_some_and(|name| name == target.name && parse_ctx.knows_env_name(name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Node, NodeId};
    use crate::parse::{AllowedMode, ContentMode, ParseContext, ParseContextBuilder};
    use crate::transform::context::TransformContext;
    use crate::transform::rule::{
        RuleConsumes, RuleEffect, RuleMeta, RulePackage, RulePhase, RuleProduces, RuleSafety,
        RuleTarget, RuleTier, TransformRule,
    };
    use crate::transform::rule_context::RuleContext;
    use texform_specs::argspec;
    use texform_specs::builtin::physics;
    use texform_specs::specs::{BuiltinCommandRecord, CommandKind};

    struct SkipRule;

    impl TransformRule for SkipRule {
        fn meta(&self) -> &'static RuleMeta {
            &SKIP_RULE_META
        }

        fn apply(
            &self,
            _cx: &mut RuleContext<'_>,
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
            package: RulePackage::Physics,
            name: "skip-me",
        },
        tier: RuleTier::Base,
        summary: "mock skip rule",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&SKIP_COMMAND)],
            touches: &[],
        },
        produces: RuleProduces { targets: &[] },
    };

    static SKIP_RULE: SkipRule = SkipRule;

    static TOUCH_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: RulePackage::Physics,
            name: "touch-me",
        },
        tier: RuleTier::Base,
        summary: "mock touch rule",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[],
            touches: &[RuleTarget::Command(&SKIP_COMMAND)],
        },
        produces: RuleProduces { targets: &[] },
    };

    struct TouchRule;

    impl TransformRule for TouchRule {
        fn meta(&self) -> &'static RuleMeta {
            &TOUCH_RULE_META
        }

        fn apply(
            &self,
            _cx: &mut RuleContext<'_>,
            _node_id: NodeId,
        ) -> Result<RuleEffect, TransformError> {
            Ok(RuleEffect::Skipped)
        }
    }

    static TOUCH_RULE: TouchRule = TouchRule;

    fn transform_context_with(rule: &'static dyn TransformRule) -> TransformContext {
        TransformContext::from_parts_for_test(vec![rule], Vec::new(), Vec::new(), 4)
    }

    #[test]
    fn report_tracks_skipped_rule_attempts_after_consumed_command_match() {
        let parse_ctx = ParseContextBuilder::empty()
            .insert_item(crate::parse::CommandItem::new(
                "skip-me",
                CommandKind::Prefix,
                AllowedMode::Math,
                "",
            ))
            .build()
            .expect("parse context should build");

        let mut ast = Ast::new();
        let node_id = ast.new_node(Node::Command {
            name: "skip-me".to_string(),
            args: Vec::new(),
            known: true,
        });
        ast.append_child(ast.root(), node_id);

        let report = transform_ast(&mut ast, &parse_ctx, &transform_context_with(&SKIP_RULE))
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

    #[test]
    fn report_tracks_skipped_rule_attempts_after_touched_command_match() {
        let parse_ctx = ParseContextBuilder::empty()
            .insert_item(crate::parse::CommandItem::new(
                "skip-me",
                CommandKind::Prefix,
                AllowedMode::Math,
                "",
            ))
            .build()
            .expect("parse context should build");

        let mut ast = Ast::new();
        let node_id = ast.new_node(Node::Command {
            name: "skip-me".to_string(),
            args: Vec::new(),
            known: true,
        });
        ast.append_child(ast.root(), node_id);

        let report = transform_ast(&mut ast, &parse_ctx, &transform_context_with(&TOUCH_RULE))
            .expect("transform with touch-only rule should succeed");

        assert_eq!(report.iterations, 1);
        assert_eq!(
            report.applied,
            vec![AppliedRuleStat {
                key: TOUCH_RULE_META.key,
                count: 0,
                skipped_count: 1,
            }]
        );
    }

    #[test]
    fn contract_ignores_unloaded_unknown_command_forms() {
        let parse_ctx = ParseContext::empty();
        assert!(
            parse_ctx
                .lookup_command("quantity", ContentMode::Math)
                .is_none()
        );

        let mut ast = parse_ctx
            .parse_to_ast(r"\quantity{a}", false)
            .expect("non-strict parse should preserve unknown package command");

        let root_children = ast.children(ast.root());
        match ast.node(root_children[0]) {
            Node::Command { name, known, .. } => {
                assert_eq!(name, "quantity");
                assert!(!known);
            }
            other => panic!("expected unknown command node, got {:?}", other),
        }

        let transform_ctx = TransformContext::from_parts_for_test(
            Vec::new(),
            Vec::new(),
            vec![RuleTarget::Command(&physics::cmd::QUANTITY).key()],
            4,
        );

        transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("runtime contract should ignore forms unavailable in this parse context");
    }

    #[test]
    fn contract_uses_name_known_union_across_both_lanes() {
        let parse_ctx = ParseContextBuilder::empty()
            .insert_item(crate::parse::CommandItem::new(
                "textonly-target",
                CommandKind::Prefix,
                AllowedMode::Text,
                "m:T",
            ))
            .build()
            .expect("parse context should build");

        assert!(
            parse_ctx
                .lookup_command("textonly-target", ContentMode::Math)
                .is_none()
        );
        assert!(parse_ctx.knows_command_name("textonly-target"));

        let mut report = TransformReport {
            applied: Vec::new(),
            iterations: 0,
        };

        let mut scratch_ast = Ast::new();
        let cx = RuleContext::new(
            &mut scratch_ast,
            parse_ctx.math_kb(),
            parse_ctx.text_kb(),
            &mut report,
        );
        assert!(cx.knows_command_name("textonly-target"));

        let mut ast = Ast::new();
        let node_id = ast.new_node(Node::Command {
            name: "textonly-target".to_string(),
            args: Vec::new(),
            known: true,
        });
        ast.append_child(ast.root(), node_id);

        let transform_ctx = TransformContext::from_parts_for_test(
            Vec::new(),
            Vec::new(),
            vec![RuleTargetKey {
                kind: crate::transform::rule::RuleTargetKind::Command,
                name: "textonly-target",
            }],
            4,
        );

        let error = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect_err("known command names should still trip contract outside the active lane");

        match error {
            TransformEngineError::ContractViolation { target, node_name } => {
                assert_eq!(target.name, "textonly-target");
                assert_eq!(node_name.as_deref(), Some("textonly-target"));
            }
            other => panic!("expected contract violation, got {other:?}"),
        }
    }
}
