//! Configuration for normalization.
//!
//! A [`Profile`] picks the normalization levels for a downstream scenario
//! ([`Authoring`](Profile::Authoring), [`Faithful`](Profile::Faithful),
//! [`Corpus`](Profile::Corpus), [`Equiv`](Profile::Equiv)); [`TransformConfig`]
//! holds the per-run phase switches; and [`NormalizeConfig`] bundles a
//! [`ParseConfig`](crate::ParseConfig) with a [`TransformConfig`] for the
//! string-to-string [`normalize_with`](crate::TransformEngine::normalize_with)
//! path. Individual rules are addressed by [`RuleKey`]; [`rule_key_from_name`]
//! resolves a key from its stable string name.

pub use texform_transform::{Profile, RuleKey, TransformConfig};

/// Resolve a transform [`RuleKey`] from its stable string name.
///
/// Rule names are the stable identifiers used by
/// [`TransformEngineBuilder::disable_rule_by_name`](crate::TransformEngineBuilder::disable_rule_by_name)
/// and by external tooling. Returns `None` if no built-in rule carries the
/// given name.
pub fn rule_key_from_name(name: &str) -> Option<RuleKey> {
    texform_transform::rewrite::all_rules()
        .iter()
        .find_map(|rule| {
            let key = rule.meta().key;
            (key.to_string() == name).then_some(key)
        })
}

/// Combined parse and transform configuration for a single normalize call.
///
/// Used by [`TransformEngine::normalize_with`](crate::TransformEngine::normalize_with),
/// the string-to-string path, to control both how the source is parsed and how
/// the resulting tree is transformed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizeConfig {
    /// How the source LaTeX is parsed before transformation.
    pub parse: texform_core::parse::ParseConfig,
    /// Which transform phases and rules run, and their per-run switches.
    pub transform: TransformConfig,
}
