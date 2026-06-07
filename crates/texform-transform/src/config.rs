//! User-facing transform configuration.

use crate::finalize_ast::FinalizeAstConfig;
use crate::flatten_groups::FlattenGroupsConfig;
use crate::rewrite::plan::RuleSelection;
use crate::rewrite::{NormalizationLevelSet, RuleKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Authoring,
    Faithful,
    Corpus,
    Equiv,
}

impl Profile {
    pub const fn normalization_levels(self) -> NormalizationLevelSet {
        match self {
            Self::Authoring => NormalizationLevelSet::STANDARD,
            Self::Faithful => NormalizationLevelSet::STANDARD.union(NormalizationLevelSet::EXPAND),
            Self::Corpus => NormalizationLevelSet::STANDARD
                .union(NormalizationLevelSet::EXPAND)
                .union(NormalizationLevelSet::DROP),
            Self::Equiv => NormalizationLevelSet::STANDARD
                .union(NormalizationLevelSet::EXPAND)
                .union(NormalizationLevelSet::DROP)
                .union(NormalizationLevelSet::EQUIV),
        }
    }

    pub const fn default_transform_config(self) -> TransformConfig {
        match self {
            Self::Authoring | Self::Faithful => TransformConfig {
                rewrite_enabled: true,
                lower_attributes_enabled: true,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRICT,
                max_iterations: 100,
            },
            Self::Corpus | Self::Equiv => TransformConfig {
                rewrite_enabled: true,
                lower_attributes_enabled: true,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildConfig {
    pub(crate) levels: NormalizationLevelSet,
    pub(crate) selection: RuleSelection,
    pub(crate) default_transform: TransformConfig,
}

impl BuildConfig {
    pub fn profile(profile: Profile) -> Self {
        Self {
            levels: profile.normalization_levels(),
            selection: RuleSelection::All,
            default_transform: profile.default_transform_config(),
        }
    }

    pub fn rewrite_levels(mut self, levels: NormalizationLevelSet) -> Self {
        self.levels = levels;
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
    pub finalize_ast: FinalizeAstConfig,
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
    use crate::rewrite::NormalizationLevel;

    #[test]
    fn profiles_compile_as_const_and_carry_expected_levels() {
        assert!(
            Profile::Authoring
                .normalization_levels()
                .contains(NormalizationLevel::Standard)
        );
        assert!(
            !Profile::Authoring
                .normalization_levels()
                .contains(NormalizationLevel::Expand)
        );
        assert!(
            Profile::Faithful
                .normalization_levels()
                .contains(NormalizationLevel::Expand)
        );
        assert!(
            !Profile::Faithful
                .normalization_levels()
                .contains(NormalizationLevel::Drop)
        );
        assert!(
            Profile::Corpus
                .normalization_levels()
                .contains(NormalizationLevel::Drop)
        );
        assert!(
            !Profile::Corpus
                .normalization_levels()
                .contains(NormalizationLevel::Equiv)
        );
        assert!(
            Profile::Equiv
                .normalization_levels()
                .contains(NormalizationLevel::Equiv)
        );
        assert!(
            Profile::Authoring
                .default_transform_config()
                .finalize_ast
                .enabled
        );
        assert!(
            Profile::Equiv
                .default_transform_config()
                .finalize_ast
                .enabled
        );
        assert!(
            Profile::Authoring
                .default_transform_config()
                .flatten_groups
                .preserve_group_adjacent_to_command_like
        );
        assert!(
            Profile::Faithful
                .default_transform_config()
                .flatten_groups
                .preserve_group_adjacent_to_command_like
        );
        assert!(
            !Profile::Corpus
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
