//! Session state and method handlers of the normalizer protocol.

use std::any::Any;
use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use texform::ParseDiagnostic;
use texform::bindings::{
    NormalizeConfigInput, TransformReportDto, format_read_error, normalize_error_to_parts, read,
};

use super::rpc::RpcError;
use crate::normalizer::{Normalized, Normalizer, ProfileName, StageTimings, Stopwatch};
use crate::output::panic_message;
use crate::{build_info, packages};

/// Bumped only for incompatible changes to method semantics or required
/// fields; optional fields and new methods keep the version.
const PROTOCOL_VERSION: u32 = 1;

pub struct Server {
    default_packages: Vec<String>,
    initialized: bool,
    shut_down: bool,
    /// Configurations that `normalize` requests reference by id.
    configs: HashMap<String, Configured>,
}

/// A configured normalizer plus the output style of its session.
struct Configured {
    normalizer: Normalizer,
    style: texform::Style,
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
        match method {
            "initialize" => self.initialize(params),
            "configure" => self.configure(params),
            "normalize" => self.normalize(params),
            "shutdown" => self.shutdown(params),
            _ => Err(RpcError::method_not_found(method)),
        }
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
                .map_err(RpcError::invalid_params)?;
        let resolved = to_json(Resolved {
            style: spec.style,
            profile: spec.profile,
            packages: packages::canonical(requested),
            config: NormalizeConfigInput::from_config(normalizer.config.clone()),
        });
        self.configs.insert(
            id,
            Configured {
                normalizer,
                style: spec.style,
            },
        );
        Ok(serde_json::json!({ "resolved": resolved }))
    }

    fn normalize(&mut self, params: Option<Value>) -> Result<Value, RpcError> {
        let params: NormalizeParams = parse_params(params)?;
        let configured = self.configs.get(&params.config).ok_or_else(|| {
            RpcError::invalid_params(format!("config `{}` is not configured", params.config))
        })?;
        let timing = params.timing.unwrap_or(false);
        let mut timings = StageTimings::default();
        let outcome = configured.normalizer.normalize_staged(
            &params.latex,
            configured.style,
            params.report.unwrap_or(false),
            &mut Stopwatch::start(timing),
            &mut timings,
        );
        let timing = timing.then_some(timings);
        match outcome {
            Ok(Normalized { output, report }) => Ok(to_json(NormalizeResult {
                output,
                report,
                timing,
            })),
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
    #[serde(default)]
    style: texform::Style,
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
    report: Option<bool>,
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
    style: texform::Style,
    profile: ProfileName,
    packages: Vec<String>,
    #[serde(flatten)]
    config: NormalizeConfigInput,
}

#[derive(Serialize)]
struct NormalizeResult {
    output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<TransformReportDto>,
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
