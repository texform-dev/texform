//! AST rewrite phase: scheduling, rule application, and eliminated-form checks.

pub mod class_set;
mod contract;
pub mod helpers;
pub(crate) mod macro_support;
mod macros;
pub mod plan;
mod registry;
pub mod rule;
pub mod rule_context;
pub mod rules;
mod scheduler;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use macros::transform_examples;
#[allow(unused_imports)]
pub(crate) use macros::{alias_rule, char_targets, cmd_targets, define_rule, env_targets};

pub use class_set::RuleClassSet;
pub use contract::{ContractViolation, collect_eliminated_violations};
pub use plan::{Plan, PlanBuildError, RuleAvailabilityFailure};
pub use registry::all_rules;
pub use rule::{
    PackageName, RewriteRule, RuleClass, RuleConsumes, RuleEffect, RuleKey, RuleMeta, RuleProduces,
    RuleSafety, RuleTarget, RuleTargetKey, RuleTargetKind,
};
pub use rule_context::{CommandView, DeclarativeView, EnvironmentView, InfixView, RuleContext};

/// Accumulates statistics across an entire rewrite pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RewriteReport {
    /// Per-rule execution counts for rules that were attempted at least once.
    pub rules: Vec<RewriteRuleStat>,
    /// The number of fixed-point iterations the Rewrite phase completed.
    pub iterations: usize,
}

/// Tracks how often a specific rule changed the AST or skipped after a scheduling target match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteRuleStat {
    /// The identity of the rule.
    pub key: RuleKey,
    /// The total number of times this rule fired.
    pub applied_count: usize,
    /// The total number of times this rule's scheduling target matched but `apply()` returned `Skipped`.
    pub skipped_count: usize,
}

impl RewriteReport {
    pub(crate) fn stat_mut(&mut self, key: RuleKey) -> &mut RewriteRuleStat {
        if let Some(index) = self.rules.iter().position(|entry| entry.key == key) {
            return &mut self.rules[index];
        }

        self.rules.push(RewriteRuleStat {
            key,
            applied_count: 0,
            skipped_count: 0,
        });
        self.rules
            .last_mut()
            .expect("newly inserted rule stat must exist")
    }

    pub fn mark_rule_applied(&mut self, key: RuleKey) {
        self.stat_mut(key).applied_count += 1;
    }

    pub fn mark_rule_skipped(&mut self, key: RuleKey) {
        self.stat_mut(key).skipped_count += 1;
    }

    pub fn record_iteration(&mut self, iterations: usize) {
        self.iterations = iterations;
    }
}

/// Top-level errors produced by the Rewrite phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RewriteError {
    /// An individual rule returned an error during application.
    Rule { rule: RuleKey, kind: RuleError },
    /// The output AST still contains a form that the rewrite contract requires to be eliminated.
    ContractViolation {
        target: RuleTargetKey,
        node_name: Option<String>,
    },
    /// The Rewrite phase did not converge within the allowed iteration budget.
    MaxIterationsExceeded { max_iterations: usize },
}

/// Errors reported by individual rules during application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleError {
    /// The rule encountered a node whose structure does not match its expectations.
    InvalidNodeShape { message: String },
    /// The rule requires knowledge-base metadata that is not present.
    MissingMetadata { name: String },
}

impl std::fmt::Display for RewriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewriteError::Rule { rule, kind } => match kind {
                RuleError::InvalidNodeShape { message } => write!(f, "{rule}: {message}"),
                RuleError::MissingMetadata { name } => {
                    write!(f, "{rule}: missing metadata for {name}")
                }
            },
            RewriteError::ContractViolation { target, node_name } => write!(
                f,
                "rewrite contract violated for {} `{}` (node {:?})",
                target.kind_label(),
                target.name,
                node_name
            ),
            RewriteError::MaxIterationsExceeded { max_iterations } => {
                write!(f, "rewrite exceeded max iterations: {max_iterations}")
            }
        }
    }
}

impl std::error::Error for RewriteError {}

use crate::ast::Ast;
use crate::parse::ParseContext;

/// Applies rewrite rules to an AST and records what changed.
pub fn run(
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    plan: &Plan,
    max_iterations: usize,
    report: &mut RewriteReport,
) -> Result<(), RewriteError> {
    scheduler::drive_fixed_point(ast, parse_ctx, plan, max_iterations, report)
}

#[cfg(test)]
pub(crate) fn run_one_rule_for_test(
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    rule: &'static dyn RewriteRule,
    class: RuleClass,
) -> Result<crate::TransformReport, crate::TransformError> {
    let build_config = crate::BuildConfig::profile(crate::Profile::Authoring)
        .rewrite_classes(RuleClassSet::from(class))
        .only_rule_for_tests(rule.meta().key);
    let context = crate::TransformContext::from_build_config(build_config, parse_ctx)
        .map_err(crate::TransformError::Build)?;
    context.run_with(
        ast,
        parse_ctx,
        &crate::TransformConfig {
            rewrite_enabled: true,
            lower_attributes_enabled: false,
            flatten_groups: crate::FlattenGroupsConfig::DISABLED,
            max_iterations: 100,
        },
    )
}
