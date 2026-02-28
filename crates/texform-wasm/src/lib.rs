use texform_core::api::{self, ParseResult};
use texform_specs::specs::{ArgForm, ArgSpec, DelimiterToken, ValueKind};
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
    | { Command: { name: string; starred: boolean; args: ArgumentSlot[] } }
    | { Infix: { name: string; starred: boolean; args: ArgumentSlot[]; left: SyntaxNode; right: SyntaxNode } }
    | { Declarative: { name: string; starred: boolean; args: ArgumentSlot[]; scope: SyntaxNode } }
    | { Environment: { name: string; starred: boolean; args: ArgumentSlot[]; body: SyntaxNode } }
    | { Scripted: { base: SyntaxNode; subscript?: SyntaxNode; superscript?: SyntaxNode } }
    | { UnknownCommand: { name: string; starred: boolean } }
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
    | { Dimension: string }
    | { Integer: string }
    | { KeyVal: string }
    | { Column: string }
    | { Boolean: boolean };
"#;

/// Parse a LaTeX formula.
///
/// Returns a JS object with `node` and `span` on success.
/// Throws an error object with `diagnostics` and `partial_result` when
/// diagnostics are present.
#[wasm_bindgen]
pub fn parse(src: &str, strict: Option<bool>) -> Result<ParseResult, JsValue> {
    let strict = strict.unwrap_or(false);
    let output = api::parse_latex(src, strict);

    if output.diagnostics.is_empty() {
        match output.result {
            Some(result) => Ok(result),
            None => Err(JsValue::from_str(
                "parse produced no output and no diagnostics",
            )),
        }
    } else {
        // Build structured error: { diagnostics, partial_result }
        let diagnostics = serde_wasm_bindgen::to_value(&output.diagnostics)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let partial_result = match &output.result {
            Some(r) => {
                serde_wasm_bindgen::to_value(r).map_err(|e| JsValue::from_str(&e.to_string()))?
            }
            None => JsValue::NULL,
        };

        let err = js_sys::Object::new();
        js_sys::Reflect::set(&err, &"diagnostics".into(), &diagnostics).unwrap();
        js_sys::Reflect::set(&err, &"partial_result".into(), &partial_result).unwrap();

        Err(err.into())
    }
}

#[wasm_bindgen]
pub fn validate_spec(spec: &str) -> JsValue {
    let result = std::panic::catch_unwind(|| texform_specs::specs::parse_arg_specs(spec, "validate_spec"));
    let value = js_sys::Object::new();

    match result {
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
        Err(payload) => {
            js_sys::Reflect::set(&value, &"valid".into(), &JsValue::FALSE).unwrap();
            js_sys::Reflect::set(&value, &"error".into(), &panic_payload_to_string(payload).into()).unwrap();
            js_sys::Reflect::set(&value, &"parsed".into(), &JsValue::NULL).unwrap();
        }
    }

    value.into()
}

fn arg_spec_to_js(spec: &ArgSpec) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"required".into(), &JsValue::from_bool(spec.required)).unwrap();
    js_sys::Reflect::set(
        &value,
        &"no_leading_space".into(),
        &JsValue::from_bool(spec.no_leading_space),
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
                texform_specs::specs::ContentMode::Math => "math",
                texform_specs::specs::ContentMode::Text => "text",
            };
            js_sys::Reflect::set(&value, &"mode".into(), &mode_str.into()).unwrap();
        }
        ValueKind::Delimiter => {
            js_sys::Reflect::set(&value, &"type".into(), &"delimiter".into()).unwrap();
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
                js_sys::Reflect::set(&pair, &"close".into(), &delimiter_token_to_js(close)).unwrap();
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

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "validate_spec panic".to_string()
}
