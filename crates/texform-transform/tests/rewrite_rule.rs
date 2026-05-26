use texform_knowledge::builtin::bboldx;
use texform_transform::{RuleTarget, RuleTargetKind};

#[test]
fn character_rule_target_exposes_lookup_key() {
    let target = RuleTarget::Character(&bboldx::chars::BBDOTLESSI);
    let key = target.key();

    assert_eq!(key.kind, RuleTargetKind::Character);
    assert_eq!(key.name, "bbdotlessi");
}
