use texform_core::transform::{PackageName, RuleClass, RuleKey};

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
