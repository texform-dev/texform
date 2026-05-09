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

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use macros::transform_examples;
#[allow(unused_imports)]
pub(crate) use macros::{alias_rule, char_targets, cmd_targets, define_rule, env_targets};

pub use context::{
    TransformBuildError, TransformContext, TransformContextBuilder, TransformProfile,
};
pub use engine::{
    AppliedRuleStat, TransformEngineError, TransformError, TransformReport, transform_ast,
};
pub use rule::{
    PackageName, RuleClass, RuleConsumes, RuleEffect, RuleKey, RuleMeta, RulePhase, RuleProduces,
    RuleSafety, RuleTarget, RuleTargetKey, RuleTargetKind, TransformRule,
};
