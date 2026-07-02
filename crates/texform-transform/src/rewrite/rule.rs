//! Rule trait and metadata for the rewrite phase.
//!
//! Every transform rule implements [`RewriteRule`] and carries a static
//! [`RuleMeta`] descriptor that the engine uses for scheduling, dependency
//! analysis, and convergence checking.
//!
//! Rules are organized along three axes:
//!
//! - **Package** ([`PackageName`]) — the owning package namespace (base, ams,
//!   physics).
//! - **Normalization level** ([`NormalizationLevel`]) — the first transform
//!   profile that accepts the rule output as a suitable product.
//! - **Fidelity** ([`RuleFidelity`]) — the rule's worst-case render-fidelity
//!   guarantee over its declared input domain.

pub use texform_knowledge::builtin::PackageName;
use texform_knowledge::specs::{
    BuiltinCharacterRecord, BuiltinCommandRecord, BuiltinEnvironmentRecord,
};

use crate::ast::NodeId;
use crate::rewrite::RuleError;
use crate::rewrite::rule_context::RuleContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NormalizationLevel {
    /// Rules whose output is suitable for authoring-oriented normalization.
    Standard,
    /// Rules that expand compact or package-specific notation while preserving
    /// the rendered formula.
    Expand,
    /// Rules that remove layout-only forms for corpus-oriented normalization.
    Drop,
    /// Rules whose output is only suitable as an equivalence-checking
    /// intermediate, not as a corpus label.
    Equiv,
}

impl NormalizationLevel {
    /// Lowest fidelity a rule at this level may declare.
    ///
    /// `level` and `fidelity` answer different questions. `level` determines
    /// when profiles accept the rewrite output; `fidelity` is the render
    /// guarantee used for contract validation. Do not infer one from the
    /// other: an `Equiv` rule may still be `Full` when its output is
    /// pixel-identical but too expanded to serve as a corpus label, as with
    /// fenced matrix environment expansion.
    pub const fn min_fidelity(self) -> RuleFidelity {
        match self {
            NormalizationLevel::Standard | NormalizationLevel::Expand => RuleFidelity::Approximate,
            NormalizationLevel::Drop | NormalizationLevel::Equiv => RuleFidelity::Semantic,
        }
    }
}

/// How faithfully a rewrite preserves the input when re-rendered.
///
/// Ordered least-to-most faithful. The value is the rule's worst-case
/// guarantee over its declared input domain. It drives contract validation,
/// but it does not choose the image comparison algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleFidelity {
    /// Mathematical meaning is preserved, but rendering may change.
    Semantic,
    /// Rendering is visually equivalent apart from minor spacing or placement.
    Approximate,
    /// Rendering is pixel-identical before and after the rewrite.
    Full,
}

/// Unique identifier for a rule, composed of its package and a human-readable name.
///
/// The `Display` impl produces the slash-separated form `"package/name"` which is
/// used in diagnostics, builder filters, and rule-selection configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleKey {
    /// The package namespace this rule belongs to.
    pub package: PackageName,
    /// A short, unique name within the package.
    pub name: &'static str,
}

impl std::fmt::Display for RuleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.package.as_str(), self.name)
    }
}

/// A specific command, environment, or character that a rule operates on,
/// references, or produces.
///
/// Targets are used in [`RuleConsumes`] and [`RuleProduces`] to declare the
/// knowledge-base entries a rule interacts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTarget {
    /// A builtin command record from `texform-knowledge`.
    Command(&'static BuiltinCommandRecord),
    /// A builtin environment record from `texform-knowledge`.
    Environment(&'static BuiltinEnvironmentRecord),
    /// A builtin character record from `texform-knowledge`.
    Character(&'static BuiltinCharacterRecord),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleTargetKind {
    Command,
    Environment,
    Character,
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
            RuleTargetKind::Character => "character",
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
            RuleTarget::Character(record) => RuleTargetKey {
                kind: RuleTargetKind::Character,
                name: record.name,
            },
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            RuleTarget::Command(record) => record.name,
            RuleTarget::Environment(record) => record.name,
            RuleTarget::Character(record) => record.name,
        }
    }
}

impl From<RuleTarget> for RuleTargetKey {
    fn from(value: RuleTarget) -> Self {
        value.key()
    }
}

/// Declares the commands, environments, or characters that a rule removes from,
/// reads, or may otherwise touch in the AST.
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
    /// Commands, environments, or characters that may appear in the AST after the rule fires.
    pub targets: &'static [RuleTarget],
}

/// Static metadata bundle that fully describes a rule's identity, scheduling,
/// and dependency contract.
///
/// The rewrite phase uses `triggers` and `consumes` to decide when to attempt a
/// rule, `produces` to verify convergence, and `fidelity` for the rule's
/// render-fidelity contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleMeta {
    /// Unique identifier for this rule.
    pub key: RuleKey,
    /// Packages that make this rule loadable when any one of them is enabled.
    pub enabled_by_packages: &'static [PackageName],
    /// Ordered normalization level used by transform profiles.
    pub level: NormalizationLevel,
    /// One-line human-readable description of what the rule does.
    pub summary: &'static str,
    /// Worst-case render-fidelity guarantee over the rule's declared input domain.
    pub fidelity: RuleFidelity,
    /// Commands, environments, or characters that decide where the engine attempts this rule.
    ///
    /// Triggers must be non-empty. They only affect scheduling and do not
    /// participate in eliminated-form contracts or dependency analysis.
    pub triggers: &'static [RuleTarget],
    /// Commands, environments, or characters this rule removes from, reads, or modifies in the AST.
    pub consumes: RuleConsumes,
    /// Commands, environments, or characters this rule may introduce into the AST.
    pub produces: RuleProduces,
}

/// Result of attempting to apply a rule to a single node.
///
/// The engine uses this to decide whether the ApplyRules loop made progress
/// in the current iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleEffect {
    /// The rule matched and the AST was modified.
    Applied,
    /// The rule matched but the node did not require transformation.
    Skipped,
}

/// The central trait that all rewrite rules implement.
///
/// Implementors provide static metadata via [`meta()`](RewriteRule::meta) and
/// the actual tree-rewriting logic via [`apply()`](RewriteRule::apply). Rules
/// are typically defined as unit structs with a `const` [`RuleMeta`] and
/// registered in the builtin rule list under `rewrite/rules/mod.rs`.
pub trait RewriteRule: Send + Sync {
    /// Returns the static metadata descriptor for this rule.
    fn meta(&self) -> &'static RuleMeta;

    /// Attempts to transform the node identified by `node_id`.
    ///
    /// Returns [`RuleEffect::Applied`] if the AST was modified, or
    /// [`RuleEffect::Skipped`] if the node did not need transformation.
    fn apply(&self, cx: &mut RuleContext<'_>, node_id: NodeId) -> Result<RuleEffect, RuleError>;
}

impl std::fmt::Debug for dyn RewriteRule + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RewriteRule")
            .field("key", &self.meta().key)
            .finish_non_exhaustive()
    }
}
