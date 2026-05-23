//! User-facing transform configuration.

use crate::flatten_groups::FlattenGroupsConfig;
use crate::rewrite::plan::RuleSelection;
use crate::rewrite::{RuleClassSet, RuleKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Authoring,
    Corpus,
    CorpusDrop,
    Equiv,
}

impl Profile {
    pub const fn rule_classes(self) -> RuleClassSet {
        match self {
            Self::Authoring => RuleClassSet::STANDARD,
            Self::Corpus => RuleClassSet::STANDARD.union(RuleClassSet::EXPAND),
            Self::CorpusDrop => RuleClassSet::STANDARD
                .union(RuleClassSet::EXPAND)
                .union(RuleClassSet::DROP),
            Self::Equiv => RuleClassSet::STANDARD
                .union(RuleClassSet::EXPAND)
                .union(RuleClassSet::DROP)
                .union(RuleClassSet::EQUIV),
        }
    }

    pub const fn default_transform_config(self) -> TransformConfig {
        match self {
            Self::Authoring | Self::Corpus => TransformConfig {
                rewrite_enabled: true,
                lower_attributes_enabled: true,
                flatten_groups: FlattenGroupsConfig::STRICT,
                max_iterations: 100,
            },
            Self::CorpusDrop | Self::Equiv => TransformConfig {
                rewrite_enabled: true,
                lower_attributes_enabled: true,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildConfig {
    pub(crate) classes: RuleClassSet,
    pub(crate) selection: RuleSelection,
    pub(crate) default_transform: TransformConfig,
}

impl BuildConfig {
    pub fn profile(profile: Profile) -> Self {
        Self {
            classes: profile.rule_classes(),
            selection: RuleSelection::All,
            default_transform: profile.default_transform_config(),
        }
    }

    pub fn rewrite_classes(mut self, classes: RuleClassSet) -> Self {
        self.classes = classes;
        self
    }

    pub fn disable_rule(mut self, key: RuleKey) -> Self {
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

    #[doc(hidden)]
    pub fn only_rule_for_tests(mut self, key: RuleKey) -> Self {
        self.selection = RuleSelection::Only(vec![key]);
        self
    }

    #[doc(hidden)]
    pub fn only_rules_for_tests(mut self, keys: Vec<RuleKey>) -> Self {
        self.selection = RuleSelection::Only(keys);
        self
    }

    pub(crate) fn default_transform(&self) -> TransformConfig {
        self.default_transform
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformConfig {
    pub rewrite_enabled: bool,
    pub lower_attributes_enabled: bool,
    pub flatten_groups: FlattenGroupsConfig,
    pub max_iterations: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizeConfig {
    pub parse: texform_core::parse::ParseConfig,
    pub transform: TransformConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::RuleClass;

    #[test]
    fn profiles_compile_as_const_and_carry_expected_classes() {
        assert!(
            Profile::Authoring
                .rule_classes()
                .contains(RuleClass::Standard)
        );
        assert!(
            !Profile::Authoring
                .rule_classes()
                .contains(RuleClass::Expand)
        );
        assert!(Profile::Corpus.rule_classes().contains(RuleClass::Expand));
        assert!(Profile::CorpusDrop.rule_classes().contains(RuleClass::Drop));
        assert!(Profile::Equiv.rule_classes().contains(RuleClass::Equiv));
        assert!(
            Profile::Authoring
                .default_transform_config()
                .flatten_groups
                .preserve_group_adjacent_to_command_like
        );
        assert!(
            Profile::Corpus
                .default_transform_config()
                .flatten_groups
                .preserve_group_adjacent_to_command_like
        );
        assert!(
            !Profile::CorpusDrop
                .default_transform_config()
                .flatten_groups
                .preserve_group_adjacent_to_command_like
        );
        assert!(
            !Profile::Equiv
                .default_transform_config()
                .flatten_groups
                .preserve_group_adjacent_to_command_like
        );
        assert!(
            Profile::Equiv
                .default_transform_config()
                .flatten_groups
                .preserve_group_containing_infix
        );
    }
}
