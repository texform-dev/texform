//! AST transformation subsystem.
//!
//! This module is organized around three layers:
//! - [`rule`] — static rule metadata and `TransformRule`
//! - [`context`] — immutable `TransformContext` and its builder
//! - [`engine`] — explicit execution over `&ParseContext + &TransformContext + &mut Ast`

pub mod context;
pub mod engine;
pub mod helpers;
pub(crate) mod macro_support;
mod macros;
pub mod registry;
pub mod rule;
pub mod rule_context;
mod rules;

#[allow(unused_imports)]
pub(crate) use macros::{
    alias_rule, cmd_targets, cmd_triggers, define_rule, env_targets, env_triggers,
};

pub use context::{
    BuiltinRuleSetId, TransformBuildError, TransformContext, TransformContextBuilder,
};
pub use engine::{
    AppliedRuleStat, TransformEngineError, TransformError, TransformReport, transform_ast,
};
pub use rule::{
    RuleConsumes, RuleEffect, RuleGroup, RuleKey, RuleMeta, RulePhase, RuleProduces, RuleSafety,
    RuleTarget, RuleTargetKey, RuleTargetKind, RuleTrigger, TransformRule,
};
