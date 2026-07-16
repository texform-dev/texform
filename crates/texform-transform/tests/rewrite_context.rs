use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ParseContext, ParseContextBuilder,
};
use texform_knowledge::builtin::MANAGED_PACKAGE_IMPORT_ORDER;
use texform_transform::rewrite::{RuleAvailabilityFailure, all_rules};
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
        .insert_item(CommandItem::new(
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
