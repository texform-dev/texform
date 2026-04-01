//! AST transformation subsystem.
//!
//! This module takes a parsed AST and applies normalization and cleanup rules
//! to produce a canonical form. The subsystem is organized into several layers:
//!
//! - **[`rule`]** — defines the [`TransformRule`] trait, rule metadata
//!   ([`RuleMeta`]), trigger conditions, and produce/consume declarations.
//! - **[`config`]** — the user-facing [`TransformProfile`] that selects a
//!   ruleset and per-rule overrides.
//! - **[`compile`]** — compiles a profile against a knowledge base into a
//!   [`CompiledProfile`] with topologically sorted phases and a normal-form
//!   contract.
//! - **[`engine`]** — executes the compiled profile against a mutable AST,
//!   running a fixed-point normalize loop followed by a single cleanup pass.

pub mod compile;
pub mod config;
pub mod context;
pub mod engine;
pub mod helpers;
pub mod registry;
pub mod rule;
mod rules;

pub use compile::{
    CompiledPhase, CompiledProfile, NormalFormContract, ProfileCompileError, RuleAvailability,
    RuleStatus,
};
pub use config::{BuiltinRuleSetId, RuleSetting, TransformProfile};
pub use context::TransformContext;
pub use engine::{
    AppliedRuleStat, TransformEngineError, TransformError, TransformReport, transform_ast,
};
pub use rule::{
    RuleConsumes, RuleEffect, RuleGroup, RuleKey, RuleMeta, RulePhase, RuleProduces, RuleSafety,
    RuleTarget, RuleTrigger, TransformRule,
};
