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

#[test]
fn public_profiles_enable_finalize_ast_by_default() {
    for profile in [
        texform::Profile::Authoring,
        texform::Profile::Faithful,
        texform::Profile::Corpus,
        texform::Profile::Equiv,
    ] {
        assert!(
            profile.default_transform_config().finalize_ast.enabled,
            "{profile:?} should enable FinalizeAst by default"
        );
    }
}
