use texform_core::parse::{AllowedMode, CommandItem, CommandKind, ParseContextBuilder};
use texform_core::transform::{BuiltinRuleSetId, TransformContextBuilder};

#[test]
fn only_many_keeps_the_requested_rules() {
    let parse_ctx = ParseContextBuilder::new()
        .packages(&["physics"])
        .build()
        .expect("parse context should build");

    let transform_ctx = TransformContextBuilder::new(BuiltinRuleSetId::Normalize)
        .only_many(&[
            texform_core::transform::registry::rules_for_ruleset(BuiltinRuleSetId::Normalize)[1]
                .meta()
                .key,
            texform_core::transform::registry::rules_for_ruleset(BuiltinRuleSetId::Normalize)[2]
                .meta()
                .key,
        ])
        .build_with(&parse_ctx)
        .expect("transform context should build");

    let active_keys = transform_ctx
        .normalize_rules()
        .iter()
        .map(|rule| rule.meta().key)
        .collect::<Vec<_>>();

    assert_eq!(active_keys.len(), 2);
}

#[test]
fn build_with_disables_rules_touching_mutated_command_names() {
    let parse_ctx = ParseContextBuilder::new()
        .packages(&["physics"])
        .insert_item(CommandItem::new(
            "quantity",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m",
        ))
        .build()
        .expect("parse context should build");

    let transform_ctx = TransformContextBuilder::new(BuiltinRuleSetId::Normalize)
        .build_with(&parse_ctx)
        .expect("transform context should build");

    let active = transform_ctx
        .normalize_rules()
        .iter()
        .map(|rule| rule.meta().key.to_string())
        .collect::<Vec<_>>();

    assert!(!active.iter().any(|key| key == "physics/quantity-to-qty"));
}
