//! User-facing transform configuration.

use crate::finalize_ast::FinalizeAstConfig;
use crate::flatten_groups::FlattenGroupsConfig;
use crate::rewrite::plan::RuleSelection;
use crate::rewrite::{NormalizationLevelSet, RuleKey};

/// Normalization target for a transform run.
///
/// A profile selects which normalization levels are active and the default
/// per-run [`TransformConfig`]. Normalization has no single correct answer, so
/// each profile canonicalizes for one downstream scenario rather than imposing
/// one true form. The levels are cumulative: each profile in this list enables
/// everything the previous one does, plus more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Polished author-facing output; stylistic choices are kept.
    Authoring,
    /// Same rendered formula, with convenience macros expanded into universal
    /// forms.
    Faithful,
    /// Training-data normalization; layout hints are dropped.
    Corpus,
    /// Aggressive canonicalization for formula equivalence comparison.
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

/// Per-run switches over the transform pipeline phases.
///
/// A [`Profile`] supplies a default `TransformConfig`; override it per call to
/// toggle individual phases or cap the rewrite loop. Disabling a phase here
/// skips it without changing which normalization levels the profile selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformConfig {
    /// Run the fixed-point Rewrite loop (legacy-syntax modernization, alias
    /// canonicalization, macro expansion).
    pub rewrite_enabled: bool,
    /// Run the LowerAttributes phase that canonicalizes font and style markup.
    pub lower_attributes_enabled: bool,
    /// Configuration for the FinalizeAst phase (local AST cleanup such as
    /// merging adjacent `Prime` nodes).
    pub finalize_ast: FinalizeAstConfig,
    /// Configuration for the FlattenGroups phase that strips redundant braces
    /// behind safety guards.
    pub flatten_groups: FlattenGroupsConfig,
    /// Upper bound on Rewrite fixed-point iterations before the run stops.
    pub max_iterations: usize,
}
