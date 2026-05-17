use texform_specs::builtin::bboldx;
use texform_transform::{PackageName, RuleClass, RuleKey, RuleTarget, RuleTargetKind};

#[test]
fn rule_class_strings_match_profile_ids() {
    assert_eq!(RuleClass::Standard.as_str(), "standard");
    assert_eq!(RuleClass::Expand.as_str(), "expand");
    assert_eq!(RuleClass::Drop.as_str(), "drop");
    assert_eq!(RuleClass::Equiv.as_str(), "equiv");
}

#[test]
fn rule_key_display_uses_generated_package_name() {
    let key = RuleKey {
        package: PackageName::Physics,
        name: "trace-to-tr",
    };
    assert_eq!(key.to_string(), "physics/trace-to-tr");
}

#[test]
fn character_rule_target_exposes_public_key_and_labels() {
    let target = RuleTarget::Character(&bboldx::chars::BBDOTLESSI);
    let key = target.key();

    assert_eq!(key.kind, RuleTargetKind::Character);
    assert_eq!(key.name, "bbdotlessi");
    assert_eq!(key.kind_label(), "character");
    assert_eq!(target.kind_label(), "character");
    assert_eq!(target.name(), "bbdotlessi");
}
