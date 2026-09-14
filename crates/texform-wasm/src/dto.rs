use serde::Serialize;
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::Document;

pub(crate) fn to_js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new()
        .serialize_missing_as_null(true)
        .serialize_maps_as_objects(true);
    value
        .serialize(&serializer)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

pub(crate) fn js_set(target: &JsValue, key: &str, value: &JsValue) -> Result<(), JsValue> {
    js_sys::Reflect::set(target, &JsValue::from_str(key), value).map(|_| ())
}

pub(crate) fn binding_dto_to_json<T: Serialize>(value: &T) -> Result<serde_json::Value, String> {
    serde_json::to_value(value)
        .map(camelize_json_keys)
        .map_err(|error| error.to_string())
}

pub(crate) fn binding_dto_to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let value = binding_dto_to_json(value).map_err(internal_message_to_js)?;
    to_js_value(&value).map_err(|error| {
        internal_message_to_js(
            error
                .as_string()
                .unwrap_or_else(|| "failed to convert binding DTO to JS".to_owned()),
        )
    })
}

fn camelize_json_keys(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    (
                        texform::bindings::snake_to_camel(&key),
                        camelize_json_keys(value),
                    )
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(camelize_json_keys).collect())
        }
        other => other,
    }
}

fn join_json_path(path: &[String]) -> String {
    let mut out = String::new();
    for segment in path {
        if segment.starts_with('[') {
            out.push_str(segment);
        } else {
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(segment);
        }
    }
    out
}

/// Convert a camelCase identifier to snake_case (`maxGroupDepth` → `max_group_depth`).
pub(crate) fn camel_to_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Recursively convert object keys to snake_case. Rejects keys that already
/// contain `_` so JS callers cannot smuggle snake_case names through.
pub(crate) fn snakeize_json_keys(
    value: serde_json::Value,
    path: &mut Vec<String>,
) -> Result<serde_json::Value, (String, String)> {
    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                if key.contains('_') {
                    path.push(key.clone());
                    let joined = join_json_path(path);
                    return Err((joined, key));
                }
                path.push(key.clone());
                let converted = snakeize_json_keys(child, path)?;
                path.pop();
                out.insert(camel_to_snake(&key), converted);
            }
            Ok(serde_json::Value::Object(out))
        }
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (index, child) in items.into_iter().enumerate() {
                path.push(format!("[{index}]"));
                out.push(snakeize_json_keys(child, path)?);
                path.pop();
            }
            Ok(serde_json::Value::Array(out))
        }
        other => Ok(other),
    }
}

pub(crate) fn binding_error_to_js(error: texform::bindings::BindingErrorDto) -> JsValue {
    binding_error_parts_to_js(texform::bindings::BindingErrorParts {
        error,
        document: None,
    })
}

pub(crate) fn binding_error_parts_to_js(parts: texform::bindings::BindingErrorParts) -> JsValue {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = parts;
        js_error_message("")
    }

    #[cfg(target_arch = "wasm32")]
    {
        let error = js_sys::Error::new(&parts.error.message);
        error.set_name(binding_error_name(parts.error.kind));
        let value: JsValue = error.into();
        if let Err(error) = js_set(&value, "kind", &parts.error.kind.into()) {
            return error;
        }
        if parts.error.kind == "parse" {
            let diagnostics = to_js_value(&parts.error.diagnostics).unwrap_or(JsValue::NULL);
            let document = match parts.document {
                Some(document) => Document::from_core(document).into(),
                None => JsValue::NULL,
            };
            if let Err(error) = js_set(&value, "diagnostics", &diagnostics) {
                return error;
            }
            if let Err(error) = js_set(&value, "document", &document) {
                return error;
            }
        }
        value
    }
}

#[cfg(target_arch = "wasm32")]
fn binding_error_name(kind: &str) -> &'static str {
    match kind {
        "parse" => "TexformParseError",
        "edit" => "TexformEditError",
        "config" => "TexformConfigError",
        "transform" => "TexformTransformError",
        _ => "TexformError",
    }
}

pub(crate) fn config_error_to_js(message: impl Into<String>) -> JsValue {
    binding_error_to_js(texform::bindings::config_error_to_dto(message))
}

pub(crate) fn parse_message_to_js(message: impl Into<String>) -> JsValue {
    binding_error_to_js(texform::bindings::BindingErrorDto {
        kind: "parse",
        message: message.into(),
        diagnostics: Vec::new(),
    })
}

pub(crate) fn edit_message_to_js(message: impl Into<String>) -> JsValue {
    binding_error_to_js(texform::bindings::BindingErrorDto {
        kind: "edit",
        message: message.into(),
        diagnostics: Vec::new(),
    })
}

pub(crate) fn internal_message_to_js(message: impl Into<String>) -> JsValue {
    binding_error_to_js(texform::bindings::BindingErrorDto {
        kind: "internal",
        message: message.into(),
        diagnostics: Vec::new(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn js_error_message(_message: &str) -> JsValue {
    JsValue::NULL
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn camel_to_snake_splits_internal_capitals() {
        assert_eq!(camel_to_snake("maxGroupDepth"), "max_group_depth");
        assert_eq!(camel_to_snake("flattenGroups"), "flatten_groups");
        assert_eq!(camel_to_snake("enabled"), "enabled");
    }

    #[test]
    fn snakeize_json_keys_converts_nested_objects_and_leaves_values() {
        let converted = snakeize_json_keys(
            json!({
                "flattenGroups": {
                    "preserveEmptyGroup": true
                },
                "rewrite": { "maxIterations": 4, "enabled": false }
            }),
            &mut Vec::new(),
        )
        .expect("camelCase keys should convert");

        assert_eq!(
            converted,
            json!({
                "flatten_groups": { "preserve_empty_group": true },
                "rewrite": { "max_iterations": 4, "enabled": false }
            })
        );
    }

    #[test]
    fn snakeize_json_keys_rejects_underscore_keys_with_path() {
        let top = snakeize_json_keys(
            json!({ "flatten_groups": { "enabled": false } }),
            &mut Vec::new(),
        )
        .expect_err("top-level snake_case should be rejected");
        assert_eq!(top, ("flatten_groups".into(), "flatten_groups".into()));

        let nested = snakeize_json_keys(
            json!({ "flattenGroups": { "preserve_empty_group": true } }),
            &mut Vec::new(),
        )
        .expect_err("nested snake_case should be rejected");
        assert_eq!(
            nested,
            (
                "flattenGroups.preserve_empty_group".into(),
                "preserve_empty_group".into()
            )
        );
    }

    #[test]
    fn snakeize_json_keys_recurses_into_arrays() {
        let converted = snakeize_json_keys(json!([{ "allowedMode": "math" }]), &mut Vec::new())
            .expect("array objects should convert");
        assert_eq!(converted, json!([{ "allowed_mode": "math" }]));
    }

    #[test]
    fn snakeize_json_keys_leaves_string_values_untouched() {
        let converted = snakeize_json_keys(json!({ "order": "sup_first" }), &mut Vec::new())
            .expect("values should not be renamed");
        assert_eq!(converted, json!({ "order": "sup_first" }));
    }

    #[test]
    fn binding_dto_to_json_surfaces_serialize_errors() {
        struct FailingDto;
        impl Serialize for FailingDto {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(serde::ser::Error::custom("boom"))
            }
        }

        let error = binding_dto_to_json(&FailingDto).expect_err("serialize error should surface");
        assert!(
            error.contains("boom"),
            "error should include the serialize message, got {error}"
        );
    }

    #[test]
    fn binding_dto_to_json_camelizes_nested_dto_keys() {
        let value = binding_dto_to_json(&texform::validate_argspec("o m"))
            .expect("argspec result should serialize");

        assert_eq!(value["valid"], true);
        assert_eq!(value["argCount"], 2);
        assert!(value.get("arg_count").is_none());
        let first_slot = &value["parsed"][0];
        assert!(first_slot.get("noLeadingSpace").is_some());
        assert!(first_slot.get("no_leading_space").is_none());
    }
}
