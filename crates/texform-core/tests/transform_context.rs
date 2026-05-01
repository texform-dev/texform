use texform_core::parse::{AllowedMode, CommandItem, CommandKind, ParseContextBuilder};
use texform_core::transform::{RuleTier, TransformBuildError, TransformProfile};

#[test]
fn only_many_keeps_the_requested_rules() {
    let parse_ctx = ParseContextBuilder::default()
        .packages(&["physics"])
        .build()
        .expect("parse context should build");

    let all_rules = texform_core::transform::registry::all_rules();
    let requested = [all_rules[1].meta().key, all_rules[2].meta().key];

    let transform_ctx = TransformProfile::AUTHORING
        .builder()
        .only_many(&requested)
        .build_with(&parse_ctx)
        .expect("transform context should build");

    let active_keys = transform_ctx
        .normalize_rules()
        .iter()
        .map(|rule| rule.meta().key)
        .collect::<Vec<_>>();

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

    let transform_ctx = TransformProfile::AUTHORING
        .builder()
        .build_with(&parse_ctx)
        .expect("transform context should build");

    let active = transform_ctx
        .normalize_rules()
        .iter()
        .map(|rule| rule.meta().key.to_string())
        .collect::<Vec<_>>();

    assert!(!active.iter().any(|key| key == "physics/quantity-to-qty"));
}

#[test]
fn empty_rule_set_error_reports_profile_name() {
    let parse_ctx = ParseContextBuilder::default()
        .packages(&["physics"])
        .build()
        .expect("parse context should build");

    let builder = texform_core::transform::registry::all_rules()
        .iter()
        .copied()
        .fold(TransformProfile::AUTHORING.builder(), |builder, rule| {
            builder.disable(rule.meta().key)
        });

    let error = match builder.build_with(&parse_ctx) {
        Ok(_) => panic!("disabling all authoring rules should fail"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        TransformBuildError::EmptyRuleSet {
            profile: "authoring",
        }
    );
}

#[test]
fn only_does_not_bypass_profile_tier_filter() {
    let parse_ctx = ParseContextBuilder::default()
        .packages(&["physics"])
        .build()
        .expect("parse context should build");

    let base_rule = texform_core::transform::registry::all_rules()[0].meta().key;
    let deep_only = TransformProfile {
        name: "deep-only",
        tiers: &[RuleTier::Deep],
    };

    let error = match deep_only.builder().only(base_rule).build_with(&parse_ctx) {
        Ok(_) => panic!("only() must still respect the profile tier filter"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        TransformBuildError::EmptyRuleSet {
            profile: "deep-only",
        }
    );
}
