//! Build-time transform context assembled from a parse context and profile.

use std::collections::{BTreeSet, VecDeque};

use crate::parse::ParseContext;
use crate::transform::registry::all_rules;
use crate::transform::rule::{
    RuleKey, RulePhase, RuleTarget, RuleTargetKey, RuleTargetKind, RuleTier, TransformRule,
};
use texform_specs::builtin::PackageName;

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
pub enum RuleAvailabilityFailure {
    DisabledByPackage {
        required: Vec<PackageName>,
        active: Vec<PackageName>,
    },
    ProducedTargetUnavailable {
        target: RuleTargetKey,
        active: Vec<PackageName>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformBuildError {
    SelectedRuleUnavailable {
        rule: RuleKey,
        reason: RuleAvailabilityFailure,
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
            TransformBuildError::SelectedRuleUnavailable { rule, reason } => {
                write!(f, "selected transform rule {rule} is unavailable: {reason}")
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

impl std::fmt::Display for RuleAvailabilityFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleAvailabilityFailure::DisabledByPackage { required, active } => write!(
                f,
                "enabled_by_packages {:?} does not intersect active packages {:?}",
                package_names_for_message(required.as_slice()),
                package_names_for_message(active.as_slice())
            ),
            RuleAvailabilityFailure::ProducedTargetUnavailable { target, active } => write!(
                f,
                "produced {} `{}` is unavailable in active packages {:?}",
                target.kind_label(),
                target.name,
                package_names_for_message(active.as_slice())
            ),
        }
    }
}

fn package_names_for_message(packages: &[PackageName]) -> Vec<&'static str> {
    packages.iter().map(|package| package.as_str()).collect()
}

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
        let enabled = filter_rules(
            all_rules(),
            self.profile,
            only.as_ref(),
            &self.disabled,
            parse_ctx,
        )?;

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

fn filter_rules(
    rules: &[&'static dyn TransformRule],
    profile: TransformProfile,
    only: Option<&BTreeSet<RuleKey>>,
    disabled: &BTreeSet<RuleKey>,
    parse_ctx: &ParseContext,
) -> Result<RuleList, TransformBuildError> {
    let mut enabled = Vec::new();

    for rule in rules.iter().copied() {
        let key = rule.meta().key;
        let explicitly_selected = only.is_some_and(|only_keys| only_keys.contains(&key));

        if !profile.includes(rule.meta().tier) {
            continue;
        }
        if only.is_some_and(|only_keys| !only_keys.contains(&key)) {
            continue;
        }
        if disabled.contains(&key) {
            continue;
        }
        if rule_touched_by_mutations(rule, parse_ctx.mutation_summary()) {
            continue;
        }

        if let Some(reason) = package_availability_failure(rule, parse_ctx) {
            if explicitly_selected {
                return Err(TransformBuildError::SelectedRuleUnavailable { rule: key, reason });
            }
            continue;
        }

        if let Some(reason) = produced_target_availability_failure(rule, parse_ctx) {
            if explicitly_selected {
                return Err(TransformBuildError::SelectedRuleUnavailable { rule: key, reason });
            }
            continue;
        }

        enabled.push(rule);
    }

    Ok(enabled)
}

fn package_availability_failure(
    rule: &'static dyn TransformRule,
    parse_ctx: &ParseContext,
) -> Option<RuleAvailabilityFailure> {
    let active = parse_ctx.enabled_packages();
    if rule
        .meta()
        .enabled_by_packages
        .iter()
        .any(|package| active.contains(package))
    {
        return None;
    }

    Some(RuleAvailabilityFailure::DisabledByPackage {
        required: rule.meta().enabled_by_packages.to_vec(),
        active: active.to_vec(),
    })
}

fn produced_target_availability_failure(
    rule: &'static dyn TransformRule,
    parse_ctx: &ParseContext,
) -> Option<RuleAvailabilityFailure> {
    rule.meta()
        .produces
        .targets
        .iter()
        .copied()
        .map(RuleTarget::key)
        .find(|target| !parse_context_knows_target(parse_ctx, *target))
        .map(
            |target| RuleAvailabilityFailure::ProducedTargetUnavailable {
                target,
                active: parse_ctx.enabled_packages().to_vec(),
            },
        )
}

fn parse_context_knows_target(parse_ctx: &ParseContext, target: RuleTargetKey) -> bool {
    match target.kind {
        RuleTargetKind::Command => parse_ctx.knows_command_name(target.name),
        RuleTargetKind::Environment => parse_ctx.knows_env_name(target.name),
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
                    .chain(normalize_rule.meta().consumes.touches.iter())
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
                .chain(to_rule.meta().consumes.touches.iter())
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
    rule: &'static dyn TransformRule,
    summary: &crate::parse::MutationSummary,
) -> bool {
    rule.meta()
        .consumes
        .eliminates
        .iter()
        .chain(rule.meta().consumes.touches.iter())
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
        RuleConsumes, RuleEffect, RuleMeta, RuleProduces, RuleSafety, RuleTier, TransformRule,
    };
    use crate::transform::rule_context::RuleContext;
    use texform_specs::argspec;
    use texform_specs::builtin::{MANAGED_PACKAGE_IMPORT_ORDER, PackageName};
    use texform_specs::specs::{
        AllowedMode, BuiltinCommandRecord, BuiltinEnvironmentRecord, CommandKind, ContentMode,
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

    static PRODUCED_ENV: BuiltinEnvironmentRecord = BuiltinEnvironmentRecord {
        name: "matrix",
        allowed_mode: AllowedMode::Math,
        argspec: argspec!(""),
        body_mode: ContentMode::Math,
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
            package: PackageName::Physics,
            name: "a",
        },
        enabled_by_packages: &[PackageName::Physics],
        tier: RuleTier::Base,
        summary: "mock rule a",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_C)],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_A)],
        },
    };

    static RULE_B_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: PackageName::Physics,
            name: "b",
        },
        enabled_by_packages: &[PackageName::Physics],
        tier: RuleTier::Base,
        summary: "mock rule b",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_A)],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_B)],
        },
    };

    static RULE_C_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: PackageName::Physics,
            name: "c",
        },
        enabled_by_packages: &[PackageName::Physics],
        tier: RuleTier::Base,
        summary: "mock rule c",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_B)],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_C)],
        },
    };

    static CLEANUP_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: PackageName::Base,
            name: "cleanup",
        },
        enabled_by_packages: &[PackageName::Base],
        tier: RuleTier::Base,
        summary: "mock cleanup rule",
        phase: RulePhase::Cleanup,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Command(&COMMAND_A)],
        },
    };

    static DUPLICATE_ELIMINATE_RULE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: PackageName::Physics,
            name: "duplicate-eliminate",
        },
        enabled_by_packages: &[PackageName::Physics],
        tier: RuleTier::Base,
        summary: "mock rule with duplicate eliminate variants",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &SHARED_VARIANT_TARGETS,
            touches: &[],
        },
        produces: RuleProduces { targets: &[] },
    };

    static PACKAGE_BASE_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: PackageName::Base,
            name: "base-only",
        },
        enabled_by_packages: &[PackageName::Base],
        tier: RuleTier::Base,
        summary: "mock base package rule",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_A)],
            touches: &[],
        },
        produces: RuleProduces { targets: &[] },
    };

    static PACKAGE_PHYSICS_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: PackageName::Physics,
            name: "physics-only",
        },
        enabled_by_packages: &[PackageName::Physics],
        tier: RuleTier::Base,
        summary: "mock physics package rule",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_B)],
            touches: &[],
        },
        produces: RuleProduces { targets: &[] },
    };

    static PRODUCES_AMS_ENV_META: RuleMeta = RuleMeta {
        key: RuleKey {
            package: PackageName::Base,
            name: "produces-ams-env",
        },
        enabled_by_packages: &[PackageName::Base],
        tier: RuleTier::Base,
        summary: "mock rule producing matrix environment",
        phase: RulePhase::Normalize,
        safety: RuleSafety::Lossless,
        consumes: RuleConsumes {
            eliminates: &[RuleTarget::Command(&COMMAND_A)],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&PRODUCED_ENV)],
        },
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
    static PACKAGE_BASE_RULE: MockRule = MockRule {
        meta: &PACKAGE_BASE_META,
    };
    static PACKAGE_PHYSICS_RULE: MockRule = MockRule {
        meta: &PACKAGE_PHYSICS_META,
    };
    static PRODUCES_AMS_ENV_RULE: MockRule = MockRule {
        meta: &PRODUCES_AMS_ENV_META,
    };

    fn filter_rules_for_test(
        rules: &[&'static dyn TransformRule],
        parse_ctx: &ParseContext,
    ) -> Result<Vec<&'static dyn TransformRule>, TransformBuildError> {
        filter_rules(
            rules,
            TransformProfile::AUTHORING,
            None,
            &BTreeSet::new(),
            parse_ctx,
        )
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

    #[test]
    fn filter_rules_keeps_only_rules_enabled_by_parse_context_packages() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let rules: [&'static dyn TransformRule; 2] = [&PACKAGE_BASE_RULE, &PACKAGE_PHYSICS_RULE];

        let enabled =
            filter_rules_for_test(rules.as_slice(), &parse_ctx).expect("filter should pass");

        assert_eq!(
            enabled
                .iter()
                .map(|rule| rule.meta().key)
                .collect::<Vec<_>>(),
            vec![PACKAGE_BASE_META.key]
        );
    }

    #[test]
    fn build_with_all_rules_filtered_by_packages_returns_empty_context() {
        let parse_ctx = ParseContext::empty();
        let context = TransformProfile::AUTHORING
            .builder()
            .build_with(&parse_ctx)
            .expect("empty package context should produce a no-op transform context");

        assert!(context.normalize_rules().is_empty());
        assert!(context.cleanup_rules().is_empty());
        assert!(context.eliminated_forms().is_empty());
    }

    #[test]
    fn only_rule_reports_error_when_required_package_is_disabled() {
        let parse_ctx = ParseContext::from_packages(&["base"]);

        let error = match filter_rules(
            &[&PACKAGE_PHYSICS_RULE],
            TransformProfile::AUTHORING,
            Some(&BTreeSet::from([PACKAGE_PHYSICS_META.key])),
            &BTreeSet::new(),
            &parse_ctx,
        ) {
            Ok(_) => panic!("only physics rule should be unavailable in base-only context"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            TransformBuildError::SelectedRuleUnavailable {
                rule: PACKAGE_PHYSICS_META.key,
                reason: RuleAvailabilityFailure::DisabledByPackage {
                    required: vec![PackageName::Physics],
                    active: vec![PackageName::Base],
                },
            }
        );
    }

    #[test]
    fn filter_rules_drops_rule_when_produced_target_is_unavailable() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let rules: [&'static dyn TransformRule; 1] = [&PRODUCES_AMS_ENV_RULE];

        let enabled =
            filter_rules_for_test(rules.as_slice(), &parse_ctx).expect("filter should pass");

        assert!(enabled.is_empty());
    }

    #[test]
    fn only_rule_reports_error_when_produced_target_is_unavailable() {
        let parse_ctx = ParseContext::from_packages(&["base"]);

        let error = match filter_rules(
            &[&PRODUCES_AMS_ENV_RULE],
            TransformProfile::AUTHORING,
            Some(&BTreeSet::from([PRODUCES_AMS_ENV_META.key])),
            &BTreeSet::new(),
            &parse_ctx,
        ) {
            Ok(_) => panic!("selected rule should be unavailable because env:matrix is not active"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            TransformBuildError::SelectedRuleUnavailable {
                rule: PRODUCES_AMS_ENV_META.key,
                reason: RuleAvailabilityFailure::ProducedTargetUnavailable {
                    target: RuleTargetKey {
                        kind: RuleTargetKind::Environment,
                        name: "matrix",
                    },
                    active: vec![PackageName::Base],
                },
            }
        );
    }

    #[test]
    fn rule_metadata_enabled_packages_match_consumed_target_signatures() {
        for rule in all_rules() {
            let inferred = inferred_enabled_packages(rule.meta());
            assert_eq!(
                inferred,
                rule.meta().enabled_by_packages,
                "rule {} enabled_by_packages should match packages inferred from eliminates first, touches fallback",
                rule.meta().key
            );
        }
    }

    #[test]
    fn rule_key_package_is_first_enabled_package_by_import_order() {
        for rule in all_rules() {
            let mut enabled = rule.meta().enabled_by_packages.to_vec();
            enabled.sort_by_key(|package| package.import_order());
            assert_eq!(
                Some(rule.meta().key.package),
                enabled.first().copied(),
                "rule {} key package should be the first enabled package by import order",
                rule.meta().key
            );
        }
    }

    #[test]
    fn rule_metadata_targets_do_not_repeat_kind_name_variants() {
        for rule in all_rules() {
            assert_unique_target_keys(
                rule.meta().consumes.eliminates,
                rule.meta().key,
                "eliminates",
            );
            assert_unique_target_keys(rule.meta().consumes.touches, rule.meta().key, "touches");
            assert_unique_target_keys(rule.meta().produces.targets, rule.meta().key, "produces");
        }
    }

    fn inferred_enabled_packages(meta: &RuleMeta) -> Vec<PackageName> {
        let source_targets = if !meta.consumes.eliminates.is_empty() {
            meta.consumes.eliminates
        } else {
            meta.consumes.touches
        };

        let mut packages = Vec::new();
        for target in source_targets {
            for package in packages_for_target_signature(*target) {
                if !packages.contains(&package) {
                    packages.push(package);
                }
            }
        }
        packages.sort_by_key(|package| package.import_order());
        packages
    }

    fn packages_for_target_signature(target: RuleTarget) -> Vec<PackageName> {
        MANAGED_PACKAGE_IMPORT_ORDER
            .iter()
            .copied()
            .filter(|package| package_contains_matching_target(*package, target))
            .collect()
    }

    fn package_contains_matching_target(package: PackageName, target: RuleTarget) -> bool {
        let builtin = package.package();
        match target {
            RuleTarget::Command(record) => builtin.commands.iter().any(|candidate| {
                candidate.name == record.name
                    && candidate.kind == record.kind
                    && candidate.argspec.source == record.argspec.source
            }),
            RuleTarget::Environment(record) => builtin.environments.iter().any(|candidate| {
                candidate.name == record.name
                    && candidate.argspec.source == record.argspec.source
                    && candidate.body_mode == record.body_mode
            }),
        }
    }

    fn assert_unique_target_keys(targets: &[RuleTarget], key: RuleKey, field: &str) {
        let mut seen = Vec::new();
        for target in targets {
            let target_key = target.key();
            assert!(
                !seen.contains(&target_key),
                "rule {key} repeats {} target {} `{}`; keep only the first builtin record by import order",
                field,
                target_key.kind_label(),
                target_key.name
            );
            seen.push(target_key);
        }
    }
}
