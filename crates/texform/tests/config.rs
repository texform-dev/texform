#[test]
fn rule_key_from_name_resolves_public_rule_names() {
    let key = texform::rule_key_from_name("physics/quantity-to-qty")
        .expect("known public rule should resolve");

    assert_eq!(key.to_string(), "physics/quantity-to-qty");
}

#[test]
fn rule_key_from_name_rejects_unknown_names() {
    assert!(texform::rule_key_from_name("missing/rule").is_none());
}
