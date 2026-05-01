//! Static registry of all available transform rules.
//!
//! Every rule implementation is registered once in the builtin rule list under
//! `transform/rules/mod.rs`. The [`all_rules()`] accessor returns that list, and
//! [`TransformContextBuilder`](crate::transform::context::TransformContextBuilder)
//! applies profile and runtime filters on top.

#[cfg(debug_assertions)]
use std::sync::Once;

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

/// Returns every registered transform rule.
pub fn all_rules() -> &'static [&'static dyn TransformRule] {
    #[cfg(debug_assertions)]
    debug_validate_registered_rules_once();

    ALL_RULES
}
