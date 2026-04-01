//! Configuration types for transform profiles.
//!
//! A [`TransformProfile`] combines a base [`BuiltinRuleSetId`] with optional
//! per-rule overrides ([`RuleSetting`]) and engine parameters such as the
//! maximum iteration count. Profiles are the primary user-facing knob for
//! controlling which rules run and how aggressively the normalize loop iterates.

use std::collections::BTreeMap;

use crate::transform::rule::RuleKey;

/// Predefined rule set identifier for common use cases.
///
/// Each variant selects a curated subset of the registered rules. The mapping
/// from identifier to concrete rule list lives in
/// [`rules_for_ruleset()`](super::registry::rules_for_ruleset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinRuleSetId {
    /// General-purpose normalization suitable for canonical output.
    Normalize,
    /// Normalization tuned for math-expression recognition (MER) data pipelines.
    Mer,
}

/// Per-rule override within a [`TransformProfile`].
///
/// When a profile's `rules` map contains an entry for a given [`RuleKey`],
/// this setting takes precedence over the base rule set's default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleSetting {
    /// The rule is explicitly enabled regardless of the base rule set.
    On,
    /// The rule is silently skipped during execution.
    Ignored,
}

/// A complete configuration for a transform pass.
///
/// The profile selects a base rule set, applies per-rule overrides, and sets
/// engine parameters. It is compiled into a [`CompiledProfile`](super::compile::CompiledProfile)
/// before execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformProfile {
    /// The base rule set to start from.
    pub ruleset: BuiltinRuleSetId,
    /// Per-rule overrides that enable or disable individual rules.
    /// A `BTreeMap` is used so that iteration order is deterministic.
    pub rules: BTreeMap<RuleKey, RuleSetting>,
    /// Safety limit for the normalize fixed-point loop.
    pub max_iterations: usize,
}

impl Default for TransformProfile {
    fn default() -> Self {
        Self {
            ruleset: BuiltinRuleSetId::Normalize,
            rules: BTreeMap::new(),
            max_iterations: 100,
        }
    }
}
