//! Engine construction and the normalize pipeline shared by `normalize`,
//! `tokenize --profile`, and the serve protocol.
//!
//! All three resolve a profile, a package selection, and a
//! `NormalizeConfigInput` overlay the same way, so a given configuration means
//! the same thing on the command line and over the protocol. `normalize` and
//! the protocol's `normalize` also run the same [`Normalizer::normalize_staged`]
//! sequence, so their output, error classification, and reports agree.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use texform::bindings::{NormalizeConfigInput, TransformReportDto, transform_report_to_dto};
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
    ) -> Result<Self, String> {
        let names: Vec<&str> = packages.iter().map(String::as_str).collect();
        let knowledge_base = texform::KnowledgeBase::builder()
            .packages(&names)
            .build()
            .map_err(|error| error.to_string())?;
        let engine = TransformEngine::builder()
            .knowledge_base(knowledge_base)
            .profile(profile.into())
            .build()
            .map_err(|error| error.to_string())?;
        let config = overrides.into_config(engine.default_normalize_config());
        if config.transform.rewrite.max_iterations == 0 {
            return Err("rewrite.max_iterations must be greater than zero".to_owned());
        }
        Ok(Self { engine, config })
    }

    /// Untimed [`normalize_staged`](Self::normalize_staged).
    pub fn normalize(
        &self,
        latex: &str,
        style: texform::Style,
        with_report: bool,
    ) -> Result<Normalized, texform::Error> {
        self.normalize_staged(
            latex,
            style,
            with_report,
            &mut Stopwatch::start(false),
            &mut StageTimings::default(),
        )
    }

    /// `TransformEngine::normalize_with`, composed from its public stages so
    /// each stage can be timed, with the output serialized in `style`.
    ///
    /// Timed and untimed requests both run this exact sequence, so `timing`
    /// cannot change the output or the error classification. A stage's time is
    /// recorded only if the stage ran.
    pub fn normalize_staged(
        &self,
        latex: &str,
        style: texform::Style,
        with_report: bool,
        clock: &mut Stopwatch,
        timings: &mut StageTimings,
    ) -> Result<Normalized, texform::Error> {
        let parsed = self
            .engine
            .parser()
            .parse_with(latex, &self.config.parse)
            .try_into_document();
        timings.parse_ns = clock.lap();
        let (mut document, _diagnostics) = parsed?;

        let transformed = if with_report {
            self.engine
                .transform_with_report(&mut document, &self.config.transform)
                .map(Some)
        } else {
            self.engine
                .transform_with(&mut document, &self.config.transform)
                .map(|()| None)
        };
        timings.transform_ns = clock.lap();
        let report = transformed?;

        let serialized = document.to_latex_with(&style.into());
        timings.serialize_ns = clock.lap();
        Ok(Normalized {
            output: serialized?,
            report: report.as_ref().map(transform_report_to_dto),
        })
    }
}

/// Serialized output of one normalize call, plus the report when requested.
#[derive(Serialize)]
pub struct Normalized {
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<TransformReportDto>,
}

/// Measures consecutive stages. A disabled stopwatch never reads the clock.
pub struct Stopwatch {
    last: Option<Instant>,
}

impl Stopwatch {
    pub fn start(enabled: bool) -> Self {
        Self {
            last: enabled.then(Instant::now),
        }
    }

    /// Nanoseconds since the previous lap, or `None` when disabled.
    fn lap(&mut self) -> Option<u64> {
        let last = self.last?;
        let now = Instant::now();
        self.last = Some(now);
        Some(u64::try_from(now.duration_since(last).as_nanos()).unwrap_or(u64::MAX))
    }
}

/// Stage durations in nanoseconds; stages that did not run are omitted.
#[derive(Default, Serialize)]
pub struct StageTimings {
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transform_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serialize_ns: Option<u64>,
}

/// Serializer style as written on the command line.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum StyleName {
    Spaced,
    Compact,
}

impl From<StyleName> for texform::Style {
    fn from(name: StyleName) -> Self {
        match name {
            StyleName::Spaced => Self::Spaced,
            StyleName::Compact => Self::Compact,
        }
    }
}
