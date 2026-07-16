//! Static registry of all available rewrite rules.

#[cfg(debug_assertions)]
use std::sync::Once;

use crate::rewrite::rule::RewriteRule;
use crate::rewrite::rules::ALL_RULES;

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

/// Returns every registered rewrite rule.
pub fn all_rules() -> &'static [&'static dyn RewriteRule] {
    #[cfg(debug_assertions)]
    debug_validate_registered_rules_once();
    ALL_RULES
}
