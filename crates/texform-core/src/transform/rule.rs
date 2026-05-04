//! Core rule abstractions for the transform engine.
//!
//! Every transform rule implements [`TransformRule`] and carries a static
//! [`RuleMeta`] descriptor that the engine uses for scheduling, dependency
//! analysis, and convergence checking.
//!
//! Rules are organized along four axes:
//!
//! - **Package** ([`RulePackage`]) — the owning package namespace (base, ams,
//!   physics).
//! - **Tier** ([`RuleTier`]) — how aggressive the rewrite is (base, expand,
//!   deep).
//! - **Phase** ([`RulePhase`]) — *when* the rule runs (normalize loop vs.
//!   one-shot cleanup).
//! - **Safety** ([`RuleSafety`]) — whether the transformation preserves full
//!   information, only semantic meaning, or is destructive.

use texform_specs::specs::{BuiltinCommandRecord, BuiltinEnvironmentRecord};

use crate::ast::NodeId;
use crate::transform::engine::TransformError;
use crate::transform::rule_context::RuleContext;

// NOTE: `Ord` is derived — variant declaration order determines comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RulePackage {
    Base,
    Ams,
    Physics,
    Bboldx,
    Textmacros,
}

impl RulePackage {
    pub const fn as_str(self) -> &'static str {
        match self {
            RulePackage::Base => "base",
            RulePackage::Ams => "ams",
            RulePackage::Physics => "physics",
            RulePackage::Bboldx => "bboldx",
            RulePackage::Textmacros => "textmacros",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleTier {
    Base,
    Expand,
    Deep,
}

impl RuleTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            RuleTier::Base => "base",
            RuleTier::Expand => "expand",
            RuleTier::Deep => "deep",
        }
    }
}

/// Execution phase that determines *how* a rule is scheduled.
///
/// The transform engine runs in two phases:
/// 1. **Normalize** — rules are applied repeatedly in a fixed-point loop until
///    no rule produces a change (or the iteration limit is reached).
/// 2. **Cleanup** — rules run exactly once after normalization is complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RulePhase {
    /// The rule participates in the normalize fixed-point loop.
    Normalize,
    /// The rule runs once after all normalize iterations have converged.
    Cleanup,
}

/// How much information a rule preserves when it transforms a node.
///
/// Safety levels let callers and builders describe how aggressively a rule set
/// may rewrite the AST, and they provide useful diagnostics when comparing
/// rules with different tradeoffs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleSafety {
    /// The transformation is fully reversible; no information is lost.
    Lossless,
    /// Mathematical meaning is preserved, but some non-semantic detail (e.g. spacing hints) may be discarded.
    Semantic,
    /// The transformation may lose information that affects rendering or meaning.
    Destructive,
}

/// Unique identifier for a rule, composed of its package and a human-readable name.
///
/// The `Display` impl produces the slash-separated form `"package/name"` which is
/// used in diagnostics, builder filters, and rule-selection configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleKey {
    /// The package this rule belongs to.
    pub package: RulePackage,
    /// A short, unique name within the package (e.g. `"over-to-frac"`).
    pub name: &'static str,
}

impl std::fmt::Display for RuleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.package.as_str(), self.name)
    }
}

/// A specific command or environment that a rule operates on, references, or produces.
///
/// Targets are used in [`RuleConsumes`] and [`RuleProduces`] to declare the
/// knowledge-base entries a rule interacts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTarget {
    /// A builtin command record from `texform-specs`.
    Command(&'static BuiltinCommandRecord),
    /// A builtin environment record from `texform-specs`.
    Environment(&'static BuiltinEnvironmentRecord),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleTargetKind {
    Command,
    Environment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleTargetKey {
    pub kind: RuleTargetKind,
    pub name: &'static str,
}

impl RuleTargetKey {
    pub const fn kind_label(self) -> &'static str {
        match self.kind {
            RuleTargetKind::Command => "command",
            RuleTargetKind::Environment => "environment",
        }
    }
}

impl RuleTarget {
    pub const fn key(self) -> RuleTargetKey {
        match self {
            RuleTarget::Command(record) => RuleTargetKey {
                kind: RuleTargetKind::Command,
                name: record.name,
            },
            RuleTarget::Environment(record) => RuleTargetKey {
                kind: RuleTargetKind::Environment,
                name: record.name,
            },
        }
    }

    pub const fn kind_label(self) -> &'static str {
        match self {
            RuleTarget::Command(_) => "command",
            RuleTarget::Environment(_) => "environment",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            RuleTarget::Command(record) => record.name,
            RuleTarget::Environment(record) => record.name,
        }
    }
}

impl From<RuleTarget> for RuleTargetKey {
    fn from(value: RuleTarget) -> Self {
        value.key()
    }
}

/// Declares the commands/environments that a rule removes from, reads, or may
/// otherwise touch in the AST.
///
/// The distinction matters for convergence analysis:
/// - **`eliminates`** — forms the rule actively removes or replaces. After the
///   rule fires these forms should no longer appear in the output AST.
/// - **`touches`** — forms that the rule may read or modify without promising
///   to eliminate them from the output AST.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleConsumes {
    /// Forms that the rule removes or replaces in the AST.
    pub eliminates: &'static [RuleTarget],
    /// Forms that the rule may read or modify but does not eliminate.
    pub touches: &'static [RuleTarget],
}

/// Declares the new forms that a rule may introduce into the AST.
///
/// The engine uses this to verify that every produced form is either in the
/// acceptable command set or is consumed by another rule, ensuring convergence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleProduces {
    /// Commands or environments that may appear in the AST after the rule fires.
    pub targets: &'static [RuleTarget],
}

/// Static metadata bundle that fully describes a rule's identity, scheduling,
/// and dependency contract.
///
/// The engine uses `consumes` to decide when to attempt a rule, `produces` to
/// verify convergence, and `phase`/`safety` to control scheduling and filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleMeta {
    /// Unique identifier for this rule.
    pub key: RuleKey,
    /// Tier classification used by transform profiles.
    pub tier: RuleTier,
    /// One-line human-readable description of what the rule does.
    pub summary: &'static str,
    /// The phase in which this rule executes.
    pub phase: RulePhase,
    /// The information-preservation guarantee this rule provides.
    pub safety: RuleSafety,
    /// Commands/environments this rule removes from, reads, or modifies in the AST.
    pub consumes: RuleConsumes,
    /// Commands/environments this rule may introduce into the AST.
    pub produces: RuleProduces,
}

/// Result of attempting to apply a rule to a single node.
///
/// The engine uses this to decide whether the normalize loop made progress
/// in the current iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleEffect {
    /// The rule matched and the AST was modified.
    Applied,
    /// The rule matched but the node did not require transformation.
    Skipped,
}

/// The central trait that all transform rules implement.
///
/// Implementors provide static metadata via [`meta()`](TransformRule::meta) and
/// the actual tree-rewriting logic via [`apply()`](TransformRule::apply). Rules
/// are typically defined as unit structs with a `const` [`RuleMeta`] and
/// registered in the builtin rule list under `transform/rules/mod.rs`.
pub trait TransformRule: Send + Sync {
    /// Returns the static metadata descriptor for this rule.
    fn meta(&self) -> &'static RuleMeta;

    /// Attempts to transform the node identified by `node_id`.
    ///
    /// Returns [`RuleEffect::Applied`] if the AST was modified, or
    /// [`RuleEffect::Skipped`] if the node did not need transformation.
    fn apply(
        &self,
        cx: &mut RuleContext<'_>,
        node_id: NodeId,
    ) -> Result<RuleEffect, TransformError>;
}

#[cfg(test)]
mod tests {
    use super::{RuleKey, RulePackage, RuleTier};

    #[test]
    fn rule_package_strings_match_registry_keys() {
        assert_eq!(RulePackage::Base.as_str(), "base");
        assert_eq!(RulePackage::Ams.as_str(), "ams");
        assert_eq!(RulePackage::Physics.as_str(), "physics");
        assert_eq!(RulePackage::Bboldx.as_str(), "bboldx");
        assert_eq!(RulePackage::Textmacros.as_str(), "textmacros");
    }

    #[test]
    fn rule_tier_strings_match_profile_ids() {
        assert_eq!(RuleTier::Base.as_str(), "base");
        assert_eq!(RuleTier::Expand.as_str(), "expand");
        assert_eq!(RuleTier::Deep.as_str(), "deep");
    }

    #[test]
    fn rule_key_display_is_package_and_name() {
        let key = RuleKey {
            package: RulePackage::Physics,
            name: "trace-to-tr",
        };
        assert_eq!(key.to_string(), "physics/trace-to-tr");
    }
}
