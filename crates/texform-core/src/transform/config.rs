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
            max_iterations: Self::DEFAULT_MAX_ITERATIONS,
        }
    }
}

impl TransformProfile {
    /// Default safety limit for the normalize fixed-point loop.
    pub const DEFAULT_MAX_ITERATIONS: usize = 100;

    /// Start building a profile for the given rule set.
    pub fn builder(ruleset: BuiltinRuleSetId) -> TransformProfileBuilder {
        TransformProfileBuilder {
            ruleset,
            rules: BTreeMap::new(),
            max_iterations: TransformProfile::DEFAULT_MAX_ITERATIONS,
        }
    }
}

/// Fluent builder for [`TransformProfile`].
pub struct TransformProfileBuilder {
    ruleset: BuiltinRuleSetId,
    rules: BTreeMap<RuleKey, RuleSetting>,
    max_iterations: usize,
}

impl TransformProfileBuilder {
    /// Enable only the specified rule, setting all other rules in the
    /// ruleset to [`RuleSetting::Ignored`].
    pub fn only(mut self, rule_key: RuleKey) -> Self {
        use crate::transform::registry::rules_for_ruleset;

        let rules = rules_for_ruleset(self.ruleset);
        debug_assert!(
            rules.iter().any(|r| r.meta().key == rule_key),
            "rule_key {rule_key} not found in ruleset {:?}",
            self.ruleset
        );
        for rule in rules {
            let key = rule.meta().key;
            if key != rule_key {
                self.rules.insert(key, RuleSetting::Ignored);
            }
        }
        self
    }

    /// Consume the builder and produce a [`TransformProfile`].
    pub fn build(self) -> TransformProfile {
        TransformProfile {
            ruleset: self.ruleset,
            rules: self.rules,
            max_iterations: self.max_iterations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::registry::rules_for_ruleset;

    #[test]
    fn builder_only_enables_single_rule_and_ignores_rest() {
        let rules = rules_for_ruleset(BuiltinRuleSetId::Normalize);
        let target_key = rules[0].meta().key;
        let profile = TransformProfile::builder(BuiltinRuleSetId::Normalize)
            .only(target_key)
            .build();

        assert_eq!(profile.ruleset, BuiltinRuleSetId::Normalize);
        assert!(!profile.rules.contains_key(&target_key));
        for rule in rules.iter().skip(1) {
            assert_eq!(
                profile.rules.get(&rule.meta().key),
                Some(&RuleSetting::Ignored),
                "rule {:?} should be Ignored",
                rule.meta().key
            );
        }
    }

    #[test]
    fn builder_default_produces_empty_overrides() {
        let profile = TransformProfile::builder(BuiltinRuleSetId::Normalize).build();
        assert!(profile.rules.is_empty());
        assert_eq!(
            profile.max_iterations,
            TransformProfile::DEFAULT_MAX_ITERATIONS
        );
    }
}
