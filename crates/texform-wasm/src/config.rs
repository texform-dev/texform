use serde::Deserialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

use texform::bindings::{
    ContextItemInput, NormalizeConfigInput, ParseConfigInput, SerializeOptionsInput,
    TransformConfigInput,
};
use texform::{ContextItem, ParseConfig, ParserBuildError, SerializeOptions, TransformConfig};

use crate::dto::{config_error_to_js, snakeize_json_keys};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, expecting = "an object")]
pub(crate) struct ParserOptions {
    pub(crate) packages: Option<Vec<String>>,
    pub(crate) items: Option<Vec<ContextItemInput>>,
    pub(crate) remove_commands: Option<Vec<String>>,
    pub(crate) remove_environments: Option<Vec<String>>,
    pub(crate) remove_delimiter_controls: Option<Vec<String>>,
    pub(crate) default_parse_config: Option<ParseConfigInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, expecting = "an object")]
pub(crate) struct TransformEngineOptions {
    pub(crate) packages: Option<Vec<String>>,
    pub(crate) items: Option<Vec<ContextItemInput>>,
    pub(crate) remove_commands: Option<Vec<String>>,
    pub(crate) remove_environments: Option<Vec<String>>,
    pub(crate) remove_delimiter_controls: Option<Vec<String>>,
    pub(crate) default_parse_config: Option<ParseConfigInput>,
    pub(crate) profile: Option<String>,
    pub(crate) disable_rules: Option<Vec<String>>,
}

/// Read a JS config object through the shared strict path.
///
/// Going through `serde_json::Value` first (rather than straight into `T`)
/// is what makes `deny_unknown_fields` effective: `serde_wasm_bindgen` only
/// visits known field names when targeting a struct, but visits every
/// `Object.entries` pair when targeting a map. `undefined` and `null`
/// property values both become `Null`, so an `Option` field reads as unset.
pub(crate) fn from_js<T: DeserializeOwned>(value: JsValue, what: &str) -> Result<T, JsValue> {
    let json: serde_json::Value = serde_wasm_bindgen::from_value(value)
        .map_err(|error| config_error_to_js(format!("invalid {what}: {error}")))?;
    let json = snakeize_json_keys(json, &mut Vec::new()).map_err(|(path, key)| {
        config_error_to_js(format!(
            "invalid {what}: {path}: unknown field `{key}`; keys are camelCase"
        ))
    })?;
    texform::bindings::read::<T>(json).map_err(|error| {
        config_error_to_js(texform::bindings::format_read_error(
            &error,
            what,
            texform::bindings::snake_to_camel,
        ))
    })
}

fn overlay_from_js<T, C>(
    value: Option<JsValue>,
    what: &str,
    base: C,
    overlay: impl FnOnce(T, C) -> C,
) -> Result<C, JsValue>
where
    T: DeserializeOwned,
{
    match value {
        Some(value) if !value.is_undefined() && !value.is_null() => {
            Ok(overlay(from_js(value, what)?, base))
        }
        _ => Ok(base),
    }
}

pub(crate) fn parse_config_from_js(
    value: Option<JsValue>,
    base: ParseConfig,
) -> Result<ParseConfig, JsValue> {
    overlay_from_js(value, "parse config", base, ParseConfigInput::into_config)
}

pub(crate) fn transform_config_from_js(
    value: Option<JsValue>,
    base: TransformConfig,
) -> Result<TransformConfig, JsValue> {
    overlay_from_js(
        value,
        "transform config",
        base,
        TransformConfigInput::into_config,
    )
}

pub(crate) fn normalize_config_from_js(
    value: Option<JsValue>,
    base: texform::NormalizeConfig,
) -> Result<texform::NormalizeConfig, JsValue> {
    overlay_from_js(
        value,
        "normalize config",
        base,
        NormalizeConfigInput::into_config,
    )
}

pub(crate) fn serialize_options_from_js(
    value: Option<JsValue>,
) -> Result<SerializeOptions, JsValue> {
    overlay_from_js(
        value,
        "serialize options",
        SerializeOptions::default(),
        SerializeOptionsInput::into_config,
    )
}

fn context_items(
    items: Option<Vec<ContextItemInput>>,
    what: &str,
) -> Result<Vec<ContextItem>, JsValue> {
    items
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            ContextItem::try_from(item).map_err(|message| {
                config_error_to_js(format!("invalid {what}: items[{index}]: {message}"))
            })
        })
        .collect()
}

fn parse_context_build_error_to_js(error: ParserBuildError) -> JsValue {
    config_error_to_js(error.to_string())
}

pub(crate) fn profile_from_name(name: &str) -> Result<texform::Profile, JsValue> {
    match name {
        "authoring" => Ok(texform::Profile::Authoring),
        "faithful" => Ok(texform::Profile::Faithful),
        "corpus" => Ok(texform::Profile::Corpus),
        "equiv" => Ok(texform::Profile::Equiv),
        other => Err(config_error_to_js(format!(
            "unknown transform profile: {other}"
        ))),
    }
}

pub(crate) fn parser_from_options(input: ParserOptions) -> Result<texform::Parser, JsValue> {
    let mut builder = match &input.packages {
        Some(packages) if packages.is_empty() => texform::Parser::builder().empty_knowledge(),
        Some(packages) => {
            let refs: Vec<&str> = packages.iter().map(String::as_str).collect();
            texform::Parser::builder().packages(refs.as_slice())
        }
        None => texform::Parser::builder(),
    };
    if let Some(parse) = input.default_parse_config {
        builder = builder.default_parse_config(parse.into_config(ParseConfig::LENIENT));
    }
    for item in context_items(input.items, "parser options")? {
        builder = builder.item(item);
    }
    for name in input.remove_commands.unwrap_or_default() {
        builder = builder.remove_command(name);
    }
    for name in input.remove_environments.unwrap_or_default() {
        builder = builder.remove_environment(name);
    }
    for name in input.remove_delimiter_controls.unwrap_or_default() {
        builder = builder.remove_delimiter_control(name);
    }
    builder.build().map_err(parse_context_build_error_to_js)
}

pub(crate) fn parser_from_js(args: Option<JsValue>) -> Result<texform::Parser, JsValue> {
    let input = match args {
        Some(value) if !value.is_undefined() && !value.is_null() => {
            from_js(value, "parser options")?
        }
        _ => ParserOptions::default(),
    };
    parser_from_options(input)
}

pub(crate) fn engine_from_js(args: Option<JsValue>) -> Result<texform::TransformEngine, JsValue> {
    let input = match args {
        Some(value) if !value.is_undefined() && !value.is_null() => {
            from_js(value, "transform engine options")?
        }
        _ => TransformEngineOptions::default(),
    };
    let profile = input
        .profile
        .as_deref()
        .ok_or_else(|| config_error_to_js("profile is required"))?;
    let mut builder = texform::TransformEngine::builder().profile(profile_from_name(profile)?);
    match &input.packages {
        Some(packages) if packages.is_empty() => {
            builder = builder.empty_knowledge();
        }
        Some(packages) => {
            let refs: Vec<&str> = packages.iter().map(String::as_str).collect();
            builder = builder.packages(refs.as_slice());
        }
        None => {}
    }
    if let Some(parse) = input.default_parse_config {
        builder = builder.default_parse_config(parse.into_config(ParseConfig::LENIENT));
    }
    for item in context_items(input.items, "transform engine options")? {
        builder = builder.item(item);
    }
    for name in input.remove_commands.unwrap_or_default() {
        builder = builder.remove_command(name);
    }
    for name in input.remove_environments.unwrap_or_default() {
        builder = builder.remove_environment(name);
    }
    for name in input.remove_delimiter_controls.unwrap_or_default() {
        builder = builder.remove_delimiter_control(name);
    }
    for name in input.disable_rules.unwrap_or_default() {
        builder = builder
            .disable_rule_by_name(&name)
            .map_err(|error| config_error_to_js(error.to_string()))?;
    }
    builder
        .build()
        .map_err(|error| config_error_to_js(error.to_string()))
}
