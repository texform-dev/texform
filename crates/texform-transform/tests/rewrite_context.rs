use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContentMode, ParseContext, ParseContextBuilder,
};
use texform_core::serialize::serialize;
use texform_knowledge::builtin::MANAGED_PACKAGE_IMPORT_ORDER;
use texform_transform::rewrite::{
    RuleAvailabilityFailure, RuleTargetKey, RuleTargetKind, all_rules,
};
use texform_transform::round_trip;
use texform_transform::{
    BuildConfig, PackageName, PlanBuildError, Profile, RuleKey, RuleLevel, RuleLevelSet, RuleMeta,
    RuleTarget, TransformBuildError, TransformContext,
};

fn active_rule_keys(context: &TransformContext) -> Vec<RuleKey> {
    context
        .rewrite_plan()
        .rules()
        .iter()
        .map(|rule| rule.meta().key)
        .collect::<Vec<_>>()
}

#[test]
fn only_many_keeps_the_requested_rules() {
    let parse_ctx = ParseContextBuilder::default()
        .packages(&["physics"])
        .build()
        .expect("parse context should build");

    let requested = all_rules()
        .iter()
        .map(|rule| rule.meta())
        .filter(|meta| {
            meta.level == RuleLevel::Authoring
                && meta.enabled_by_packages.contains(&PackageName::Physics)
        })
        .take(2)
        .map(|meta| meta.key)
        .collect::<Vec<_>>();
    assert_eq!(requested.len(), 2);

    let config = BuildConfig::profile(Profile::Authoring).only_rules_for_tests(requested.clone());
    let transform_ctx = TransformContext::from_build_config(config, &parse_ctx)
        .expect("transform context should build");

    let active_keys = active_rule_keys(&transform_ctx);

    assert_eq!(active_keys.len(), 2);
    assert!(requested.iter().all(|key| active_keys.contains(key)));
}

#[test]
fn build_with_disables_rules_touching_mutated_command_names() {
    let parse_ctx = ParseContextBuilder::default()
        .packages(&["physics"])
        .item(CommandItem::new(
            "quantity",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        ))
        .build()
        .expect("parse context should build");

    let transform_ctx =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");

    let active = transform_ctx
        .rewrite_plan()
        .rules()
        .iter()
        .map(|rule| rule.meta().key.to_string())
        .collect::<Vec<_>>();

    assert!(!active.iter().any(|key| key == "physics/quantity-to-qty"));
}

#[test]
fn disabling_all_rules_builds_empty_transform_context() {
    let parse_ctx = ParseContextBuilder::default()
        .packages(&["physics"])
        .build()
        .expect("parse context should build");

    let mut config = BuildConfig::profile(Profile::Authoring);
    for rule in all_rules() {
        config = config.disable_rule(rule.meta().key);
    }

    let transform_ctx = TransformContext::from_build_config(config, &parse_ctx)
        .expect("empty transform context should be a valid no-op");

    assert!(transform_ctx.rewrite_plan().rules().is_empty());
}

#[test]
fn only_does_not_bypass_profile_class_filter_and_can_return_empty_context() {
    let parse_ctx = ParseContextBuilder::default()
        .packages(&["physics"])
        .build()
        .expect("parse context should build");

    let non_equiv_rule = all_rules()
        .iter()
        .map(|rule| rule.meta())
        .find(|meta| meta.level != RuleLevel::Equiv)
        .expect("registry should contain a non-equiv rule")
        .key;
    let config = BuildConfig::profile(Profile::Authoring)
        .rule_levels(RuleLevelSet::EQUIV)
        .only_rule_for_tests(non_equiv_rule);
    let transform_ctx = TransformContext::from_build_config(config, &parse_ctx)
        .expect("empty transform context should be a valid no-op");

    assert!(transform_ctx.rewrite_plan().rules().is_empty());
}

#[test]
fn build_with_all_rules_filtered_by_packages_returns_empty_context() {
    let parse_ctx = ParseContext::empty();
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("empty package context should produce a no-op transform context");

    let plan = context.rewrite_plan();
    assert!(plan.rules().is_empty());
    assert!(plan.eliminated_forms().is_empty());
}

#[test]
fn build_with_keeps_only_rules_enabled_by_parse_context_packages() {
    let parse_ctx = ParseContext::from_packages(&["base"]);

    let transform_ctx =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");

    let active = transform_ctx
        .rewrite_plan()
        .rules()
        .iter()
        .map(|rule| rule.meta())
        .collect::<Vec<_>>();

    assert!(!active.is_empty());
    assert!(
        active
            .iter()
            .all(|meta| meta.enabled_by_packages.contains(&PackageName::Base))
    );
    assert!(
        !active
            .iter()
            .any(|meta| meta.key.to_string() == "physics/quantity-to-qty")
    );
}

#[test]
fn only_rule_reports_error_when_required_package_is_disabled() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let physics_rule = all_rules()
        .iter()
        .find(|rule| rule.meta().key.to_string() == "physics/quantity-to-qty")
        .expect("physics quantity rule should be registered");

    let config =
        BuildConfig::profile(Profile::Authoring).only_rule_for_tests(physics_rule.meta().key);
    let error = match TransformContext::from_build_config(config, &parse_ctx) {
        Ok(_) => panic!("only physics rule should be unavailable in base-only context"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        TransformBuildError::Rewrite(PlanBuildError::SelectedRuleUnavailable {
            rule: physics_rule.meta().key,
            reason: RuleAvailabilityFailure::DisabledByPackage {
                required: vec![PackageName::Physics],
                active: vec![PackageName::Base],
            },
        })
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

struct BoundedConfig {
    packages: Vec<PackageName>,
    parse_ctx: ParseContext,
    equiv_ctx: TransformContext,
}

fn discover_order_sensitive_pairs() -> Vec<(PackageName, PackageName)> {
    let optional: Vec<PackageName> = MANAGED_PACKAGE_IMPORT_ORDER
        .iter()
        .copied()
        .filter(|&package| package != PackageName::Base)
        .collect();
    let mut pairs = Vec::new();
    for i in 0..optional.len() {
        for j in (i + 1)..optional.len() {
            let p = optional[i];
            let q = optional[j];
            let pq = ParseContext::from_packages(&["base", p.as_str(), q.as_str()]);
            let qp = ParseContext::from_packages(&["base", q.as_str(), p.as_str()]);
            if catalogs_differ(&pq, &qp) {
                pairs.push((p, q));
            }
        }
    }
    pairs
}

fn catalogs_differ(left: &ParseContext, right: &ParseContext) -> bool {
    for mode in [ContentMode::Math, ContentMode::Text] {
        let left_cat = left.catalog(mode);
        let right_cat = right.catalog(mode);

        let cmd_names: BTreeSet<&str> = left_cat
            .commands()
            .into_iter()
            .chain(right_cat.commands())
            .map(|record| record.name)
            .collect();
        for name in cmd_names {
            let same = match (
                left_cat.lookup_command(name),
                right_cat.lookup_command(name),
                left_cat.lookup_active_character(name),
                right_cat.lookup_active_character(name),
            ) {
                (Some(l), Some(r), Some(lc), Some(rc)) => {
                    l.kind == r.kind
                        && l.argspec.source == r.argspec.source
                        && lc.unicode_value == rc.unicode_value
                }
                (Some(l), Some(r), None, None) => {
                    l.kind == r.kind && l.argspec.source == r.argspec.source
                }
                (None, None, None, None) => true,
                _ => false,
            };
            if !same {
                return true;
            }
        }

        let env_names: BTreeSet<&str> = left_cat
            .environments()
            .into_iter()
            .chain(right_cat.environments())
            .map(|record| record.name)
            .collect();
        for name in env_names {
            let same = match (left_cat.lookup_env(name), right_cat.lookup_env(name)) {
                (Some(l), Some(r)) => {
                    l.argspec.source == r.argspec.source && l.body_mode == r.body_mode
                }
                (None, None) => true,
                _ => false,
            };
            if !same {
                return true;
            }
        }
    }
    false
}

fn discover_relevant_packages(
    order_sensitive_pairs: &[(PackageName, PackageName)],
) -> (Vec<PackageName>, Vec<PackageName>) {
    let mut relevant = BTreeSet::new();

    // Packages participating in order-sensitive pairs are always relevant.
    for &(p, q) in order_sensitive_pairs {
        relevant.insert(p);
        relevant.insert(q);
    }

    // Collect every command/char and environment name referenced in any rule's
    // consumes or produces sets, plus every package listed in enabled_by_packages.
    let mut rule_cmd_or_char_names = BTreeSet::new();
    let mut rule_env_names = BTreeSet::new();
    for rule in all_rules() {
        let meta = rule.meta();
        for &package in meta.enabled_by_packages {
            relevant.insert(package);
        }
        for target in meta
            .consumes
            .eliminates
            .iter()
            .chain(meta.consumes.touches.iter())
            .chain(meta.produces.targets.iter())
        {
            match target {
                RuleTarget::Command(record) => {
                    rule_cmd_or_char_names.insert(record.name);
                }
                RuleTarget::Character(record) => {
                    rule_cmd_or_char_names.insert(record.name);
                }
                RuleTarget::Environment(record) => {
                    rule_env_names.insert(record.name);
                }
            }
        }
    }

    // Any package defining a rule-referenced name can affect target availability
    // or active-definition matching.
    for &package in MANAGED_PACKAGE_IMPORT_ORDER {
        let builtin = package.package();
        let defines_referenced = builtin
            .commands
            .iter()
            .any(|cmd| rule_cmd_or_char_names.contains(cmd.name))
            || builtin
                .characters
                .iter()
                .any(|ch| rule_cmd_or_char_names.contains(ch.name))
            || builtin
                .environments
                .iter()
                .any(|env| rule_env_names.contains(env.name));
        if defines_referenced {
            relevant.insert(package);
        }
    }

    let relevant_ordered: Vec<PackageName> = MANAGED_PACKAGE_IMPORT_ORDER
        .iter()
        .copied()
        .filter(|package| relevant.contains(package))
        .collect();
    let independent_ordered: Vec<PackageName> = MANAGED_PACKAGE_IMPORT_ORDER
        .iter()
        .copied()
        .filter(|package| !relevant.contains(package))
        .collect();
    (relevant_ordered, independent_ordered)
}

fn expand_orderings_for_subset(
    subset: &[PackageName],
    order_sensitive_pairs: &[(PackageName, PackageName)],
) -> Vec<Vec<PackageName>> {
    let has_base = subset.contains(&PackageName::Base);
    let optional: Vec<PackageName> = subset
        .iter()
        .copied()
        .filter(|&package| package != PackageName::Base)
        .collect();
    let active_pairs: Vec<(PackageName, PackageName)> = order_sensitive_pairs
        .iter()
        .copied()
        .filter(|(p, q)| optional.contains(p) && optional.contains(q))
        .collect();

    if active_pairs.is_empty() {
        let mut ordered = Vec::with_capacity(subset.len());
        if has_base {
            ordered.push(PackageName::Base);
        }
        ordered.extend(optional);
        return vec![ordered];
    }

    let mut seen = BTreeSet::new();
    let mut results = Vec::new();
    let pair_count = active_pairs.len();
    for dir_mask in 0..(1usize << pair_count) {
        // Build precedence edges from the chosen direction of each active pair.
        let mut constraints: Vec<(PackageName, PackageName)> = Vec::with_capacity(pair_count);
        for (idx, &(p, q)) in active_pairs.iter().enumerate() {
            if dir_mask & (1 << idx) == 0 {
                constraints.push((p, q));
            } else {
                constraints.push((q, p));
            }
        }

        // Deterministic topological sort (Kahn's algorithm breaking ties by canonical import_order).
        let mut remaining: Vec<PackageName> = optional.clone();
        let mut ordered_optional = Vec::with_capacity(optional.len());
        let mut valid = true;
        while !remaining.is_empty() {
            let next = remaining
                .iter()
                .copied()
                .filter(|candidate| {
                    !constraints
                        .iter()
                        .any(|&(before, after)| after == *candidate && remaining.contains(&before))
                })
                .min_by_key(|package| package.import_order());
            match next {
                Some(pkg) => {
                    ordered_optional.push(pkg);
                    remaining.retain(|&candidate| candidate != pkg);
                }
                None => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid {
            continue;
        }

        let mut ordered = Vec::with_capacity(subset.len());
        if has_base {
            ordered.push(PackageName::Base);
        }
        ordered.extend(ordered_optional);
        if seen.insert(ordered.clone()) {
            results.push(ordered);
        }
    }
    results
}

fn bounded_configurations() -> &'static [BoundedConfig] {
    static CONFIGS: OnceLock<Vec<BoundedConfig>> = OnceLock::new();
    CONFIGS
        .get_or_init(|| {
            let order_sensitive_pairs = discover_order_sensitive_pairs();
            let (relevant, independent) = discover_relevant_packages(&order_sensitive_pairs);

            let mut configs = Vec::new();
            let include_modes: &[bool] = if independent.is_empty() {
                &[false]
            } else {
                &[false, true]
            };

            for mask in 0..(1usize << relevant.len()) {
                let base_subset: Vec<PackageName> = relevant
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, &package)| package)
                    .collect();

                for &with_independent in include_modes {
                    let mut subset = base_subset.clone();
                    if with_independent {
                        subset.extend_from_slice(&independent);
                        subset.sort_by_key(|package| package.import_order());
                    }

                    for packages in expand_orderings_for_subset(&subset, &order_sensitive_pairs) {
                        let package_strs: Vec<&str> =
                            packages.iter().map(|package| package.as_str()).collect();
                        let parse_ctx = ParseContextBuilder::default()
                            .packages(&package_strs)
                            .build()
                            .unwrap_or_else(|error| panic!("packages {package_strs:?}: {error}"));
                        let equiv_ctx = TransformContext::from_build_config(
                            BuildConfig::profile(Profile::Equiv),
                            &parse_ctx,
                        )
                        .unwrap_or_else(|error| panic!("packages {package_strs:?}: {error}"));
                        configs.push(BoundedConfig {
                            packages,
                            parse_ctx,
                            equiv_ctx,
                        });
                    }
                }
            }

            configs
        })
        .as_slice()
}

fn target_mismatches_active_definition(parse_ctx: &ParseContext, target: RuleTarget) -> bool {
    let allowed_mode = match target {
        RuleTarget::Command(record) => record.allowed_mode,
        RuleTarget::Environment(record) => record.allowed_mode,
        RuleTarget::Character(record) => record.allowed_mode,
    };

    for mode in [ContentMode::Math, ContentMode::Text] {
        if !allowed_mode.allows(mode) {
            continue;
        }
        let matches = match target {
            RuleTarget::Command(record) => match parse_ctx.lookup_command(record.name, mode) {
                Some(active) => {
                    active.kind == record.kind && active.argspec.source == record.argspec.source
                }
                None => true,
            },
            RuleTarget::Environment(record) => match parse_ctx.lookup_env(record.name, mode) {
                Some(active) => {
                    active.argspec.source == record.argspec.source
                        && active.body_mode == record.body_mode
                }
                None => true,
            },
            RuleTarget::Character(record) => match parse_ctx.lookup_command(record.name, mode) {
                Some(_) => parse_ctx
                    .lookup_active_character(record.name, mode)
                    .is_some_and(|active| active.unicode_value == record.unicode_value),
                None => true,
            },
        };
        if !matches {
            return true;
        }
    }
    false
}

#[test]
fn every_package_subset_has_unique_eliminate_owners() {
    // Equiv includes every rule level; bounded projection covers all relevant
    // package subsets, all relative orderings of order-sensitive pairs, and
    // independent packages toggled together.
    for config in bounded_configurations() {
        let mut owners = HashMap::new();
        for rule in config.equiv_ctx.rewrite_plan().rules() {
            let meta = rule.meta();
            for target in meta.consumes.eliminates {
                let target = target.key();
                if let Some(previous) = owners.insert(target, meta.key) {
                    panic!(
                        "packages {:?}: {} `{}` has eliminate owners {previous} and {}",
                        config.packages,
                        target.kind_label(),
                        target.name,
                        meta.key
                    );
                }
            }
        }
    }
}

#[test]
fn consumed_definition_check_excludes_only_overridden_physics_braket_rules() {
    let physics_rules = [
        ("physics/physics-bra-expand", "bra"),
        ("physics/physics-ket-expand", "ket"),
        ("physics/physics-braket-expand", "braket"),
    ];

    // braket before physics: physics definitions are active, so all three rules are enabled.
    let braket_then_physics = ParseContext::from_packages(&["base", "ams", "braket", "physics"]);
    let enabled_ctx = TransformContext::from_build_config(
        BuildConfig::profile(Profile::Faithful),
        &braket_then_physics,
    )
    .expect("faithful context should build");
    let enabled_keys: Vec<String> = enabled_ctx
        .rewrite_plan()
        .rules()
        .iter()
        .map(|rule| rule.meta().key.to_string())
        .collect();
    for (rule_name, _) in physics_rules {
        assert!(
            enabled_keys.iter().any(|key| key == rule_name),
            "{rule_name} should be enabled when physics follows braket"
        );
    }

    // physics before braket: braket overrides bra/ket/braket, so the three rules are excluded
    // and their eliminated targets do not enter eliminated_forms.
    let physics_then_braket = ParseContext::from_packages(&["base", "ams", "physics", "braket"]);
    let excluded_ctx = TransformContext::from_build_config(
        BuildConfig::profile(Profile::Faithful),
        &physics_then_braket,
    )
    .expect("faithful context should build");
    let excluded_keys: Vec<String> = excluded_ctx
        .rewrite_plan()
        .rules()
        .iter()
        .map(|rule| rule.meta().key.to_string())
        .collect();
    let eliminated = excluded_ctx.rewrite_plan().eliminated_forms();

    for (rule_name, cmd_name) in physics_rules {
        assert!(
            !excluded_keys.iter().any(|key| key == rule_name),
            "{rule_name} should be excluded when braket follows physics"
        );
        assert!(
            !eliminated.contains(&RuleTargetKey {
                kind: RuleTargetKind::Command,
                name: cmd_name,
            }),
            "cmd:{cmd_name} must not remain in eliminated_forms"
        );

        let rule_key = all_rules()
            .iter()
            .find(|rule| rule.meta().key.to_string() == rule_name)
            .expect("rule should exist")
            .meta()
            .key;
        let error = match TransformContext::from_build_config(
            BuildConfig::profile(Profile::Faithful).only_rule_for_tests(rule_key),
            &physics_then_braket,
        ) {
            Ok(_) => panic!("explicitly selecting an overridden rule should fail"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            TransformBuildError::Rewrite(PlanBuildError::SelectedRuleUnavailable {
                rule: rule_key,
                reason: RuleAvailabilityFailure::ConsumedDefinitionMismatch {
                    target: RuleTargetKey {
                        kind: RuleTargetKind::Command,
                        name: cmd_name,
                    },
                    mode: ContentMode::Math,
                    active: vec![
                        PackageName::Base,
                        PackageName::Ams,
                        PackageName::Physics,
                        PackageName::Braket,
                    ],
                },
            })
        );
        assert_eq!(
            error.to_string(),
            format!(
                "selected transform rule {rule_name} is unavailable: consumed command `{cmd_name}` resolves to a different definition in math mode with active packages [\"base\", \"ams\", \"physics\", \"braket\"]"
            )
        );
    }

    // Across all bounded configurations, only those three rules are ever excluded
    // by consumed-definition mismatch, and strictly when braket follows physics.
    let mut excluded_by_mismatch: BTreeSet<String> = BTreeSet::new();
    for config in bounded_configurations() {
        let active_plan_keys: BTreeSet<RuleKey> = config
            .equiv_ctx
            .rewrite_plan()
            .rules()
            .iter()
            .map(|rule| rule.meta().key)
            .collect();
        let physics_pos = config
            .packages
            .iter()
            .position(|&package| package == PackageName::Physics);
        let braket_pos = config
            .packages
            .iter()
            .position(|&package| package == PackageName::Braket);
        let braket_after_physics = matches!((physics_pos, braket_pos), (Some(p), Some(b)) if b > p);

        for rule in all_rules() {
            let meta = rule.meta();
            if active_plan_keys.contains(&meta.key) {
                continue;
            }
            if !meta
                .enabled_by_packages
                .iter()
                .any(|package| config.packages.contains(package))
            {
                continue;
            }
            let result = TransformContext::from_build_config(
                BuildConfig::profile(Profile::Equiv).only_rule_for_tests(meta.key),
                &config.parse_ctx,
            );
            if matches!(
                result,
                Err(TransformBuildError::Rewrite(
                    PlanBuildError::SelectedRuleUnavailable {
                        reason: RuleAvailabilityFailure::ConsumedDefinitionMismatch { .. },
                        ..
                    }
                ))
            ) {
                assert!(
                    braket_after_physics,
                    "unexpected consumed-definition mismatch for {} in packages {:?}",
                    meta.key, config.packages
                );
                excluded_by_mismatch.insert(meta.key.to_string());
            }
        }
    }

    let expected: BTreeSet<String> = physics_rules
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect();
    assert_eq!(excluded_by_mismatch, expected);
}

#[test]
fn produced_definition_mismatch_snapshot() {
    let mut minimal_by_pair: BTreeMap<(String, String), Vec<&'static str>> = BTreeMap::new();

    for config in bounded_configurations() {
        let package_names: Vec<&'static str> = config
            .packages
            .iter()
            .map(|package| package.as_str())
            .collect();
        for rule in config.equiv_ctx.rewrite_plan().rules() {
            let meta = rule.meta();
            for &target in meta.produces.targets {
                if target_mismatches_active_definition(&config.parse_ctx, target) {
                    let entry_key = (
                        meta.key.to_string(),
                        format!("{}:{}", target.key().kind_label(), target.key().name),
                    );
                    let replace = match minimal_by_pair.get(&entry_key) {
                        None => true,
                        Some(existing) => {
                            (package_names.len(), package_names.as_slice())
                                < (existing.len(), existing.as_slice())
                        }
                    };
                    if replace {
                        minimal_by_pair.insert(entry_key, package_names.clone());
                    }
                }
            }
        }
    }

    let actual: Vec<(String, String, Vec<&'static str>)> = minimal_by_pair
        .into_iter()
        .map(|((rule, target), packages)| (rule, target, packages))
        .collect();

    let expected_names = [
        "Pr", "arccos", "arcsin", "arctan", "cos", "cosh", "cot", "coth", "csc", "det", "exp",
        "ln", "log", "sec", "sin", "sinh", "tan", "tanh",
    ];
    let expected: Vec<(String, String, Vec<&'static str>)> = expected_names
        .into_iter()
        .map(|name| {
            (
                "ams/operatorname-to-named-function".to_string(),
                format!("command:{name}"),
                vec!["base", "ams", "physics"],
            )
        })
        .collect();

    assert_eq!(actual, expected);
}

#[test]
fn physics_braket_order_faithful_outputs_match_spec() {
    let normalize_faithful = |packages: &[&str], source: &str| -> String {
        let parse_ctx = ParseContext::from_packages(packages);
        let engine = TransformContext::from_build_config(
            BuildConfig::profile(Profile::Faithful),
            &parse_ctx,
        )
        .expect("faithful engine should build");
        let mut ast = round_trip::parse(&parse_ctx, source)
            .unwrap_or_else(|| panic!("parse failed for {source} with {packages:?}"));
        engine.run(&mut ast, &parse_ctx).unwrap_or_else(|error| {
            panic!("transform failed for {source} with {packages:?}: {error}")
        });
        serialize(&ast)
    };

    let cases: &[(&str, &str, &str)] = &[
        (
            r"\braket{\phi}{\psi}",
            r"\left \langle \phi \middle \vert \psi \right \rangle",
            r"\braket { \phi } { \psi }",
        ),
        (
            r"\braket{a|b}",
            r"\left \langle a | b \middle \vert a | b \right \rangle",
            r"\braket { a | b }",
        ),
        (
            r"\bra{\psi}",
            r"\left \langle \psi \right \vert",
            r"\bra { \psi }",
        ),
        (
            r"\ket*{\psi}",
            r"\vert \psi \rangle",
            r"\ket { * } { \psi }",
        ),
        (
            r"\ketbra{a}{b}",
            r"\ketbra { a } { b }",
            r"\ketbra { a } { b }",
        ),
    ];

    for &(source, physics_active, braket_active) in cases {
        assert_eq!(
            normalize_faithful(&["base", "ams", "physics"], source),
            physics_active,
            "base,ams,physics: {source}"
        );
        assert_eq!(
            normalize_faithful(&["base", "ams", "braket", "physics"], source),
            physics_active,
            "base,ams,braket,physics: {source}"
        );
        assert_eq!(
            normalize_faithful(&["base", "ams", "physics", "braket"], source),
            braket_active,
            "base,ams,physics,braket: {source}"
        );
        assert_eq!(
            normalize_faithful(&["base", "ams", "braket"], source),
            braket_active,
            "base,ams,braket: {source}"
        );
    }
}

#[test]
fn rule_metadata_eliminates_and_produces_are_disjoint() {
    for rule in all_rules() {
        let meta = rule.meta();
        for eliminated in meta.consumes.eliminates {
            let target = eliminated.key();
            assert!(
                !meta
                    .produces
                    .targets
                    .iter()
                    .any(|produced| produced.key() == target),
                "rule {} both eliminates and produces {} `{}`",
                meta.key,
                target.kind_label(),
                target.name
            );
        }
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
                && candidate.allowed_mode == record.allowed_mode
                && candidate.argspec.source == record.argspec.source
        }),
        RuleTarget::Environment(record) => builtin.environments.iter().any(|candidate| {
            candidate.name == record.name
                && candidate.argspec.source == record.argspec.source
                && candidate.body_mode == record.body_mode
        }),
        RuleTarget::Character(record) => builtin.characters.iter().any(|candidate| {
            candidate.name == record.name
                && candidate.allowed_mode == record.allowed_mode
                && candidate.unicode_value == record.unicode_value
        }),
    }
}

#[test]
fn command_signature_inference_distinguishes_allowed_modes() {
    use texform_knowledge::builtin::base;

    assert_eq!(
        packages_for_target_signature(RuleTarget::Command(&base::cmd::MKERN)),
        vec![PackageName::Base]
    );
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
