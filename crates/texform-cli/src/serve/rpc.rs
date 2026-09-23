//! JSON-RPC 2.0 envelope: request validation, responses, and error objects.

use serde::Serialize;
use serde_json::Value;

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const SERVER_NOT_INITIALIZED: i64 = -32002;
/// Application code for a formula that failed to normalize.
pub const NORMALIZE_FAILED: i64 = 1;

/// A structurally valid request or notification.
pub struct Request {
    /// `None` for a notification, which never receives a response.
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// A message that is valid JSON but not a valid request.
pub struct InvalidRequest {
    /// The request id when it could be read, otherwise `null`.
    pub id: Value,
    pub error: RpcError,
}

impl Request {
    pub fn from_value(value: Value) -> Result<Self, InvalidRequest> {
        let invalid = |id: Value, detail: &str| InvalidRequest {
            id,
            error: RpcError::invalid_request(detail),
        };
        let mut object = match value {
            Value::Object(object) => object,
            Value::Array(_) => {
                return Err(invalid(Value::Null, "batch requests are not supported"));
            }
            _ => return Err(invalid(Value::Null, "message must be a JSON object")),
        };
        let id = match object.remove("id") {
            None => None,
            Some(id @ (Value::Null | Value::Number(_) | Value::String(_))) => Some(id),
            Some(_) => {
                return Err(invalid(
                    Value::Null,
                    "`id` must be a string, a number, or null",
                ));
            }
        };
        let reply_id = id.clone().unwrap_or(Value::Null);
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(invalid(reply_id, "`jsonrpc` must be \"2.0\""));
        }
        let method = match object.remove("method") {
            Some(Value::String(method)) => method,
            _ => return Err(invalid(reply_id, "`method` must be a string")),
        };
        let params = match object.remove("params") {
            None => None,
            Some(params @ (Value::Object(_) | Value::Array(_))) => Some(params),
            Some(_) => {
                return Err(invalid(reply_id, "`params` must be an object or an array"));
            }
        };
        Ok(Self { id, method, params })
    }
}

#[derive(Serialize)]
pub struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

impl Response {
    pub fn new(id: Value, outcome: Result<Value, RpcError>) -> Self {
        let (result, error) = match outcome {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            jsonrpc: "2.0",
            id,
            result,
            error,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: String, data: Option<Value>) -> Self {
        Self {
            code,
            message,
            data,
        }
    }

    pub fn parse_error(detail: impl std::fmt::Display) -> Self {
        Self::new(PARSE_ERROR, format!("Parse error: {detail}"), None)
    }

    pub fn invalid_request(detail: &str) -> Self {
        Self::new(INVALID_REQUEST, format!("Invalid Request: {detail}"), None)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
            None,
        )
    }

    /// Detail goes in `data.message`, as defined by the protocol.
    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self::new(
            INVALID_PARAMS,
            "Invalid params".to_owned(),
            Some(serde_json::json!({ "message": detail.into() })),
        )
    }

    pub fn not_initialized() -> Self {
        Self::new(
            SERVER_NOT_INITIALIZED,
            "Server not initialized".to_owned(),
            None,
        )
    }

    /// Per-formula failure. `data` carries `kind`, `diagnostics`, and
    /// optionally `timing`.
    pub fn normalize_failed(message: String, data: Value) -> Self {
        Self::new(NORMALIZE_FAILED, message, Some(data))
    }
}
