//! Static registry of all available transform rules.
//!
//! Every rule implementation is registered once in the builtin rule list under
//! `transform/rules/mod.rs`. The [`rules_for_ruleset()`] function then maps a
//! [`BuiltinRuleSetId`] to the appropriate subset of that list, which the
//! engine compiles into an executable profile.

#[cfg(debug_assertions)]
use std::sync::Once;

use crate::transform::config::BuiltinRuleSetId;
use crate::transform::config::BuiltinRuleSetId::{Mer, Normalize};
use crate::transform::rule::TransformRule;
use crate::transform::rules::ALL_RULES;

#[cfg(debug_assertions)]
fn debug_validate_registered_rules_once() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // Touching every meta once makes macro-level debug assertions fail as
        // soon as the registry is consumed, instead of much later during rule
        // execution.
        for rule in ALL_RULES {
            let _ = rule.meta();
        }
    });
}

/// Returns the subset of registered rules that belong to the given rule set.
pub fn rules_for_ruleset(ruleset: BuiltinRuleSetId) -> &'static [&'static dyn TransformRule] {
    #[cfg(debug_assertions)]
    debug_validate_registered_rules_once();

    match ruleset {
        // Both rule sets currently return the full list; they will diverge as
        // more rules are added and MER-specific filtering is introduced.
        Normalize | Mer => ALL_RULES,
    }
}
