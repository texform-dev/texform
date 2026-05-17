//! User-facing transform configuration.

use crate::flatten_groups::FlattenGroupsConfig;
use crate::lower_attributes::LowerAttributesConfig;
use crate::rewrite::{RuleClassSet, RuleKey, RuleSelection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformConfig {
    pub lower_attributes: LowerAttributesConfig,
    pub rewrite: RewriteConfig,
    pub flatten_groups: FlattenGroupsConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteConfig {
    pub enabled: bool,
    pub classes: RuleClassSet,
    pub max_iterations: usize,
    pub selection: RuleSelection,
}

impl RewriteConfig {
    pub const DEFAULTS: Self = Self {
        enabled: true,
        classes: RuleClassSet::empty(),
        max_iterations: 100,
        selection: RuleSelection::All,
    };

    pub const DISABLED: Self = Self {
        enabled: false,
        classes: RuleClassSet::empty(),
        max_iterations: 100,
        selection: RuleSelection::All,
    };

    pub fn only(&mut self, key: RuleKey) -> &mut Self {
        self.selection = RuleSelection::Only(vec![key]);
        self
    }

    pub fn only_many(&mut self, keys: &[RuleKey]) -> &mut Self {
        self.selection = RuleSelection::Only(keys.to_vec());
        self
    }

    pub fn disable(&mut self, key: RuleKey) -> &mut Self {
        match &mut self.selection {
            RuleSelection::Except(keys) => {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            _ => self.selection = RuleSelection::Except(vec![key]),
        }
        self
    }

    pub fn disable_many(&mut self, keys: &[RuleKey]) -> &mut Self {
        for key in keys {
            self.disable(*key);
        }
        self
    }
}

impl TransformConfig {
    pub const AUTHORING: Self = Self {
        lower_attributes: LowerAttributesConfig::ENABLED,
        rewrite: RewriteConfig {
            enabled: true,
            classes: RuleClassSet::STANDARD,
            max_iterations: 100,
            selection: RuleSelection::All,
        },
        flatten_groups: FlattenGroupsConfig::ENABLED,
    };

    pub const CORPUS: Self = Self {
        lower_attributes: LowerAttributesConfig::ENABLED,
        rewrite: RewriteConfig {
            enabled: true,
            classes: RuleClassSet::STANDARD.union(RuleClassSet::EXPAND),
            max_iterations: 100,
            selection: RuleSelection::All,
        },
        flatten_groups: FlattenGroupsConfig::ENABLED,
    };

    pub const CORPUS_DROP: Self = Self {
        lower_attributes: LowerAttributesConfig::ENABLED,
        rewrite: RewriteConfig {
            enabled: true,
            classes: RuleClassSet::STANDARD
                .union(RuleClassSet::EXPAND)
                .union(RuleClassSet::DROP),
            max_iterations: 100,
            selection: RuleSelection::All,
        },
        flatten_groups: FlattenGroupsConfig::ENABLED,
    };

    pub const EQUIV: Self = Self {
        lower_attributes: LowerAttributesConfig::ENABLED,
        rewrite: RewriteConfig {
            enabled: true,
            classes: RuleClassSet::STANDARD
                .union(RuleClassSet::EXPAND)
                .union(RuleClassSet::DROP)
                .union(RuleClassSet::EQUIV),
            max_iterations: 100,
            selection: RuleSelection::All,
        },
        flatten_groups: FlattenGroupsConfig::ENABLED,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::RuleClass;

    #[test]
    fn presets_compile_as_const_and_carry_expected_classes() {
        assert!(
            TransformConfig::AUTHORING
                .rewrite
                .classes
                .contains(RuleClass::Standard)
        );
        assert!(
            !TransformConfig::AUTHORING
                .rewrite
                .classes
                .contains(RuleClass::Expand)
        );
        assert!(
            TransformConfig::CORPUS
                .rewrite
                .classes
                .contains(RuleClass::Expand)
        );
        assert!(
            TransformConfig::CORPUS_DROP
                .rewrite
                .classes
                .contains(RuleClass::Drop)
        );
        assert!(
            TransformConfig::EQUIV
                .rewrite
                .classes
                .contains(RuleClass::Equiv)
        );
    }
}
