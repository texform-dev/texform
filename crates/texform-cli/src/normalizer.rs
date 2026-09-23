//! Engine construction shared by `normalize`, `tokenize --profile`, and the
//! serve protocol's `configure`.
//!
//! All three resolve a profile, a package selection, and a
//! `NormalizeConfigInput` overlay the same way, so a given configuration means
//! the same thing on the command line and over the protocol.

use serde::{Deserialize, Serialize};
use texform::bindings::NormalizeConfigInput;
use texform::{NormalizeConfig, Profile, TransformEngine};

/// Transform profile name as written on the command line and in the protocol.
#[derive(Clone, Copy, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ProfileName {
    Authoring,
    Faithful,
    Corpus,
    Equiv,
}

impl From<ProfileName> for Profile {
    fn from(name: ProfileName) -> Self {
        match name {
            ProfileName::Authoring => Profile::Authoring,
            ProfileName::Faithful => Profile::Faithful,
            ProfileName::Corpus => Profile::Corpus,
            ProfileName::Equiv => Profile::Equiv,
        }
    }
}

/// An engine plus the effective config its normalize calls use.
pub struct Normalizer {
    pub engine: TransformEngine,
    pub config: NormalizeConfig,
}

impl Normalizer {
    /// Build an engine for `profile` over `packages` and layer `overrides` on
    /// the engine's default normalize config.
    pub fn build(
        profile: ProfileName,
        packages: &[String],
        overrides: NormalizeConfigInput,
    ) -> Result<Self, texform::Error> {
        let names: Vec<&str> = packages.iter().map(String::as_str).collect();
        let engine = TransformEngine::builder()
            .packages(&names)
            .profile(profile.into())
            .build()?;
        let config = overrides.into_config(engine.default_normalize_config());
        Ok(Self { engine, config })
    }
}
