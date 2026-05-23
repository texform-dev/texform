#[derive(Debug, serde::Serialize)]
pub struct ValidateArgspecResult {
    pub ok: bool,
    pub error: Option<String>,
}

pub fn validate_argspec(spec: &str) -> ValidateArgspecResult {
    match texform_argspec::parse_arg_specs(spec, "validate_argspec") {
        Ok(_) => ValidateArgspecResult {
            ok: true,
            error: None,
        },
        Err(error) => ValidateArgspecResult {
            ok: false,
            error: Some(error.to_string()),
        },
    }
}
