//! Built-in rewrite rule metadata.

pub use texform_transform::{RuleFidelity, RuleLevel};

use crate::RuleKey;

/// Summary of one built-in rewrite rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleInfo {
    /// Stable rule key, displayed as `package/name` (e.g. `base/over-to-frac`).
    pub key: RuleKey,
    /// The first transform profile that accepts the rule's output.
    pub level: RuleLevel,
    /// Worst-case equivalence guarantee over the rule's declared input domain.
    pub fidelity: RuleFidelity,
    /// One-line description of what the rule does.
    pub summary: String,
    /// Package names that make the rule loadable when any one of them is enabled.
    pub enabled_by_packages: Vec<String>,
}

/// List all built-in rewrite rules, sorted by key.
///
/// The keys are the names accepted by
/// [`TransformEngineBuilder::disable_rule_by_name`](crate::TransformEngineBuilder::disable_rule_by_name)
/// and reported in rewrite statistics. A rule runs only when its level is
/// selected by the engine's profile and the knowledge base enables one of its
/// packages.
pub fn list_rules() -> Vec<RuleInfo> {
    let mut rules: Vec<RuleInfo> = texform_transform::rewrite::all_rules()
        .iter()
        .map(|rule| {
            let meta = rule.meta();
            RuleInfo {
                key: meta.key,
                level: meta.level,
                fidelity: meta.fidelity,
                summary: meta.summary.to_string(),
                enabled_by_packages: meta
                    .enabled_by_packages
                    .iter()
                    .map(|package| package.as_str().to_string())
                    .collect(),
            }
        })
        .collect();
    // `RuleKey` orders by package enum discriminant, not by its displayed
    // form, so sort by the string key the bindings expose.
    rules.sort_by_cached_key(|rule| rule.key.to_string());
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_rules_covers_registry_once_sorted_by_key() {
        let rules = list_rules();
        let keys: Vec<String> = rules.iter().map(|rule| rule.key.to_string()).collect();

        assert_eq!(rules.len(), texform_transform::rewrite::all_rules().len());
        assert!(
            keys.windows(2).all(|pair| pair[0] < pair[1]),
            "keys must be sorted and unique"
        );
    }
}
