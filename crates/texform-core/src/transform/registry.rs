//! Static registry of all available transform rules.
//!
//! Every rule implementation is registered once in [`ALL_RULES`]. The
//! [`rules_for_ruleset()`] function then maps a [`BuiltinRuleSetId`] to the
//! appropriate subset of that list, which the engine compiles into an
//! executable profile.

use crate::transform::config::BuiltinRuleSetId;
use crate::transform::config::BuiltinRuleSetId::{Mer, Normalize};
use crate::transform::rule::TransformRule;
use crate::transform::rules;

/// The complete list of all registered transform rule implementations.
pub static ALL_RULES: &[&dyn TransformRule] = &[&rules::over_to_frac::OVER_TO_FRAC];

/// Returns the subset of registered rules that belong to the given rule set.
pub fn rules_for_ruleset(ruleset: BuiltinRuleSetId) -> &'static [&'static dyn TransformRule] {
    match ruleset {
        // Both rule sets currently return the full list; they will diverge as
        // more rules are added and MER-specific filtering is introduced.
        Normalize | Mer => ALL_RULES,
    }
}
