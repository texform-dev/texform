//! Build-time transform context assembled from a parse context and profile.

use std::collections::{BTreeSet, VecDeque};

use crate::parse::ParseContext;
use crate::transform::registry::all_rules;
use crate::transform::rule::{
    RuleKey, RulePhase, RuleTarget, RuleTargetKey, RuleTargetKind, RuleTier, TransformRule,
};

pub(crate) type RuleList = Vec<&'static dyn TransformRule>;
pub(crate) type EliminatedForms = Vec<RuleTargetKey>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformProfile {
    pub name: &'static str,
    pub tiers: &'static [RuleTier],
}

impl TransformProfile {
    pub const AUTHORING: Self = Self {
        name: "authoring",
        tiers: &[RuleTier::Base],
    };

    pub const CORPUS: Self = Self {
        name: "corpus",
        tiers: &[RuleTier::Base, RuleTier::Expand],
    };

    pub const EQUIV: Self = Self {
        name: "equiv",
        tiers: &[RuleTier::Base, RuleTier::Expand, RuleTier::Deep],
    };

    pub fn builder(self) -> TransformContextBuilder {
        TransformContextBuilder::new(self)
    }

    fn includes(self, tier: RuleTier) -> bool {
        self.tiers.contains(&tier)
    }
}

#[derive(Clone)]
pub struct TransformContext {
    normalize_rules: RuleList,
    cleanup_rules: RuleList,
    eliminated_forms: EliminatedForms,
    max_iterations: usize,
}

impl TransformContext {
    pub fn normalize_rules(&self) -> &[&'static dyn TransformRule] {
        self.normalize_rules.as_slice()
    }

    pub fn cleanup_rules(&self) -> &[&'static dyn TransformRule] {
        self.cleanup_rules.as_slice()
    }

    pub fn eliminated_forms(&self) -> &[RuleTargetKey] {
        self.eliminated_forms.as_slice()
    }

    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn from_parts_for_test(
        normalize_rules: RuleList,
        cleanup_rules: RuleList,
        eliminated_forms: EliminatedForms,
        max_iterations: usize,
    ) -> Self {
        Self {
            normalize_rules,
            cleanup_rules,
            eliminated_forms,
            max_iterations,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformBuildError {
    EmptyRuleSet {
        profile: &'static str,
    },
    DependencyCycle {
        chain: Vec<RuleKey>,
    },
    CleanupBoundaryConflict {
        cleanup_rule: RuleKey,
        normalize_rule: RuleKey,
    },
}

impl std::fmt::Display for TransformBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformBuildError::EmptyRuleSet { profile } => {
                write!(f, "transform profile `{profile}` has no active rules")
            }
            TransformBuildError::DependencyCycle { chain } => {
                let chain = chain
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                write!(f, "transform dependency cycle detected: {chain}")
            }
            TransformBuildError::CleanupBoundaryConflict {
                cleanup_rule,
                normalize_rule,
            } => write!(
                f,
                "cleanup rule {cleanup_rule} produces input consumed by normalize rule {normalize_rule}"
            ),
        }
    }
}

impl std::error::Error for TransformBuildError {}

pub struct TransformContextBuilder {
    profile: TransformProfile,
    only: Option<BTreeSet<RuleKey>>,
    disabled: BTreeSet<RuleKey>,
    max_iterations: usize,
}

impl TransformContextBuilder {
    pub fn new(profile: TransformProfile) -> Self {
        Self {
            profile,
            only: None,
            disabled: BTreeSet::new(),
            max_iterations: 100,
        }
    }

    pub fn from_tiers(tiers: &'static [RuleTier]) -> Self {
        Self::new(TransformProfile {
            name: "custom",
            tiers,
        })
    }

    pub fn only(mut self, key: RuleKey) -> Self {
        self.only = Some(std::iter::once(key).collect());
        self
    }

    pub fn only_many(mut self, keys: &[RuleKey]) -> Self {
        self.only = Some(keys.iter().copied().collect());
        self
    }

    pub fn disable(mut self, key: RuleKey) -> Self {
        self.disabled.insert(key);
        self
    }

    pub fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    pub fn build_with(
        self,
        parse_ctx: &ParseContext,
    ) -> Result<TransformContext, TransformBuildError> {
        let only = self.only;
        let enabled = all_rules()
            .iter()
            .copied()
            .filter(|rule| self.profile.includes(rule.meta().tier))
            .filter(|rule| match only.as_ref() {
                Some(only_keys) => only_keys.contains(&rule.meta().key),
                None => true,
            })
            .filter(|rule| !self.disabled.contains(&rule.meta().key))
            .filter(|rule| !rule_touched_by_mutations(rule, parse_ctx.mutation_summary()))
            .collect::<Vec<_>>();

        if enabled.is_empty() {
            return Err(TransformBuildError::EmptyRuleSet {
                profile: self.profile.name,
            });
        }

        let normalize_rules = topological_sort(
            enabled
                .iter()
                .copied()
                .filter(|rule| matches!(rule.meta().phase, RulePhase::Normalize))
                .collect::<Vec<_>>()
                .as_slice(),
        )?;
        let cleanup_rules = topological_sort(
            enabled
                .iter()
                .copied()
                .filter(|rule| matches!(rule.meta().phase, RulePhase::Cleanup))
                .collect::<Vec<_>>()
                .as_slice(),
        )?;

        assert_cleanup_boundary(normalize_rules.as_slice(), cleanup_rules.as_slice())?;

        Ok(TransformContext {
            normalize_rules,
            cleanup_rules,
            eliminated_forms: derive_eliminated_forms(enabled.as_slice()),
            max_iterations: self.max_iterations,
        })
    }
}

fn derive_eliminated_forms(rules: &[&'static dyn TransformRule]) -> EliminatedForms {
    let mut forms = Vec::new();
    for rule in rules {
        for target in rule
            .meta()
            .consumes
            .eliminates
            .iter()
            .copied()
            .map(RuleTarget::key)
        {
            if !forms.contains(&target) {
                forms.push(target);
            }
        }
    }
    forms
}

fn assert_cleanup_boundary(
    normalize_rules: &[&'static dyn TransformRule],
    cleanup_rules: &[&'static dyn TransformRule],
) -> Result<(), TransformBuildError> {
    for cleanup_rule in cleanup_rules {
        for produced in cleanup_rule
            .meta()
            .produces
            .targets
            .iter()
            .copied()
            .map(RuleTarget::key)
        {
            for normalize_rule in normalize_rules {
                let consumes = normalize_rule
                    .meta()
                    .consumes
                    .eliminates
                    .iter()
                    .chain(normalize_rule.meta().consumes.requires.iter())
                    .copied()
                    .map(RuleTarget::key);
                if consumes.into_iter().any(|consumed| consumed == produced) {
                    return Err(TransformBuildError::CleanupBoundaryConflict {
                        cleanup_rule: cleanup_rule.meta().key,
                        normalize_rule: normalize_rule.meta().key,
                    });
                }
            }
        }
    }
    Ok(())
}

fn topological_sort(
    rules: &[&'static dyn TransformRule],
) -> Result<Vec<&'static dyn TransformRule>, TransformBuildError> {
    let mut incoming = vec![0usize; rules.len()];
    let mut edges = vec![Vec::<usize>::new(); rules.len()];

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

    Err(TransformBuildError::DependencyCycle {
        chain: detect_cycle(rules, edges.as_slice()),
    })
}

fn rules_intersect(
    from_rule: &&'static dyn TransformRule,
    to_rule: &&'static dyn TransformRule,
) -> bool {
    from_rule
        .meta()
        .produces
        .targets
        .iter()
        .copied()
        .map(RuleTarget::key)
        .any(|produced| {
            to_rule
                .meta()
                .consumes
                .eliminates
                .iter()
                .chain(to_rule.meta().consumes.requires.iter())
                .copied()
                .map(RuleTarget::key)
                .any(|consumed| consumed == produced)
        })
}

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

fn rule_touched_by_mutations(
    rule: &&'static dyn TransformRule,
    summary: &crate::parse::MutationSummary,
) -> bool {
    rule.meta()
        .consumes
        .eliminates
        .iter()
        .chain(rule.meta().consumes.requires.iter())
        .chain(rule.meta().produces.targets.iter())
        .copied()
        .map(RuleTarget::key)
        .any(|target| match target.kind {
            RuleTargetKind::Command => summary.touched_commands.contains(target.name),
            RuleTargetKind::Environment => summary.touched_environments.contains(target.name),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::NodeId;
    use crate::transform::engine::TransformError;
    use crate::transform::rule::{
        RuleConsumes, RuleEffect, RuleMeta, RulePackage, RuleProduces, RuleSafety, RuleTier,
        TransformRule,
    };
    use crate::transform::rule_context::RuleContext;
    use texform_specs::argspec;
    use texform_specs::specs::{AllowedMode, BuiltinCommandRecord, CommandKind};

    struct MockRule {
        meta: &'static RuleMeta,
    }

    impl TransformRule for MockRule {
        fn meta(&self) -> &'static RuleMeta {
            self.meta
        }

        fn apply(
            &self,
            _cx: &mut RuleContext<'_>,
            _node_id: NodeId,
        ) -> Result<RuleEffect, TransformError> {
            Ok(RuleEffect::Skipped)
        }
    }

    static COMMAND_A: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "rule-a-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!(""),
        tags: &[],
    };

    static COMMAND_B: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "rule-b-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!(""),
        tags: &[],
    };

    static COMMAND_C: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "rule-c-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!(""),
        tags: &[],
    };

    static SHARED_VARIANT_BASE: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "shared-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m"),
        tags: &[],
    };

    static SHARED_VARIANT_AMS: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "shared-target",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Both,
        argspec: argspec!("m"),
        tags: &[],
    };

    static SHARED_VARIANT_TARGETS: [RuleTarget; 2] = [
        RuleTarget::Command(&SHARED_VARIANT_BASE),
        RuleTarget::Command(&SHARED_VARIANT_AMS),
    ];

    static RULE_A_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: RulePackage::Physics,
            name: "a",
        },
        tier: RuleTier::Base,
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
            package: RulePackage::Physics,
            name: "b",
        },
        tier: RuleTier::Base,
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
            package: RulePackage::Physics,
            name: "c",
        },
        tier: RuleTier::Base,
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

    static CLEANUP_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: RulePackage::Base,
            name: "cleanup",
        },
        tier: RuleTier::Base,
        summary: "mock cleanup rule",
        phase: RulePhase::Cleanup,
        safety: RuleSafety::Lossless,
        triggers: &[],
        consumes: RuleConsumes {
            eliminates: &[],
            requires: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_A)],
        },
    };

    static DUPLICATE_ELIMINATE_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: RulePackage::Physics,
            name: "duplicate-eliminate",
        },
        tier: RuleTier::Base,
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

    static RULE_A: MockRule = MockRule { meta: &RULE_A_META };
    static RULE_B: MockRule = MockRule { meta: &RULE_B_META };
    static RULE_C: MockRule = MockRule { meta: &RULE_C_META };
    static CLEANUP_RULE: MockRule = MockRule {
        meta: &CLEANUP_RULE_META,
    };
    static DUPLICATE_ELIMINATE_RULE: MockRule = MockRule {
        meta: &DUPLICATE_ELIMINATE_RULE_META,
    };

    #[test]
    fn topological_sort_reports_concrete_cycle_chain() {
        let rules: [&'static dyn TransformRule; 3] = [&RULE_A, &RULE_B, &RULE_C];

        let error = match topological_sort(rules.as_slice()) {
            Ok(_) => panic!("expected a cycle"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            TransformBuildError::DependencyCycle {
                chain: vec![
                    RULE_A_META.key,
                    RULE_B_META.key,
                    RULE_C_META.key,
                    RULE_A_META.key,
                ],
            }
        );
    }

    #[test]
    fn cleanup_boundary_reports_conflicting_rules() {
        let normalize_rules: [&'static dyn TransformRule; 1] = [&RULE_B];
        let cleanup_rules: [&'static dyn TransformRule; 1] = [&CLEANUP_RULE];

        let error = assert_cleanup_boundary(normalize_rules.as_slice(), cleanup_rules.as_slice())
            .expect_err("cleanup boundary should reject normalize input");

        assert_eq!(
            error,
            TransformBuildError::CleanupBoundaryConflict {
                cleanup_rule: CLEANUP_RULE_META.key,
                normalize_rule: RULE_B_META.key,
            }
        );
    }

    #[test]
    fn derive_eliminated_forms_deduplicates_same_name_package_variants() {
        let enabled_rules: [&'static dyn TransformRule; 1] = [&DUPLICATE_ELIMINATE_RULE];
        let eliminated_forms = derive_eliminated_forms(enabled_rules.as_slice());

        assert_eq!(eliminated_forms.len(), 1);
        assert_eq!(
            eliminated_forms[0],
            RuleTargetKey {
                kind: RuleTargetKind::Command,
                name: "shared-target",
            }
        );
    }
}
