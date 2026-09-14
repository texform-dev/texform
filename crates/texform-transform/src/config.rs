//! User-facing transform configuration.

use crate::finalize_ast::FinalizeAstConfig;
use crate::flatten_groups::FlattenGroupsConfig;
use crate::lower_attributes::LowerAttributesConfig;
use crate::rewrite::plan::RuleSelection;
use crate::rewrite::{RuleKey, RuleLevelSet};

/// Per-run switches for the Rewrite phase.
///
/// Nested under [`TransformConfig::rewrite`] so Rewrite has the same shape as
/// the other pipeline stages: an enable flag plus the settings that phase
/// reads. `max_iterations` caps the fixed-point loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RewriteConfig {
    /// Run the fixed-point Rewrite loop (legacy-syntax modernization, alias
    /// canonicalization, macro expansion).
    pub enabled: bool,
    /// Upper bound on Rewrite fixed-point iterations before the run stops.
    pub max_iterations: usize,
}

impl RewriteConfig {
    /// Enabled rewrite loop with the historical 100-iteration cap.
    pub const DEFAULT: Self = Self {
        enabled: true,
        max_iterations: 100,
    };
}

/// Normalization target for a transform run.
///
/// A profile selects which rule levels are active and the default
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
    pub const fn rule_levels(self) -> RuleLevelSet {
        match self {
            Self::Authoring => RuleLevelSet::AUTHORING,
            Self::Faithful => RuleLevelSet::AUTHORING.union(RuleLevelSet::FAITHFUL),
            Self::Corpus => RuleLevelSet::AUTHORING
                .union(RuleLevelSet::FAITHFUL)
                .union(RuleLevelSet::CORPUS),
            Self::Equiv => RuleLevelSet::AUTHORING
                .union(RuleLevelSet::FAITHFUL)
                .union(RuleLevelSet::CORPUS)
                .union(RuleLevelSet::EQUIV),
        }
    }

    pub const fn default_transform_config(self) -> TransformConfig {
        match self {
            Self::Authoring | Self::Faithful => TransformConfig {
                lower_attributes: LowerAttributesConfig::ENABLED,
                rewrite: RewriteConfig::DEFAULT,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRICT,
            },
            Self::Corpus | Self::Equiv => TransformConfig {
                lower_attributes: LowerAttributesConfig::ENABLED,
                rewrite: RewriteConfig::DEFAULT,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildConfig {
    pub(crate) rule_levels: RuleLevelSet,
    pub(crate) selection: RuleSelection,
    pub(crate) default_transform: TransformConfig,
}

impl BuildConfig {
    pub fn profile(profile: Profile) -> Self {
        Self {
            rule_levels: profile.rule_levels(),
            selection: RuleSelection::All,
            default_transform: profile.default_transform_config(),
        }
    }

    pub fn rule_levels(mut self, rule_levels: RuleLevelSet) -> Self {
        self.rule_levels = rule_levels;
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

/// Per-run switches over the transform pipeline phases, in execution order.
///
/// A [`Profile`] supplies a default `TransformConfig`; override it per call to
/// toggle individual phases or cap the rewrite loop. Disabling a phase here
/// skips it without changing which rule levels the profile selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformConfig {
    /// LowerAttributes phase that canonicalizes font and style markup.
    pub lower_attributes: LowerAttributesConfig,
    /// Rewrite phase: enablement and the fixed-point iteration cap.
    pub rewrite: RewriteConfig,
    /// FinalizeAst phase (local AST cleanup such as merging adjacent `Prime`
    /// nodes).
    pub finalize_ast: FinalizeAstConfig,
    /// FlattenGroups phase that strips redundant braces behind safety guards.
    pub flatten_groups: FlattenGroupsConfig,
}
