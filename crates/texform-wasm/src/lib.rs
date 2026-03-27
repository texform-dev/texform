use serde::Deserialize;
use texform_core::api::{self, ParseOutput};
use texform_core::context::{
    CommandItem, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseContext as CoreParseContext,
};
use texform_core::knowledge::{self, AllowedMode, CommandKind, CommandMeta, EnvMeta};
use texform_specs::specs::{ArgForm, ArgSpec, ContentMode, DelimiterToken, ValueKind};
use wasm_bindgen::prelude::*;

// MANUAL TypeScript type declarations for SyntaxNode and related types.
//
// These must be kept in sync with the Rust definitions in
// `texform-interface/src/syntax_node.rs`. They mirror serde's default
// externally-tagged enum representation.
//
// Why manual? tsify-next auto-generates TS declarations via
// `#[wasm_bindgen(typescript_custom_section)]`, but wasm-lld's dead-code
// elimination drops those sections from dependency crates (texform-interface)
// because no exported `#[wasm_bindgen]` function directly references their
// WasmDescribe impls. Only types from the cdylib crate itself (or types
// with `into_wasm_abi` used in an exported function signature) survive.
//
// If you modify SyntaxNode or its sub-types, update the definitions below.
#[wasm_bindgen(typescript_custom_section)]
const SYNTAX_NODE_TYPES: &str = r#"
export type SyntaxNode =
    | { Group: { mode: ContentMode; kind: GroupKind; children: SyntaxNode[] } }
    | { Command: { name: string; args: ArgumentSlot[] } }
    | { Infix: { name: string; args: ArgumentSlot[]; left: SyntaxNode; right: SyntaxNode } }
    | { Declarative: { name: string; args: ArgumentSlot[]; scope: SyntaxNode } }
    | { Environment: { name: string; args: ArgumentSlot[]; body: SyntaxNode } }
    | { Scripted: { base: SyntaxNode; subscript?: SyntaxNode; superscript?: SyntaxNode } }
    | { UnknownCommand: { name: string } }
    | { Text: string }
    | { Char: string }
    | "ActiveSpace";

export type ContentMode = "Math" | "Text";

export type GroupKind =
    | "Explicit"
    | "Implicit"
    | { Delimited: { left: Delimiter; right: Delimiter } }
    | "InlineMath";

export type Delimiter =
    | "None"
    | { Char: string }
    | { Control: string };

export type Argument = {
    kind: ArgumentKind;
    value: ArgumentValue;
};

export type ArgumentSlot = Argument | null;

export type ArgumentKind =
    | "Mandatory"
    | "Optional"
    | "Star"
    | "Group"
    | { Delimited: { open: Delimiter; close: Delimiter } }
    | { Paired: { open: Delimiter; close: Delimiter } };

export type ArgumentValue =
    | { Content: SyntaxNode }
    | { Delimiter: Delimiter }
    | { CSName: string }
    | { Dimension: string }
    | { Integer: string }
    | { KeyVal: string }
    | { Column: string }
    | { Boolean: boolean };

export type ArgSpecInfo = {
    required: boolean;
    no_leading_space: boolean;
    nullable: boolean;
    kind: unknown;
    form: unknown;
};

export type CommandInfo = {
    name: string;
    kind: "prefix" | "infix" | "declarative";
    allowed_mode: "math" | "text" | "both";
    spec_string: string;
    package: string;
    tags: string[];
    args: ArgSpecInfo[];
};

export type EnvInfo = {
    name: string;
    allowed_mode: "math" | "text" | "both";
    body_mode: "math" | "text";
    spec_string: string;
    package: string;
    tags: string[];
    args: ArgSpecInfo[];
};

export type ContextItem =
    | {
          target: "command";
          name: string;
          kind: "prefix" | "infix" | "declarative";
          allowed_mode: "math" | "text" | "both";
          spec: string;
          tags?: string[];
      }
    | {
          target: "environment";
          name: string;
          allowed_mode: "math" | "text" | "both";
          body_mode: "math" | "text";
          spec: string;
          tags?: string[];
      }
    | {
          target: "delimiter";
          name: string;
      };
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "target", rename_all = "lowercase")]
enum ContextItemInput {
    Command {
        name: String,
        kind: String,
        allowed_mode: String,
        spec: String,
        tags: Option<Vec<String>>,
    },
    Environment {
        name: String,
        allowed_mode: String,
        body_mode: String,
        spec: String,
        tags: Option<Vec<String>>,
    },
    Delimiter {
        name: String,
    },
}

#[wasm_bindgen]
pub struct ParseContext {
    inner: CoreParseContext,
}

#[wasm_bindgen]
impl ParseContext {
    #[wasm_bindgen(constructor)]
    pub fn new(packages: Option<Vec<String>>) -> Result<ParseContext, JsValue> {
        let inner = match packages {
            Some(pkgs) if !pkgs.is_empty() => {
                let refs: Vec<&str> = pkgs.iter().map(String::as_str).collect();
                CoreParseContext::try_from_packages(refs.as_slice()).map_err(|error| {
                    JsValue::from_str(&format!("package loading failed: {}", error))
                })?
            }
            _ => CoreParseContext::clone_runtime_default(),
        };
        Ok(ParseContext { inner })
    }

    pub fn insert_item(&mut self, item: JsValue) -> Result<(), JsValue> {
        let item: ContextItemInput = serde_wasm_bindgen::from_value(item)
            .map_err(|error| JsValue::from_str(&format!("invalid context item: {}", error)))?;
        self.inner
            .insert_item(parse_context_item_input(item)?)
            .map_err(|error| JsValue::from_str(&format!("spec validation failed: {}", error)))
    }

    pub fn insert_items(&mut self, items: JsValue) -> Result<(), JsValue> {
        let items: Vec<ContextItemInput> = serde_wasm_bindgen::from_value(items)
            .map_err(|error| JsValue::from_str(&format!("invalid context items: {}", error)))?;
        let core_items = items
            .into_iter()
            .map(parse_context_item_input)
            .collect::<Result<Vec<_>, _>>()?;
        self.inner
            .insert_items(core_items)
            .map_err(|error| JsValue::from_str(&format!("spec validation failed: {}", error)))
    }

    pub fn remove_item(&mut self, item: JsValue) -> Result<bool, JsValue> {
        let item: ContextItemInput = serde_wasm_bindgen::from_value(item)
            .map_err(|error| JsValue::from_str(&format!("invalid context item: {}", error)))?;
        Ok(self.inner.remove_item(parse_context_item_input(item)?))
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    pub fn parse(&self, src: &str, strict: Option<bool>) -> Result<JsValue, JsValue> {
        let strict = strict.unwrap_or(false);
        parse_output_to_result(self.inner.parse(src, strict))
    }

    pub fn lookup_command(&self, name: &str) -> JsValue {
        match self.inner.lookup_command(name) {
            Some(meta) => command_meta_to_js(meta),
            None => JsValue::NULL,
        }
    }

    pub fn lookup_env(&self, name: &str) -> JsValue {
        match self.inner.lookup_env(name) {
            Some(meta) => env_meta_to_js(meta),
            None => JsValue::NULL,
        }
    }
}

/// Parse a LaTeX formula.
///
/// Returns a JS object with `node` and `span` on success.
/// Throws an error object with `diagnostics` and `partial_result` when
/// diagnostics are present.
#[wasm_bindgen]
pub fn parse(src: &str, strict: Option<bool>) -> Result<JsValue, JsValue> {
    let strict = strict.unwrap_or(false);
    parse_output_to_result(api::parse_latex(src, strict))
}

/// Test one or more context items by injecting them and parsing one or more inputs.
///
/// Supported targets are command, environment, and delimiter control.
#[wasm_bindgen]
pub fn parse_with_context_items(
    items: JsValue,
    inputs: Vec<String>,
    packages: Option<Vec<String>>,
    strict: Option<bool>,
) -> Result<JsValue, JsValue> {
    let strict = strict.unwrap_or(false);
    let items: Vec<ContextItemInput> = serde_wasm_bindgen::from_value(items)
        .map_err(|error| JsValue::from_str(&format!("invalid context items: {}", error)))?;

    let input_refs: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let package_refs = packages
        .as_ref()
        .map(|values| values.iter().map(String::as_str).collect::<Vec<_>>());

    let core_items: Vec<ContextItem> = items
        .into_iter()
        .map(parse_context_item_input)
        .collect::<Result<_, _>>()?;

    let output = api::parse_with_context_items(
        &core_items,
        input_refs.as_slice(),
        package_refs.as_deref(),
        strict,
    );
    parse_with_context_output_to_result(output)
}

fn parse_context_item_input(input: ContextItemInput) -> Result<ContextItem, JsValue> {
    match input {
        ContextItemInput::Command {
            name,
            kind,
            allowed_mode,
            spec,
            tags,
        } => {
            let kind = parse_command_kind(kind.as_str())?;
            let allowed_mode = parse_allowed_mode(allowed_mode.as_str())?;
            Ok(CommandItem::new(name, kind, allowed_mode, spec)
                .with_tags(tags.unwrap_or_default())
                .into())
        }
        ContextItemInput::Environment {
            name,
            allowed_mode,
            body_mode,
            spec,
            tags,
        } => {
            let allowed_mode = parse_allowed_mode(allowed_mode.as_str())?;
            let body_mode = parse_content_mode(body_mode.as_str())?;
            Ok(EnvironmentItem::new(name, allowed_mode, body_mode, spec)
                .with_tags(tags.unwrap_or_default())
                .into())
        }
        ContextItemInput::Delimiter { name } => Ok(DelimiterControlItem::new(name).into()),
    }
}

#[wasm_bindgen]
pub fn lookup_command_info(name: &str) -> JsValue {
    match knowledge::lookup_command(name) {
        Some(meta) => command_meta_to_js(meta),
        None => JsValue::NULL,
    }
}

#[wasm_bindgen]
pub fn lookup_env_info(name: &str) -> JsValue {
    match knowledge::lookup_env(name) {
        Some(meta) => env_meta_to_js(meta),
        None => JsValue::NULL,
    }
}

#[wasm_bindgen]
pub fn validate_spec(spec: &str) -> JsValue {
    let value = js_sys::Object::new();

    match texform_specs::specs::parse_arg_specs(spec, "validate_spec") {
        Ok(args) => {
            let parsed = js_sys::Array::new();
            for arg in &args {
                parsed.push(&arg_spec_to_js(arg));
            }
            js_sys::Reflect::set(&value, &"valid".into(), &JsValue::TRUE).unwrap();
            js_sys::Reflect::set(
                &value,
                &"arg_count".into(),
                &JsValue::from_f64(args.len() as f64),
            )
            .unwrap();
            js_sys::Reflect::set(&value, &"parsed".into(), &parsed.into()).unwrap();
            js_sys::Reflect::set(&value, &"error".into(), &JsValue::NULL).unwrap();
        }
        Err(error) => {
            js_sys::Reflect::set(&value, &"valid".into(), &JsValue::FALSE).unwrap();
            js_sys::Reflect::set(&value, &"error".into(), &error.to_string().into()).unwrap();
            js_sys::Reflect::set(&value, &"parsed".into(), &JsValue::NULL).unwrap();
        }
    }

    value.into()
}

fn parse_with_context_output_to_result(
    output: api::ParseWithContextOutput,
) -> Result<JsValue, JsValue> {
    let results = js_sys::Array::new();
    for item in &output {
        results.push(&parse_output_to_batch_entry(&item.input, &item.output)?);
    }
    Ok(results.into())
}

fn parse_output_to_batch_entry(input: &str, output: &ParseOutput) -> Result<JsValue, JsValue> {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"input".into(), &input.into()).unwrap();

    if output.diagnostics.is_empty() {
        match &output.result {
            Some(result) => {
                let display = result.node.to_string();
                let js = serde_wasm_bindgen::to_value(&result)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                js_sys::Reflect::set(&js, &"display".into(), &display.clone().into()).unwrap();
                let diagnostics = js_sys::Array::new();

                js_sys::Reflect::set(&value, &"success".into(), &JsValue::TRUE).unwrap();
                js_sys::Reflect::set(&value, &"result".into(), &js).unwrap();
                js_sys::Reflect::set(&value, &"display".into(), &display.into()).unwrap();
                js_sys::Reflect::set(&value, &"diagnostics".into(), &diagnostics.into()).unwrap();
                js_sys::Reflect::set(&value, &"partial_result".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(&value, &"partial_display".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(&value, &"error".into(), &JsValue::NULL).unwrap();
                Ok(value.into())
            }
            None => {
                let diagnostics = js_sys::Array::new();
                js_sys::Reflect::set(&value, &"success".into(), &JsValue::FALSE).unwrap();
                js_sys::Reflect::set(&value, &"result".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(&value, &"display".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(&value, &"diagnostics".into(), &diagnostics.into()).unwrap();
                js_sys::Reflect::set(&value, &"partial_result".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(&value, &"partial_display".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(
                    &value,
                    &"error".into(),
                    &JsValue::from_str("parse produced no output and no diagnostics"),
                )
                .unwrap();
                Ok(value.into())
            }
        }
    } else {
        let diagnostics = serde_wasm_bindgen::to_value(&output.diagnostics)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let (partial_result, partial_display) = match &output.result {
            Some(result) => {
                let js = serde_wasm_bindgen::to_value(result)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                (js, result.node.to_string().into())
            }
            None => (JsValue::NULL, JsValue::NULL),
        };

        let message = output
            .diagnostics
            .first()
            .map(|diag| diag.message.clone())
            .unwrap_or_else(|| "parse failed".to_string());

        js_sys::Reflect::set(&value, &"success".into(), &JsValue::FALSE).unwrap();
        js_sys::Reflect::set(&value, &"result".into(), &JsValue::NULL).unwrap();
        js_sys::Reflect::set(&value, &"display".into(), &JsValue::NULL).unwrap();
        js_sys::Reflect::set(&value, &"diagnostics".into(), &diagnostics).unwrap();
        js_sys::Reflect::set(&value, &"partial_result".into(), &partial_result).unwrap();
        js_sys::Reflect::set(&value, &"partial_display".into(), &partial_display).unwrap();
        js_sys::Reflect::set(&value, &"error".into(), &message.into()).unwrap();

        Ok(value.into())
    }
}

fn parse_output_to_result(output: ParseOutput) -> Result<JsValue, JsValue> {
    if output.diagnostics.is_empty() {
        match output.result {
            Some(result) => {
                let display = result.node.to_string();
                let js = serde_wasm_bindgen::to_value(&result)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                js_sys::Reflect::set(&js, &"display".into(), &display.into()).unwrap();
                Ok(js)
            }
            None => Err(JsValue::from_str(
                "parse produced no output and no diagnostics",
            )),
        }
    } else {
        // Build structured error: { diagnostics, partial_result, partial_display }
        let diagnostics = serde_wasm_bindgen::to_value(&output.diagnostics)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let (partial_result, partial_display) = match &output.result {
            Some(r) => {
                let js = serde_wasm_bindgen::to_value(r)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                let display: JsValue = r.node.to_string().into();
                (js, display)
            }
            None => (JsValue::NULL, JsValue::NULL),
        };

        let err = js_sys::Object::new();
        let message = output
            .diagnostics
            .first()
            .map(|diag| diag.message.clone())
            .unwrap_or_else(|| "parse failed".to_string());
        js_sys::Reflect::set(&err, &"diagnostics".into(), &diagnostics).unwrap();
        js_sys::Reflect::set(&err, &"partial_result".into(), &partial_result).unwrap();
        js_sys::Reflect::set(&err, &"partial_display".into(), &partial_display).unwrap();
        js_sys::Reflect::set(&err, &"message".into(), &message.into()).unwrap();

        Err(err.into())
    }
}

fn parse_command_kind(value: &str) -> Result<CommandKind, JsValue> {
    match value {
        "prefix" => Ok(CommandKind::Prefix),
        "infix" => Ok(CommandKind::Infix),
        "declarative" => Ok(CommandKind::Declarative),
        _ => Err(JsValue::from_str(&format!(
            "unsupported command kind: {}",
            value
        ))),
    }
}

fn command_kind_to_string(kind: CommandKind) -> &'static str {
    match kind {
        CommandKind::Prefix => "prefix",
        CommandKind::Infix => "infix",
        CommandKind::Declarative => "declarative",
    }
}

fn parse_allowed_mode(value: &str) -> Result<AllowedMode, JsValue> {
    match value {
        "math" => Ok(AllowedMode::Math),
        "text" => Ok(AllowedMode::Text),
        "both" => Ok(AllowedMode::Both),
        _ => Err(JsValue::from_str(&format!(
            "unsupported allowed mode: {}",
            value
        ))),
    }
}

fn parse_content_mode(value: &str) -> Result<ContentMode, JsValue> {
    match value {
        "math" => Ok(ContentMode::Math),
        "text" => Ok(ContentMode::Text),
        _ => Err(JsValue::from_str(&format!(
            "unsupported content mode: {}",
            value
        ))),
    }
}

fn content_mode_to_string(mode: ContentMode) -> &'static str {
    match mode {
        ContentMode::Math => "math",
        ContentMode::Text => "text",
    }
}

fn command_meta_to_js(meta: &CommandMeta) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"name".into(), &meta.name.into()).unwrap();
    js_sys::Reflect::set(
        &value,
        &"kind".into(),
        &command_kind_to_string(meta.kind).into(),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"allowed_mode".into(),
        &meta.allowed_mode.as_str().into(),
    )
    .unwrap();
    js_sys::Reflect::set(&value, &"spec_string".into(), &meta.spec_string.into()).unwrap();
    js_sys::Reflect::set(&value, &"package".into(), &meta.package.into()).unwrap();

    let tags = js_sys::Array::new();
    for &tag in meta.tags {
        tags.push(&tag.into());
    }
    js_sys::Reflect::set(&value, &"tags".into(), &tags.into()).unwrap();

    let args = js_sys::Array::new();
    for spec in meta.args {
        args.push(&arg_spec_to_js(spec));
    }
    js_sys::Reflect::set(&value, &"args".into(), &args.into()).unwrap();

    value.into()
}

fn env_meta_to_js(meta: &EnvMeta) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"name".into(), &meta.name.into()).unwrap();
    js_sys::Reflect::set(
        &value,
        &"allowed_mode".into(),
        &meta.allowed_mode.as_str().into(),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"body_mode".into(),
        &content_mode_to_string(meta.body_mode).into(),
    )
    .unwrap();
    js_sys::Reflect::set(&value, &"spec_string".into(), &meta.spec_string.into()).unwrap();
    js_sys::Reflect::set(&value, &"package".into(), &meta.package.into()).unwrap();

    let tags = js_sys::Array::new();
    for &tag in meta.tags {
        tags.push(&tag.into());
    }
    js_sys::Reflect::set(&value, &"tags".into(), &tags.into()).unwrap();

    let args = js_sys::Array::new();
    for spec in meta.args {
        args.push(&arg_spec_to_js(spec));
    }
    js_sys::Reflect::set(&value, &"args".into(), &args.into()).unwrap();

    value.into()
}

fn arg_spec_to_js(spec: &ArgSpec) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(
        &value,
        &"required".into(),
        &JsValue::from_bool(spec.required),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"no_leading_space".into(),
        &JsValue::from_bool(spec.no_leading_space),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"nullable".into(),
        &JsValue::from_bool(spec.nullable),
    )
    .unwrap();
    js_sys::Reflect::set(&value, &"kind".into(), &value_kind_to_js(&spec.kind)).unwrap();
    js_sys::Reflect::set(&value, &"form".into(), &arg_form_to_js(&spec.form)).unwrap();
    value.into()
}

fn value_kind_to_js(kind: &ValueKind) -> JsValue {
    let value = js_sys::Object::new();
    match kind {
        ValueKind::Content { mode } => {
            js_sys::Reflect::set(&value, &"type".into(), &"content".into()).unwrap();
            let mode_str = match mode {
                ContentMode::Math => "math",
                ContentMode::Text => "text",
            };
            js_sys::Reflect::set(&value, &"mode".into(), &mode_str.into()).unwrap();
        }
        ValueKind::Delimiter => {
            js_sys::Reflect::set(&value, &"type".into(), &"delimiter".into()).unwrap();
        }
        ValueKind::CSName => {
            js_sys::Reflect::set(&value, &"type".into(), &"csname".into()).unwrap();
        }
        ValueKind::Dimension => {
            js_sys::Reflect::set(&value, &"type".into(), &"dimension".into()).unwrap();
        }
        ValueKind::Integer => {
            js_sys::Reflect::set(&value, &"type".into(), &"integer".into()).unwrap();
        }
        ValueKind::KeyVal => {
            js_sys::Reflect::set(&value, &"type".into(), &"keyval".into()).unwrap();
        }
        ValueKind::Column => {
            js_sys::Reflect::set(&value, &"type".into(), &"column".into()).unwrap();
        }
        ValueKind::Star => {
            js_sys::Reflect::set(&value, &"type".into(), &"star".into()).unwrap();
        }
    }
    value.into()
}

fn arg_form_to_js(form: &ArgForm) -> JsValue {
    let value = js_sys::Object::new();
    match form {
        ArgForm::Standard => {
            js_sys::Reflect::set(&value, &"type".into(), &"standard".into()).unwrap();
        }
        ArgForm::Star => {
            js_sys::Reflect::set(&value, &"type".into(), &"star".into()).unwrap();
        }
        ArgForm::Group => {
            js_sys::Reflect::set(&value, &"type".into(), &"group".into()).unwrap();
        }
        ArgForm::Delimited { open, close } => {
            js_sys::Reflect::set(&value, &"type".into(), &"delimited".into()).unwrap();
            js_sys::Reflect::set(&value, &"open".into(), &delimiter_token_to_js(open)).unwrap();
            js_sys::Reflect::set(&value, &"close".into(), &delimiter_token_to_js(close)).unwrap();
        }
        ArgForm::Paired { pairs } => {
            js_sys::Reflect::set(&value, &"type".into(), &"paired".into()).unwrap();
            let pairs_value = js_sys::Array::new();
            for (open, close) in pairs.iter() {
                let pair = js_sys::Object::new();
                js_sys::Reflect::set(&pair, &"open".into(), &delimiter_token_to_js(open)).unwrap();
                js_sys::Reflect::set(&pair, &"close".into(), &delimiter_token_to_js(close))
                    .unwrap();
                pairs_value.push(&pair.into());
            }
            js_sys::Reflect::set(&value, &"pairs".into(), &pairs_value.into()).unwrap();
        }
    }
    value.into()
}

fn delimiter_token_to_js(token: &DelimiterToken) -> JsValue {
    let value = js_sys::Object::new();
    match token {
        DelimiterToken::Char(ch) => {
            js_sys::Reflect::set(&value, &"type".into(), &"char".into()).unwrap();
            js_sys::Reflect::set(&value, &"value".into(), &ch.to_string().into()).unwrap();
        }
        DelimiterToken::ControlSeq(name) => {
            js_sys::Reflect::set(&value, &"type".into(), &"control-seq".into()).unwrap();
            js_sys::Reflect::set(&value, &"value".into(), &name.as_ref().into()).unwrap();
        }
    }
    value.into()
}
