pub use texform_transform::{Profile, RuleKey, TransformConfig};

pub fn rule_key_from_name(name: &str) -> Option<RuleKey> {
    texform_transform::rewrite::all_rules()
        .iter()
        .find_map(|rule| {
            let key = rule.meta().key;
            (key.to_string() == name).then_some(key)
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizeConfig {
    pub parse: texform_core::parse::ParseConfig,
    pub transform: TransformConfig,
}
