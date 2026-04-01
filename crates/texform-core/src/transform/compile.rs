//! Compiles a [`TransformProfile`] into a [`CompiledProfile`] ready for execution.
//!
//! Compilation performs several validation and ordering steps:
//!
//! 1. **Rule availability** — checks that every command and environment referenced
//!    by a rule exists in the knowledge base with compatible metadata.
//! 2. **Setting resolution** — merges per-rule overrides from the profile with the
//!    default setting (`On`).
//! 3. **Phase splitting** — partitions enabled rules into normalize and cleanup phases.
//! 4. **Topological sort** — orders rules within each phase so that a rule producing
//!    a form always runs before rules that consume that form.
//! 5. **Cleanup boundary validation** — ensures cleanup rules do not produce forms
//!    that normalize rules consume (which would create a cross-phase infinite loop).
//! 6. **Contract derivation** — builds the set of forms that must be absent from the
//!    output AST after transformation.

use std::collections::VecDeque;

use texform_specs::specs::{BuiltinCommandRecord, BuiltinEnvironmentRecord};

use crate::knowledge::KnowledgeBase;
use crate::transform::config::{BuiltinRuleSetId, RuleSetting, TransformProfile};
use crate::transform::registry::rules_for_ruleset;
use crate::transform::rule::{RuleKey, RulePhase, RuleTarget, TransformRule};

/// Whether a rule can be used with the current knowledge base.
///
/// A rule is available only when every command and environment it references
/// exists in the knowledge base with matching metadata (kind, spec, mode).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleAvailability {
    /// All referenced targets exist and are compatible.
    Available,
    /// A referenced command or environment is not present in the knowledge base.
    TargetAbsent {
        kind: &'static str,
        name: &'static str,
    },
    /// The target exists but its active metadata does not match what the rule expects.
    IncompatibleActive {
        kind: &'static str,
        name: &'static str,
        reason: &'static str,
    },
    /// The target exists but resolves to a different semantic class than expected.
    WrongActiveSemanticClass {
        kind: &'static str,
        name: &'static str,
        expected: &'static str,
    },
}

/// Combines a rule reference with its resolved availability and configuration setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleStatus {
    /// The identity of the rule.
    pub key: RuleKey,
    /// The resolved setting after merging profile overrides with the default.
    pub config: RuleSetting,
    /// Whether the rule can actually execute against the current knowledge base.
    pub availability: RuleAvailability,
}

/// Defines what the output AST should look like after transformation.
///
/// The contract lists all forms (commands/environments) that must be absent
/// from the final AST. The engine validates this after both phases complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalFormContract {
    /// Forms that no enabled rule should leave in the output AST.
    pub eliminated_forms: Vec<RuleTarget>,
}

/// An ordered sequence of rules for one execution phase.
///
/// Rules are topologically sorted so that producers run before consumers.
#[derive(Clone)]
pub struct CompiledPhase {
    /// Which phase this belongs to (normalize or cleanup).
    pub phase: RulePhase,
    /// Rules in dependency-respecting execution order.
    pub ordered_rules: Vec<&'static dyn TransformRule>,
}

/// The fully compiled profile, ready to be fed into the transform engine.
#[derive(Clone)]
pub struct CompiledProfile {
    /// The normalize phase with topologically sorted rules.
    pub normalize_phase: CompiledPhase,
    /// The optional cleanup phase, present only when cleanup rules exist.
    pub cleanup_phase: Option<CompiledPhase>,
    /// Availability and setting status for every rule in the ruleset.
    pub statuses: Vec<RuleStatus>,
    /// The contract that the output AST must satisfy.
    pub contract: NormalFormContract,
    /// Maximum number of normalize iterations before the engine reports an error.
    pub max_iterations: usize,
}

/// All the ways profile compilation can fail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileCompileError {
    /// The profile specifies a setting override for a rule that does not exist in the ruleset.
    UnknownRuleOverride { key: RuleKey },
    /// The ruleset has no registered rules, or all rules were filtered out.
    EmptyRuleSet { ruleset: BuiltinRuleSetId },
    /// The dependency graph among rules within a phase contains a cycle.
    CycleDetected { chain: Vec<RuleKey> },
    /// A cleanup rule produces a form consumed by a normalize rule, which would create an infinite loop across phases.
    CleanupProducesNormalizeInput {
        cleanup_rule: RuleKey,
        normalize_rule: RuleKey,
    },
}

impl std::fmt::Display for ProfileCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileCompileError::UnknownRuleOverride { key } => {
                write!(f, "unknown transform rule override: {key}")
            }
            ProfileCompileError::EmptyRuleSet { ruleset } => {
                write!(f, "transform ruleset {:?} has no registered rules", ruleset)
            }
            ProfileCompileError::CycleDetected { chain } => {
                let chain = chain
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                write!(
                    f,
                    "failed to compile transform profile: cycle detected: {chain}"
                )
            }
            ProfileCompileError::CleanupProducesNormalizeInput {
                cleanup_rule,
                normalize_rule,
            } => write!(
                f,
                "cleanup rule {cleanup_rule} produces input consumed by normalize rule {normalize_rule}"
            ),
        }
    }
}

impl std::error::Error for ProfileCompileError {}

/// Compiles a [`TransformProfile`] into a [`CompiledProfile`] ready for the engine.
///
/// The compilation pipeline: load rules from the ruleset, validate overrides,
/// build per-rule statuses (availability + setting), filter to enabled rules,
/// split by phase, topologically sort each phase, validate the cleanup boundary,
/// and derive the normal-form contract.
pub fn compile_profile(
    kb: &KnowledgeBase,
    profile: &TransformProfile,
) -> Result<CompiledProfile, ProfileCompileError> {
    let all_rules = rules_for_ruleset(profile.ruleset);
    if all_rules.is_empty() {
        return Err(ProfileCompileError::EmptyRuleSet {
            ruleset: profile.ruleset,
        });
    }

    let known_rule_keys = all_rules
        .iter()
        .map(|rule| rule.meta().key)
        .collect::<Vec<_>>();
    for key in profile.rules.keys() {
        if !known_rule_keys.contains(key) {
            return Err(ProfileCompileError::UnknownRuleOverride { key: *key });
        }
    }

    let statuses = all_rules
        .iter()
        .map(|rule| {
            let key = rule.meta().key;
            // Default setting is On when the profile does not explicitly configure this rule.
            let config = profile.rules.get(&key).copied().unwrap_or(RuleSetting::On);
            let availability = if matches!(config, RuleSetting::Ignored) {
                RuleAvailability::Available
            } else {
                validate_rule_availability(kb, rule)
            };
            RuleStatus {
                key,
                config,
                availability,
            }
        })
        .collect::<Vec<_>>();

    let enabled_rules = all_rules
        .iter()
        .filter(|rule| {
            statuses.iter().any(|status| {
                status.key == rule.meta().key
                    && matches!(status.config, RuleSetting::On)
                    && matches!(status.availability, RuleAvailability::Available)
            })
        })
        .copied()
        .collect::<Vec<_>>();

    if enabled_rules.is_empty() {
        return Err(ProfileCompileError::EmptyRuleSet {
            ruleset: profile.ruleset,
        });
    }

    let normalize_rules = enabled_rules
        .iter()
        .filter(|rule| matches!(rule.meta().phase, RulePhase::Normalize))
        .copied()
        .collect::<Vec<_>>();
    let cleanup_rules = enabled_rules
        .iter()
        .filter(|rule| matches!(rule.meta().phase, RulePhase::Cleanup))
        .copied()
        .collect::<Vec<_>>();

    let normalize_phase = CompiledPhase {
        phase: RulePhase::Normalize,
        ordered_rules: topological_sort(normalize_rules.as_slice())?,
    };
    let cleanup_phase = if cleanup_rules.is_empty() {
        None
    } else {
        Some(CompiledPhase {
            phase: RulePhase::Cleanup,
            ordered_rules: topological_sort(cleanup_rules.as_slice())?,
        })
    };

    validate_cleanup_boundary(
        normalize_phase.ordered_rules.as_slice(),
        cleanup_phase
            .as_ref()
            .map(|phase| phase.ordered_rules.as_slice())
            .unwrap_or(&[]),
    )?;

    Ok(CompiledProfile {
        normalize_phase,
        cleanup_phase,
        statuses,
        contract: derive_contract(enabled_rules.as_slice()),
        max_iterations: profile.max_iterations,
    })
}

/// Checks that every consumed or produced target referenced by the rule exists
/// in the knowledge base with compatible metadata.
///
/// Triggers are intentionally excluded: they use OR semantics at runtime and
/// do not participate in topo-sort or contract derivation, so a missing trigger
/// command is a no-op, not an availability error.
fn validate_rule_availability(
    kb: &KnowledgeBase,
    rule: &&'static dyn TransformRule,
) -> RuleAvailability {
    let targets = rule
        .meta()
        .consumes
        .eliminates
        .iter()
        .chain(rule.meta().consumes.requires.iter())
        .chain(rule.meta().produces.targets.iter())
        .copied()
        .collect::<Vec<_>>();

    validate_targets_with_variants(kb, targets.as_slice())
}

fn validate_target(kb: &KnowledgeBase, target: RuleTarget) -> RuleAvailability {
    match target {
        RuleTarget::Command(record) => validate_command_target(kb, record),
        RuleTarget::Environment(record) => validate_environment_target(kb, record),
    }
}

/// A rule targeting a command like `\frac` must work regardless of which package
/// (`base` or `ams`) provides it in the active KB. To achieve this, rule authors
/// list all package variants in their metadata, and this function groups them by
/// `(kind, name)`: the group passes when *any* variant matches the active record.
fn validate_targets_with_variants(kb: &KnowledgeBase, targets: &[RuleTarget]) -> RuleAvailability {
    let mut groups = Vec::<Vec<RuleTarget>>::new();
    for &target in targets {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| same_target_form(group[0], target))
        {
            group.push(target);
        } else {
            groups.push(vec![target]);
        }
    }

    for group in groups {
        let Some((&reference, variants)) = group.split_first() else {
            continue;
        };

        for &variant in variants {
            let structurally_compatible = match (reference, variant) {
                (RuleTarget::Command(left), RuleTarget::Command(right)) => {
                    left.name == right.name
                        && left.kind == right.kind
                        && left.spec_string == right.spec_string
                }
                (RuleTarget::Environment(left), RuleTarget::Environment(right)) => {
                    left.name == right.name
                        && left.spec_string == right.spec_string
                        && left.body_mode == right.body_mode
                }
                _ => false,
            };
            // A variant group only makes sense when every listed package record
            // describes the same target shape. If authors mix incompatible
            // records under one name, that is a rule-definition bug, not a KB
            // availability problem.
            debug_assert!(
                structurally_compatible,
                "package-variant targets are structurally incompatible: reference={reference:?}, variant={variant:?}",
            );
        }

        let mut last_failure = RuleAvailability::Available;
        let mut matched_variant = false;
        for variant in group {
            let availability = validate_target(kb, variant);
            if matches!(availability, RuleAvailability::Available) {
                matched_variant = true;
                break;
            }
            last_failure = availability;
        }

        if !matched_variant {
            return last_failure;
        }
    }

    RuleAvailability::Available
}

fn validate_command_target(
    kb: &KnowledgeBase,
    record: &'static BuiltinCommandRecord,
) -> RuleAvailability {
    if kb.lookup_command(record.name).is_none() {
        return RuleAvailability::TargetAbsent {
            kind: "command",
            name: record.name,
        };
    }

    let Some(explicit) = kb.lookup_active_explicit_command(record.name) else {
        return RuleAvailability::WrongActiveSemanticClass {
            kind: "command",
            name: record.name,
            expected: "explicit-command",
        };
    };
    if !record.structurally_compatible_with(explicit) {
        if explicit.kind != record.kind {
            return RuleAvailability::IncompatibleActive {
                kind: "command",
                name: record.name,
                reason: "kind mismatch",
            };
        }
        if explicit.spec_string != record.spec_string {
            return RuleAvailability::IncompatibleActive {
                kind: "command",
                name: record.name,
                reason: "spec_string mismatch",
            };
        }
        // name always matches (by-name lookup), so incompatibility
        // can only come from kind or spec_string — both checked above.
        unreachable!("structurally_compatible_with fields exhausted");
    }

    RuleAvailability::Available
}

fn validate_environment_target(
    kb: &KnowledgeBase,
    record: &'static BuiltinEnvironmentRecord,
) -> RuleAvailability {
    let Some(active) = kb.lookup_env(record.name) else {
        return RuleAvailability::TargetAbsent {
            kind: "environment",
            name: record.name,
        };
    };
    if !record.structurally_compatible_with(active) {
        if active.spec_string != record.spec_string {
            return RuleAvailability::IncompatibleActive {
                kind: "environment",
                name: record.name,
                reason: "spec_string mismatch",
            };
        }
        if active.body_mode != record.body_mode {
            return RuleAvailability::IncompatibleActive {
                kind: "environment",
                name: record.name,
                reason: "body_mode mismatch",
            };
        }
        // name always matches (by-name lookup), so incompatibility
        // can only come from spec_string or body_mode — both checked above.
        unreachable!("structurally_compatible_with fields exhausted");
    }

    RuleAvailability::Available
}

/// Builds the normal-form contract by collecting all forms that at least one
/// enabled rule declares it eliminates. Any such form remaining in the output
/// AST after transformation constitutes a contract violation.
fn derive_contract(enabled_rules: &[&'static dyn TransformRule]) -> NormalFormContract {
    let mut eliminated_forms = Vec::new();
    for rule in enabled_rules {
        for target in rule.meta().consumes.eliminates {
            if !eliminated_forms
                .iter()
                .any(|existing| same_target_form(*existing, *target))
            {
                eliminated_forms.push(*target);
            }
        }
    }
    NormalFormContract { eliminated_forms }
}

/// Ensures that no cleanup rule produces a form consumed by any normalize rule.
///
/// If a cleanup rule were to produce such a form, the engine would need to
/// re-enter the normalize loop after cleanup, creating a potential infinite cycle.
fn validate_cleanup_boundary(
    normalize_rules: &[&'static dyn TransformRule],
    cleanup_rules: &[&'static dyn TransformRule],
) -> Result<(), ProfileCompileError> {
    for cleanup_rule in cleanup_rules {
        for produced in cleanup_rule.meta().produces.targets {
            for normalize_rule in normalize_rules {
                let mut consumes = normalize_rule
                    .meta()
                    .consumes
                    .eliminates
                    .iter()
                    .chain(normalize_rule.meta().consumes.requires.iter());
                if consumes.any(|consumed| same_target_form(*produced, *consumed)) {
                    return Err(ProfileCompileError::CleanupProducesNormalizeInput {
                        cleanup_rule: cleanup_rule.meta().key,
                        normalize_rule: normalize_rule.meta().key,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Orders rules using Kahn's algorithm so that producers run before consumers.
///
/// An edge from rule A to rule B means A's `produces` overlaps with B's
/// `consumes` — therefore A must execute before B. If the graph contains a
/// cycle, the sort fails and the cycle is reported via [`detect_cycle`].
fn topological_sort(
    rules: &[&'static dyn TransformRule],
) -> Result<Vec<&'static dyn TransformRule>, ProfileCompileError> {
    let mut incoming = vec![0usize; rules.len()];
    let mut edges = vec![Vec::<usize>::new(); rules.len()];

    // Build the dependency graph: edge from_rule → to_rule means
    // from_rule produces a form that to_rule consumes.
    for (from_index, from_rule) in rules.iter().enumerate() {
        for (to_index, to_rule) in rules.iter().enumerate() {
            if from_index == to_index {
                continue;
            }
            if rules_intersect(from_rule, to_rule) {
                edges[from_index].push(to_index);
                incoming[to_index] += 1;
            }
        }
    }

    let mut queue = VecDeque::new();
    for (index, &count) in incoming.iter().enumerate() {
        if count == 0 {
            queue.push_back(index);
        }
    }

    let mut ordered = Vec::with_capacity(rules.len());
    while let Some(index) = queue.pop_front() {
        ordered.push(rules[index]);
        for next in &edges[index] {
            incoming[*next] -= 1;
            if incoming[*next] == 0 {
                queue.push_back(*next);
            }
        }
    }

    if ordered.len() == rules.len() {
        return Ok(ordered);
    }

    Err(ProfileCompileError::CycleDetected {
        chain: detect_cycle(rules, edges.as_slice()),
    })
}

/// Determines whether a dependency edge should exist from `from_rule` to `to_rule`.
///
/// Returns `true` when `from_rule` produces at least one form that `to_rule`
/// consumes (either eliminates or requires), meaning `from_rule` must run first.
fn rules_intersect(
    from_rule: &&'static dyn TransformRule,
    to_rule: &&'static dyn TransformRule,
) -> bool {
    from_rule.meta().produces.targets.iter().any(|produced| {
        to_rule
            .meta()
            .consumes
            .eliminates
            .iter()
            .chain(to_rule.meta().consumes.requires.iter())
            .any(|consumed| same_target_form(*produced, *consumed))
    })
}

/// Returns `true` if two rule targets refer to the same command or environment by name.
fn same_target_form(left: RuleTarget, right: RuleTarget) -> bool {
    left.kind_label() == right.kind_label() && left.name() == right.name()
}

/// Uses DFS to find and report a concrete cycle in the dependency graph.
///
/// Each node has one of three states: 0 = unvisited, 1 = in current DFS path,
/// 2 = fully explored. Encountering state 1 means we have found a back-edge
/// and can extract the cycle from the recursion stack.
fn detect_cycle(rules: &[&'static dyn TransformRule], edges: &[Vec<usize>]) -> Vec<RuleKey> {
    let mut stack = Vec::new();
    let mut state = vec![0u8; rules.len()];

    for index in 0..rules.len() {
        if let Some(chain) = visit_cycle(index, rules, edges, &mut state, &mut stack) {
            return chain;
        }
    }

    rules.iter().map(|rule| rule.meta().key).collect()
}

fn visit_cycle(
    index: usize,
    rules: &[&'static dyn TransformRule],
    edges: &[Vec<usize>],
    state: &mut [u8],
    stack: &mut Vec<usize>,
) -> Option<Vec<RuleKey>> {
    if state[index] == 1 {
        let cycle_start = stack.iter().position(|node| *node == index).unwrap_or(0);
        let mut chain = stack[cycle_start..]
            .iter()
            .map(|node| rules[*node].meta().key)
            .collect::<Vec<_>>();
        chain.push(rules[index].meta().key);
        return Some(chain);
    }

    if state[index] == 2 {
        return None;
    }

    state[index] = 1;
    stack.push(index);
    for &next in &edges[index] {
        if let Some(chain) = visit_cycle(next, rules, edges, state, stack) {
            return Some(chain);
        }
    }
    stack.pop();
    state[index] = 2;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::NodeId;
    use crate::knowledge::KnowledgeBase;
    use crate::transform::context::TransformContext;
    use crate::transform::engine::TransformError;
    use crate::transform::rule::{
        RuleConsumes, RuleEffect, RuleGroup, RuleMeta, RuleProduces, RuleSafety, RuleTrigger,
        TransformRule,
    };
    use texform_specs::specs::{
        AllowedMode, BuiltinCommandRecord, CharacterAttributes, CharacterSpec, CommandKind,
        CommandSpec, PackageSpecs,
    };

    struct MockRule {
        meta: &'static RuleMeta,
    }

    impl TransformRule for MockRule {
        fn meta(&self) -> &'static RuleMeta {
            self.meta
        }

        fn apply(
            &self,
            _cx: &mut TransformContext<'_>,
            _node_id: NodeId,
        ) -> Result<RuleEffect, TransformError> {
            Ok(RuleEffect::Skipped)
        }
    }

    static COMMAND_A: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "rule-a-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        args: &[],
        tags: &[],
        spec_string: "",
    };

    static COMMAND_B: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "rule-b-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        args: &[],
        tags: &[],
        spec_string: "",
    };

    static COMMAND_C: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "rule-c-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        args: &[],
        tags: &[],
        spec_string: "",
    };

    static ACTIVE_EXPLICIT_COMMAND: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "foo",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        args: &[],
        tags: &[],
        spec_string: "",
    };

    static SHARED_VARIANT_BASE: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "shared-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        args: &[],
        tags: &[],
        spec_string: "m",
    };

    static SHARED_VARIANT_AMS: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "shared-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Both,
        args: &[],
        tags: &[],
        spec_string: "m",
    };

    static SHARED_VARIANT_BROKEN: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "shared-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        args: &[],
        tags: &[],
        spec_string: "m m",
    };

    static MISSING_TRIGGER_COMMAND: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "missing-trigger",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        args: &[],
        tags: &[],
        spec_string: "",
    };

    static SHARED_VARIANT_TARGETS: [RuleTarget; 2] = [
        RuleTarget::Command(&SHARED_VARIANT_BASE),
        RuleTarget::Command(&SHARED_VARIANT_AMS),
    ];

    static BROKEN_VARIANT_TARGETS: [RuleTarget; 2] = [
        RuleTarget::Command(&SHARED_VARIANT_BASE),
        RuleTarget::Command(&SHARED_VARIANT_BROKEN),
    ];

    static SINGLE_VARIANT_TARGETS: [RuleTarget; 1] = [RuleTarget::Command(&SHARED_VARIANT_BASE)];

    static RULE_A_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Canonical,
            name: "a",
        },
        summary: "mock rule a",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[],
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_C)],
            requires: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_A)],
        },
    };

    static RULE_B_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Canonical,
            name: "b",
        },
        summary: "mock rule b",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[],
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_A)],
            requires: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_B)],
        },
    };

    static RULE_C_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Canonical,
            name: "c",
        },
        summary: "mock rule c",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[],
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_B)],
            requires: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_C)],
        },
    };

    static RULE_A: MockRule = MockRule { meta: &RULE_A_META };
    static RULE_B: MockRule = MockRule { meta: &RULE_B_META };
    static RULE_C: MockRule = MockRule { meta: &RULE_C_META };

    static MULTI_VARIANT_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Canonical,
            name: "multi-variant",
        },
        summary: "mock multi-variant rule",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[],
        consumes: RuleConsumes {
            eliminates: &[],
            requires: &[],
        },
        produces: RuleProduces {
            targets: &SHARED_VARIANT_TARGETS,
        },
    };

    static SINGLE_VARIANT_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Canonical,
            name: "single-variant",
        },
        summary: "mock single-variant rule",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[],
        consumes: RuleConsumes {
            eliminates: &[],
            requires: &[],
        },
        produces: RuleProduces {
            targets: &SINGLE_VARIANT_TARGETS,
        },
    };

    static TRIGGER_IGNORED_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Canonical,
            name: "trigger-ignored",
        },
        summary: "mock rule with a missing trigger",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[RuleTrigger::Command(&MISSING_TRIGGER_COMMAND)],
        consumes: RuleConsumes {
            eliminates: &[],
            requires: &[],
        },
        produces: RuleProduces {
            targets: &SINGLE_VARIANT_TARGETS,
        },
    };

    static DUPLICATE_ELIMINATE_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            group: RuleGroup::Canonical,
            name: "duplicate-eliminate",
        },
        summary: "mock rule with duplicate eliminate variants",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        triggers: &[],
        consumes: RuleConsumes {
            eliminates: &SHARED_VARIANT_TARGETS,
            requires: &[],
        },
        produces: RuleProduces { targets: &[] },
    };

    static MULTI_VARIANT_RULE: MockRule = MockRule {
        meta: &MULTI_VARIANT_RULE_META,
    };
    static SINGLE_VARIANT_RULE: MockRule = MockRule {
        meta: &SINGLE_VARIANT_RULE_META,
    };
    static TRIGGER_IGNORED_RULE: MockRule = MockRule {
        meta: &TRIGGER_IGNORED_RULE_META,
    };
    static DUPLICATE_ELIMINATE_RULE: MockRule = MockRule {
        meta: &DUPLICATE_ELIMINATE_RULE_META,
    };

    #[test]
    fn validate_command_target_rejects_character_backed_active_command() {
        let mut kb = KnowledgeBase::empty();
        kb.insert_or_override_command(CommandSpec {
            name: "foo".to_string(),
            kind: CommandKind::Prefix,
            allowed_mode: AllowedMode::Math,
            args: vec![],
            tags: vec![],
            spec_string: "".to_string(),
        });
        kb.import_package(PackageSpecs {
            characters: vec![CharacterSpec {
                name: "foo".to_string(),
                allowed_mode: AllowedMode::Text,
                unicode_value: "ƒ".to_string(),
                attributes: CharacterAttributes::default(),
            }],
            commands: vec![],
            environments: vec![],
            delimiter_controls: vec![],
        });

        assert_eq!(
            validate_command_target(&kb, &ACTIVE_EXPLICIT_COMMAND),
            RuleAvailability::WrongActiveSemanticClass {
                kind: "command",
                name: "foo",
                expected: "explicit-command",
            }
        );
    }

    #[test]
    fn topological_sort_reports_concrete_cycle_chain() {
        let rules: [&'static dyn TransformRule; 3] = [&RULE_A, &RULE_B, &RULE_C];

        let error = match topological_sort(rules.as_slice()) {
            Ok(_) => panic!("expected a cycle"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            ProfileCompileError::CycleDetected {
                chain: vec![
                    RULE_A_META.key,
                    RULE_B_META.key,
                    RULE_C_META.key,
                    RULE_A_META.key
                ],
            }
        );
    }

    #[test]
    fn validate_rule_availability_accepts_structurally_compatible_package_variants() {
        let mut kb = KnowledgeBase::empty();
        kb.insert_or_override_command(CommandSpec {
            name: "shared-target".to_string(),
            kind: CommandKind::Prefix,
            allowed_mode: AllowedMode::Math,
            args: vec![],
            tags: vec![],
            spec_string: "m".to_string(),
        });

        let rule: &'static dyn TransformRule = &MULTI_VARIANT_RULE;
        assert_eq!(
            validate_rule_availability(&kb, &rule),
            RuleAvailability::Available
        );
    }

    #[test]
    fn validate_rule_availability_rejects_absent_package_variant_groups() {
        let rule: &'static dyn TransformRule = &MULTI_VARIANT_RULE;
        assert_eq!(
            validate_rule_availability(&KnowledgeBase::empty(), &rule),
            RuleAvailability::TargetAbsent {
                kind: "command",
                name: "shared-target",
            }
        );
    }

    #[test]
    #[should_panic(expected = "structurally")]
    fn validate_rule_availability_panics_for_structurally_inconsistent_package_variants() {
        let _ = validate_targets_with_variants(&KnowledgeBase::empty(), &BROKEN_VARIANT_TARGETS);
    }

    #[test]
    fn validate_rule_availability_ignores_missing_triggers() {
        let mut kb = KnowledgeBase::empty();
        kb.insert_or_override_command(CommandSpec {
            name: "shared-target".to_string(),
            kind: CommandKind::Prefix,
            allowed_mode: AllowedMode::Math,
            args: vec![],
            tags: vec![],
            spec_string: "m".to_string(),
        });

        let rule: &'static dyn TransformRule = &TRIGGER_IGNORED_RULE;
        assert_eq!(
            validate_rule_availability(&kb, &rule),
            RuleAvailability::Available
        );
    }

    #[test]
    fn validate_rule_availability_preserves_single_variant_behavior() {
        let mut kb = KnowledgeBase::empty();
        kb.insert_or_override_command(CommandSpec {
            name: "shared-target".to_string(),
            kind: CommandKind::Prefix,
            allowed_mode: AllowedMode::Math,
            args: vec![],
            tags: vec![],
            spec_string: "m".to_string(),
        });

        let rule: &'static dyn TransformRule = &SINGLE_VARIANT_RULE;
        assert_eq!(
            validate_rule_availability(&kb, &rule),
            RuleAvailability::Available
        );
    }

    #[test]
    fn derive_contract_deduplicates_same_name_package_variants() {
        let enabled_rules: [&'static dyn TransformRule; 1] = [&DUPLICATE_ELIMINATE_RULE];
        let contract = derive_contract(enabled_rules.as_slice());

        assert_eq!(contract.eliminated_forms.len(), 1);
        assert_eq!(contract.eliminated_forms[0].kind_label(), "command");
        assert_eq!(contract.eliminated_forms[0].name(), "shared-target");
    }
}
