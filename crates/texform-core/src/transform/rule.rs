//! Core rule abstractions for the transform engine.
//!
//! Every transform rule implements [`TransformRule`] and carries a static
//! [`RuleMeta`] descriptor that the engine uses for scheduling, dependency
//! analysis, and convergence checking.
//!
//! Rules are organized along three axes:
//!
//! - **Group** ([`RuleGroup`]) — the semantic category (physics, desugar,
//!   cleanup).
//! - **Phase** ([`RulePhase`]) — *when* the rule runs (normalize loop vs.
//!   one-shot cleanup).
//! - **Safety** ([`RuleSafety`]) — whether the transformation preserves full
//!   information, only semantic meaning, or is destructive.

use texform_specs::specs::{BuiltinCommandRecord, BuiltinEnvironmentRecord};

use crate::ast::{NodeId, NodeKind};
use crate::transform::context::TransformContext;
use crate::transform::engine::TransformError;

// NOTE: `Ord` is derived — variant declaration order determines comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleGroup {
    Physics,
    PlainTex,
    Desugar,
    FontVariant,
    SymbolAlias,
    SpacingLayout,
    MatrixEnv,
    PostNorm,
    Cleanup,
}

impl RuleGroup {
    pub const fn as_str(self) -> &'static str {
        match self {
            RuleGroup::Physics => "physics",
            RuleGroup::PlainTex => "plain_tex",
            RuleGroup::Desugar => "desugar",
            RuleGroup::FontVariant => "font_variant",
            RuleGroup::SymbolAlias => "symbol_alias",
            RuleGroup::SpacingLayout => "spacing_layout",
            RuleGroup::MatrixEnv => "matrix_env",
            RuleGroup::PostNorm => "post_norm",
            RuleGroup::Cleanup => "cleanup",
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
/// Safety levels are used by profiles to decide which rules are acceptable
/// for a given use case (e.g. MER normalization tolerates `Semantic` but
/// not `Destructive`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleSafety {
    /// The transformation is fully reversible; no information is lost.
    Lossless,
    /// Mathematical meaning is preserved, but some non-semantic detail (e.g. spacing hints) may be discarded.
    Semantic,
    /// The transformation may lose information that affects rendering or meaning.
    Destructive,
}

/// Unique identifier for a rule, composed of its group and a human-readable name.
///
/// The `Display` impl produces the slash-separated form `"group/name"` which is
/// used in diagnostics and profile overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleKey {
    /// The group this rule belongs to.
    pub group: RuleGroup,
    /// A short, unique name within the group (e.g. `"over_to_frac"`).
    pub name: &'static str,
}

impl std::fmt::Display for RuleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.group.as_str(), self.name)
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

impl RuleTarget {
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

/// Declares the commands/environments that a rule needs to exist in the
/// knowledge base and which ones it will eliminate from the AST.
///
/// The distinction matters for convergence analysis:
/// - **`eliminates`** — forms the rule actively removes or replaces. After the
///   rule fires these forms should no longer appear in the output AST.
/// - **`requires`** — forms that must be present in the knowledge base for the
///   rule to function (e.g. the rule reads metadata from them) but are *not*
///   removed from the AST.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleConsumes {
    /// Forms that the rule removes or replaces in the AST.
    pub eliminates: &'static [RuleTarget],
    /// Forms that must exist in the knowledge base but are left untouched in the AST.
    pub requires: &'static [RuleTarget],
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

/// Condition that determines when the engine should attempt to apply a rule to a node.
///
/// During tree traversal the engine checks each node against every active
/// rule's triggers. A rule fires only when at least one trigger matches. Tag-based
/// triggers (`CommandTag`, `EnvironmentTag`) allow a single rule to match a broad
/// set of commands/environments without enumerating them individually.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTrigger {
    /// Matches any node of the given AST kind.
    NodeKind(NodeKind),
    /// Matches a specific builtin command by record identity.
    Command(&'static BuiltinCommandRecord),
    /// Matches a specific builtin environment by record identity.
    Environment(&'static BuiltinEnvironmentRecord),
    /// Matches any command whose knowledge-base entry carries the given tag.
    CommandTag(&'static str),
    /// Matches any environment whose knowledge-base entry carries the given tag.
    EnvironmentTag(&'static str),
}

/// Static metadata bundle that fully describes a rule's identity, scheduling,
/// and dependency contract.
///
/// The engine uses `triggers` to decide *when* to attempt a rule, `consumes` and
/// `produces` to verify convergence, and `phase`/`safety` to control scheduling
/// and filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleMeta {
    /// Unique identifier for this rule.
    pub key: RuleKey,
    /// One-line human-readable description of what the rule does.
    pub summary: &'static str,
    /// The phase in which this rule executes.
    pub phase: RulePhase,
    /// The information-preservation guarantee this rule provides.
    pub safety: RuleSafety,
    /// Conditions under which the engine will attempt to apply this rule to a node.
    pub triggers: &'static [RuleTrigger],
    /// Commands/environments this rule removes from or depends on in the AST.
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
    /// The rule's trigger matched but the node did not require transformation.
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
        cx: &mut TransformContext<'_>,
        node_id: NodeId,
    ) -> Result<RuleEffect, TransformError>;
}

#[cfg(test)]
mod tests {
    use super::RuleGroup;

    #[test]
    fn rule_group_strings_match_registry_keys() {
        assert_eq!(RuleGroup::Physics.as_str(), "physics");
        assert_eq!(RuleGroup::PlainTex.as_str(), "plain_tex");
        assert_eq!(RuleGroup::Desugar.as_str(), "desugar");
        assert_eq!(RuleGroup::FontVariant.as_str(), "font_variant");
        assert_eq!(RuleGroup::SymbolAlias.as_str(), "symbol_alias");
        assert_eq!(RuleGroup::SpacingLayout.as_str(), "spacing_layout");
        assert_eq!(RuleGroup::MatrixEnv.as_str(), "matrix_env");
        assert_eq!(RuleGroup::PostNorm.as_str(), "post_norm");
        assert_eq!(RuleGroup::Cleanup.as_str(), "cleanup");
    }
}
