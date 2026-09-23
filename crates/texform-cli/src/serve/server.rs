//! Session state and method handlers of the normalizer protocol.

use std::any::Any;
use std::collections::HashMap;
use std::time::Instant;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use texform::ParseDiagnostic;
use texform::bindings::{NormalizeConfigInput, format_read_error, normalize_error_to_parts, read};

use super::rpc::RpcError;
use crate::normalizer::{Normalizer, ProfileName};
use crate::output::panic_message;
use crate::{build_info, packages};

/// Bumped only for incompatible changes to method semantics or required
/// fields; optional fields and new methods keep the version.
const PROTOCOL_VERSION: u32 = 1;

pub struct Server {
    default_packages: Vec<String>,
    initialized: bool,
    shut_down: bool,
    /// Normalizers that `normalize` requests reference by id.
    configs: HashMap<String, Normalizer>,
}

impl Server {
    pub fn new(default_packages: Vec<String>) -> Self {
        Self {
            default_packages,
            initialized: false,
            shut_down: false,
            configs: HashMap::new(),
        }
    }

    pub fn dispatch(&mut self, method: &str, params: Option<Value>) -> Result<Value, RpcError> {
        if self.shut_down {
            return Err(RpcError::invalid_request("server has been shut down"));
        }
        if !self.initialized && method != "initialize" {
            return Err(RpcError::not_initialized());
        }
        let handler: fn(&mut Self, Option<Value>) -> Result<Value, RpcError> = match method {
            "initialize" => Self::initialize,
            "configure" => Self::configure,
            "normalize" => Self::normalize,
            "shutdown" => Self::shutdown,
            _ => return Err(RpcError::method_not_found(method)),
        };
        handler(self, params)
    }

    /// Repeating `initialize` is harmless and returns the same result.
    fn initialize(&mut self, params: Option<Value>) -> Result<Value, RpcError> {
        let EmptyParams {} = parse_params(params)?;
        self.initialized = true;
        Ok(to_json(InitializeResult {
            protocol_version: PROTOCOL_VERSION,
            server_info: ServerInfo {
                name: "texform",
                version: build_info::VERSION,
                commit: build_info::commit(),
                commit_date: build_info::commit_date(),
                dirty: build_info::dirty(),
            },
        }))
    }

    /// Build an engine and cache it under the given id, replacing any previous
    /// entry. A failed `configure` leaves the cache unchanged.
    fn configure(&mut self, params: Option<Value>) -> Result<Value, RpcError> {
        let ConfigureParams { id, config: spec } = parse_params(params)?;
        let requested = spec.packages.as_deref().unwrap_or(&self.default_packages);
        let normalizer =
            Normalizer::build(spec.profile, requested, spec.overrides.unwrap_or_default())
                .map_err(|error| RpcError::invalid_params(error.to_string()))?;
        let resolved = to_json(Resolved {
            profile: spec.profile,
            packages: packages::canonical(requested),
            config: NormalizeConfigInput::from_config(normalizer.config.clone()),
        });
        self.configs.insert(id, normalizer);
        Ok(serde_json::json!({ "resolved": resolved }))
    }

    fn normalize(&mut self, params: Option<Value>) -> Result<Value, RpcError> {
        let params: NormalizeParams = parse_params(params)?;
        let normalizer = self.configs.get(&params.config).ok_or_else(|| {
            RpcError::invalid_params(format!("config `{}` is not configured", params.config))
        })?;
        let timing = params.timing.unwrap_or(false);
        let mut timings = StageTimings::default();
        let outcome =
            normalizer.normalize_staged(&params.latex, &mut Stopwatch::start(timing), &mut timings);
        let timing = timing.then_some(timings);
        match outcome {
            Ok(output) => Ok(to_json(NormalizeResult { output, timing })),
            Err(error) => {
                let error = normalize_error_to_parts(error).error;
                let data = FailureData {
                    kind: error.kind,
                    diagnostics: error.diagnostics,
                    timing,
                };
                Err(RpcError::normalize_failed(error.message, to_json(data)))
            }
        }
    }

    fn shutdown(&mut self, params: Option<Value>) -> Result<Value, RpcError> {
        let EmptyParams {} = parse_params(params)?;
        self.shut_down = true;
        Ok(Value::Null)
    }
}

impl Normalizer {
    /// `TransformEngine::normalize_with`, composed from its public stages so
    /// each stage can be timed.
    ///
    /// Timed and untimed requests both run this exact sequence, so `timing`
    /// cannot change the output or the error classification. A stage's time is
    /// recorded only if the stage ran.
    fn normalize_staged(
        &self,
        latex: &str,
        clock: &mut Stopwatch,
        timings: &mut StageTimings,
    ) -> Result<String, texform::Error> {
        let parsed = self
            .engine
            .parser()
            .parse_with(latex, &self.config.parse)
            .try_into_document();
        timings.parse_ns = clock.lap();
        let (mut document, _diagnostics) = parsed?;

        let transformed = self
            .engine
            .transform_with(&mut document, &self.config.transform);
        timings.transform_ns = clock.lap();
        transformed?;

        let serialized = document.to_latex();
        timings.serialize_ns = clock.lap();
        Ok(serialized?)
    }
}

/// `code 1, kind "internal"` failure for a request whose handler panicked.
pub fn panic_error(payload: &(dyn Any + Send)) -> RpcError {
    let detail = panic_message(payload);
    let data = FailureData {
        kind: "internal",
        diagnostics: Vec::new(),
        timing: None,
    };
    RpcError::normalize_failed(format!("internal panic: {detail}"), to_json(data))
}

/// Deserialize request params, rejecting non-object params and reporting the
/// offending path. Omitted params are treated as `{}`.
fn parse_params<T: DeserializeOwned>(params: Option<Value>) -> Result<T, RpcError> {
    let params = params.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    read(params).map_err(|error| {
        RpcError::invalid_params(format_read_error(&error, "params", str::to_owned))
    })
}

fn to_json(value: impl Serialize) -> Value {
    // Every value serialized here consists of strings, numbers, booleans,
    // options, sequences, and structs, which cannot fail to serialize.
    serde_json::to_value(value).expect("protocol values serialize to JSON")
}

/// Measures consecutive stages. A disabled stopwatch never reads the clock.
struct Stopwatch {
    last: Option<Instant>,
}

impl Stopwatch {
    fn start(enabled: bool) -> Self {
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

#[derive(Deserialize)]
#[serde(expecting = "an object")]
struct EmptyParams {}

#[derive(Deserialize)]
#[serde(expecting = "an object")]
struct ConfigureParams {
    id: String,
    config: ConfigSpec,
}

/// Normalizer configuration. Unknown keys are rejected so that a client
/// relying on an unsupported option fails at `configure` instead of silently
/// measuring a different configuration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "an object")]
struct ConfigSpec {
    profile: ProfileName,
    packages: Option<Vec<String>>,
    overrides: Option<NormalizeConfigInput>,
}

#[derive(Deserialize)]
#[serde(expecting = "an object")]
struct NormalizeParams {
    config: String,
    latex: String,
    timing: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult {
    protocol_version: u32,
    server_info: ServerInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerInfo {
    name: &'static str,
    version: &'static str,
    commit: Option<&'static str>,
    commit_date: Option<&'static str>,
    dirty: bool,
}

/// Fully expanded effective config plus the engine selection.
#[derive(Serialize)]
struct Resolved {
    profile: ProfileName,
    packages: Vec<String>,
    #[serde(flatten)]
    config: NormalizeConfigInput,
}

#[derive(Serialize)]
struct NormalizeResult {
    output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timing: Option<StageTimings>,
}

#[derive(Serialize)]
struct FailureData {
    kind: &'static str,
    diagnostics: Vec<ParseDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timing: Option<StageTimings>,
}

/// Stage durations in nanoseconds; stages that did not run are omitted.
#[derive(Default, Serialize)]
struct StageTimings {
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transform_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serialize_ns: Option<u64>,
}
