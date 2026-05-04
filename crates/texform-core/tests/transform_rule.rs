use texform_core::transform::{PackageName, RuleKey, RuleTier};

#[test]
fn rule_tier_strings_match_profile_ids() {
    assert_eq!(RuleTier::Base.as_str(), "base");
    assert_eq!(RuleTier::Expand.as_str(), "expand");
    assert_eq!(RuleTier::Deep.as_str(), "deep");
}

#[test]
fn rule_key_display_uses_generated_package_name() {
    let key = RuleKey {
        package: PackageName::Physics,
        name: "trace-to-tr",
    };
    assert_eq!(key.to_string(), "physics/trace-to-tr");
}
