//! Compiled rewrite plan: filtered rules and eliminated forms.

use std::collections::VecDeque;

use crate::config::BuildConfig;
use crate::parse::{MutationSummary, Parser};
use crate::rewrite::registry;
use crate::rewrite::rule::{
    PackageName, RewriteRule, RuleKey, RuleTarget, RuleTargetKey, RuleTargetKind,
};

#[derive(Clone, Debug)]
pub struct Plan {
    rules: Vec<&'static dyn RewriteRule>,
    eliminated_forms: Vec<RuleTargetKey>,
}

impl Plan {
    pub fn build(config: &BuildConfig, parse_ctx: &Parser) -> Result<Self, PlanBuildError> {
        let enabled = filter_rules(registry::all_rules(), config, parse_ctx)?;
        let ordered = topological_sort(enabled.as_slice())?;
        let eliminated_forms = derive_eliminated_forms(ordered.as_slice());
        Ok(Self {
            rules: ordered,
            eliminated_forms,
        })
    }

    pub fn rules(&self) -> &[&'static dyn RewriteRule] {
        self.rules.as_slice()
    }

    pub fn eliminated_forms(&self) -> &[RuleTargetKey] {
        self.eliminated_forms.as_slice()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RuleSelection {
    All,
    Only(Vec<RuleKey>),
    Except(Vec<RuleKey>),
}

impl Default for RuleSelection {
    fn default() -> Self {
        RuleSelection::All
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanBuildError {
    SelectedRuleUnavailable {
        rule: RuleKey,
        reason: RuleAvailabilityFailure,
    },
    InvalidRuleMetadata {
        rule: RuleKey,
        message: &'static str,
    },
    DependencyCycle {
        chain: Vec<RuleKey>,
    },
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

impl std::fmt::Display for PlanBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanBuildError::SelectedRuleUnavailable { rule, reason } => {
                write!(f, "selected transform rule {rule} is unavailable: {reason}")
            }
            PlanBuildError::InvalidRuleMetadata { rule, message } => {
                write!(f, "transform rule {rule} has invalid metadata: {message}")
            }
            PlanBuildError::DependencyCycle { chain } => {
                let chain = chain
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                write!(f, "transform dependency cycle detected: {chain}")
            }
        }
    }
}

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

impl std::error::Error for PlanBuildError {}

fn package_names_for_message(packages: &[PackageName]) -> Vec<&'static str> {
    packages.iter().map(|package| package.as_str()).collect()
}

fn filter_rules(
    rules: &[&'static dyn RewriteRule],
    config: &BuildConfig,
    parse_ctx: &Parser,
) -> Result<Vec<&'static dyn RewriteRule>, PlanBuildError> {
    let mut enabled = Vec::new();

    for rule in rules.iter().copied() {
        let key = rule.meta().key;
        let in_selection = match &config.selection {
            RuleSelection::All => true,
            RuleSelection::Only(keys) => keys.contains(&key),
            RuleSelection::Except(keys) => !keys.contains(&key),
        };
        let explicitly_selected =
            matches!(&config.selection, RuleSelection::Only(keys) if keys.contains(&key));

        if !in_selection {
            continue;
        }
        if !config.classes.contains(rule.meta().class) {
            continue;
        }
        if rule_touched_by_mutations(rule, parse_ctx.mutation_summary()) {
            continue;
        }

        validate_rule_metadata(rule)?;

        if let Some(reason) = package_availability_failure(rule, parse_ctx) {
            if explicitly_selected {
                return Err(PlanBuildError::SelectedRuleUnavailable { rule: key, reason });
            }
            continue;
        }

        if let Some(reason) = produced_target_availability_failure(rule, parse_ctx) {
            if explicitly_selected {
                return Err(PlanBuildError::SelectedRuleUnavailable { rule: key, reason });
            }
            continue;
        }

        enabled.push(rule);
    }

    Ok(enabled)
}

fn validate_rule_metadata(rule: &'static dyn RewriteRule) -> Result<(), PlanBuildError> {
    let meta = rule.meta();
    if meta.triggers.is_empty() {
        return Err(PlanBuildError::InvalidRuleMetadata {
            rule: meta.key,
            message: "triggers must be non-empty",
        });
    }

    let consumes = meta
        .consumes
        .eliminates
        .iter()
        .chain(meta.consumes.touches.iter())
        .copied()
        .map(RuleTarget::key)
        .collect::<Vec<_>>();
    if meta
        .triggers
        .iter()
        .copied()
        .map(RuleTarget::key)
        .any(|trigger| !consumes.contains(&trigger))
    {
        return Err(PlanBuildError::InvalidRuleMetadata {
            rule: meta.key,
            message: "triggers must be a subset of consumes",
        });
    }

    Ok(())
}

fn package_availability_failure(
    rule: &'static dyn RewriteRule,
    parse_ctx: &Parser,
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
    rule: &'static dyn RewriteRule,
    parse_ctx: &Parser,
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

fn parse_context_knows_target(parse_ctx: &Parser, target: RuleTargetKey) -> bool {
    match target.kind {
        RuleTargetKind::Command => parse_ctx.knows_command_name(target.name),
        RuleTargetKind::Environment => parse_ctx.knows_env_name(target.name),
        RuleTargetKind::Character => parse_ctx.knows_character_name(target.name),
    }
}

fn rule_touched_by_mutations(rule: &'static dyn RewriteRule, summary: &MutationSummary) -> bool {
    rule.meta()
        .consumes
        .eliminates
        .iter()
        .chain(rule.meta().consumes.touches.iter())
        .chain(rule.meta().produces.targets.iter())
        .copied()
        .map(RuleTarget::key)
        .any(|target| match target.kind {
            RuleTargetKind::Command | RuleTargetKind::Character => {
                summary.touched_commands.contains(target.name)
            }
            RuleTargetKind::Environment => summary.touched_environments.contains(target.name),
        })
}

fn derive_eliminated_forms(rules: &[&'static dyn RewriteRule]) -> Vec<RuleTargetKey> {
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

fn topological_sort(
    rules: &[&'static dyn RewriteRule],
) -> Result<Vec<&'static dyn RewriteRule>, PlanBuildError> {
    let mut incoming = vec![0usize; rules.len()];
    let mut edges = vec![Vec::<usize>::new(); rules.len()];

    for (from_index, from_rule) in rules.iter().enumerate() {
        for (to_index, to_rule) in rules.iter().enumerate() {
            if from_index == to_index {
                continue;
            }
            if rules_intersect(*from_rule, *to_rule) {
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

    Err(PlanBuildError::DependencyCycle {
        chain: detect_cycle(rules, edges.as_slice()),
    })
}

fn rules_intersect(from_rule: &'static dyn RewriteRule, to_rule: &'static dyn RewriteRule) -> bool {
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

fn detect_cycle(rules: &[&'static dyn RewriteRule], edges: &[Vec<usize>]) -> Vec<RuleKey> {
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
    rules: &[&'static dyn RewriteRule],
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
