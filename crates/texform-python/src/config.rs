//! Complete-config classes and the Python → `serde_json::Value` reader.
//!
//! Keyword overrides and nested dicts go through [`py_to_json`] so bools are
//! exact Python `bool`s (never truthy strings) and only `dict` / `list` /
//! `tuple` containers are accepted. Shared [`texform::bindings::read`] then
//! applies `deny_unknown_fields` and the objects-only rule.

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple, PyType};
use serde::de::DeserializeOwned;
use texform::bindings::{
    ContextItemInput, NormalizeConfigInput, ParseConfigInput, SerializeOptionsInput,
    TransformConfigInput,
};

use crate::ConfigError;

fn py_bool_lit(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

fn format_json_path(path: &[String]) -> String {
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

fn invalid_at(what: &str, path: &[String], message: impl std::fmt::Display) -> PyErr {
    if path.is_empty() {
        ConfigError::new_err(format!("invalid {what}: {message}"))
    } else {
        ConfigError::new_err(format!(
            "invalid {what}: {}: {message}",
            format_json_path(path)
        ))
    }
}

fn python_type_name(value: &Bound<'_, PyAny>) -> String {
    match value.get_type().name() {
        Ok(name) => name.to_string(),
        Err(_) => "unknown".to_string(),
    }
}

/// Walk a Python value into JSON using exact host types, recording `path` for errors.
pub(crate) fn py_to_json(
    value: &Bound<'_, PyAny>,
    what: &str,
    path: &mut Vec<String>,
) -> PyResult<serde_json::Value> {
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }
    // `bool` is an `int` subclass; it must win so `True` is never a number.
    if let Ok(boolean) = value.cast::<PyBool>() {
        return Ok(serde_json::Value::Bool(boolean.is_true()));
    }
    if let Ok(int) = value.cast::<PyInt>() {
        if let Ok(number) = int.extract::<i64>() {
            return Ok(serde_json::Value::Number(number.into()));
        }
        if let Ok(number) = int.extract::<u64>() {
            return Ok(serde_json::Value::Number(number.into()));
        }
        return Err(invalid_at(what, path, "integer out of range"));
    }
    if let Ok(float) = value.cast::<PyFloat>() {
        let number = float.extract::<f64>()?;
        return match serde_json::Number::from_f64(number) {
            Some(number) => Ok(serde_json::Value::Number(number)),
            None => Err(invalid_at(what, path, "non-finite float")),
        };
    }
    if let Ok(string) = value.cast::<PyString>() {
        return Ok(serde_json::Value::String(
            string.to_string_lossy().into_owned(),
        ));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut object = serde_json::Map::new();
        for (key, child) in dict.iter() {
            let Ok(key) = key.cast::<PyString>() else {
                return Err(invalid_at(what, path, "dict keys must be strings"));
            };
            let key = key.to_string_lossy().into_owned();
            path.push(key.clone());
            object.insert(key, py_to_json(&child, what, path)?);
            path.pop();
        }
        return Ok(serde_json::Value::Object(object));
    }
    if let Ok(list) = value.cast::<PyList>() {
        return py_sequence_to_json(list.iter(), list.len(), what, path);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return py_sequence_to_json(tuple.iter(), tuple.len(), what, path);
    }
    Err(invalid_at(
        what,
        path,
        format!(
            "unsupported value of type `{}`; pass a dict",
            python_type_name(value)
        ),
    ))
}

fn py_sequence_to_json<'py>(
    items: impl Iterator<Item = Bound<'py, PyAny>>,
    len: usize,
    what: &str,
    path: &mut Vec<String>,
) -> PyResult<serde_json::Value> {
    let mut array = Vec::with_capacity(len);
    for (index, child) in items.enumerate() {
        path.push(format!("[{index}]"));
        array.push(py_to_json(&child, what, path)?);
        path.pop();
    }
    Ok(serde_json::Value::Array(array))
}

pub(crate) fn from_python<T: DeserializeOwned>(
    value: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<T> {
    let json = py_to_json(value, what, &mut Vec::new())?;
    texform::bindings::read::<T>(json).map_err(|error| {
        ConfigError::new_err(texform::bindings::format_read_error(&error, what, |key| {
            key.to_owned()
        }))
    })
}

fn complete_transform_config(value: &Bound<'_, PyAny>) -> PyResult<texform::TransformConfig> {
    let complete = value.extract::<PyRef<'_, PyTransformConfig>>().map_err(|_| {
        ConfigError::new_err(
            "config must be a TransformConfig; pass overrides as keyword arguments (use **overrides for a dict)",
        )
    })?;
    Ok(complete.to_core(value.py()))
}

pub(crate) fn parse_config_from_python(
    config: Option<&Bound<'_, PyAny>>,
    overrides: Option<&Bound<'_, PyDict>>,
    base: texform::ParseConfig,
) -> PyResult<texform::ParseConfig> {
    let mut current = base;
    if let Some(value) = config
        && !value.is_none()
    {
        let complete = value.extract::<PyRef<'_, PyParseConfig>>().map_err(|_| {
            ConfigError::new_err(
                "config must be a ParseConfig; pass overrides as keyword arguments (use **overrides for a dict)",
            )
        })?;
        current = complete.to_core();
    }
    if let Some(overrides) = overrides
        && !overrides.is_empty()
    {
        current = from_python::<ParseConfigInput>(overrides.as_any(), "parse overrides")?
            .into_config(current);
    }
    Ok(current)
}

pub(crate) fn transform_config_from_python(
    config: Option<&Bound<'_, PyAny>>,
    overrides: Option<&Bound<'_, PyDict>>,
    base: texform::TransformConfig,
) -> PyResult<texform::TransformConfig> {
    let mut current = base;
    if let Some(value) = config
        && !value.is_none()
    {
        current = complete_transform_config(value)?;
    }
    if let Some(overrides) = overrides
        && !overrides.is_empty()
    {
        current = from_python::<TransformConfigInput>(overrides.as_any(), "transform overrides")?
            .into_config(current);
    }
    Ok(current)
}

pub(crate) fn normalize_config_from_python(
    config: Option<&Bound<'_, PyAny>>,
    overrides: Option<&Bound<'_, PyDict>>,
    mut base: texform::NormalizeConfig,
) -> PyResult<texform::NormalizeConfig> {
    if let Some(value) = config
        && !value.is_none()
    {
        base.transform = complete_transform_config(value)?;
    }
    if let Some(overrides) = overrides
        && !overrides.is_empty()
    {
        base = from_python::<NormalizeConfigInput>(overrides.as_any(), "normalize overrides")?
            .into_config(base);
    }
    Ok(base)
}

pub(crate) fn serialize_options_from_python(
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<texform::SerializeOptions> {
    match options {
        Some(options) if !options.is_empty() => Ok(from_python::<SerializeOptionsInput>(
            options.as_any(),
            "serialize options",
        )?
        .into_config(texform::SerializeOptions::default())),
        _ => Ok(texform::SerializeOptions::default()),
    }
}

pub(crate) fn context_items_from_python(
    items: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<texform::ContextItem>> {
    let Some(items) = items else {
        return Ok(Vec::new());
    };
    if items.is_none() {
        return Ok(Vec::new());
    }
    let inputs = from_python::<Vec<ContextItemInput>>(items, "items")?;
    inputs
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            texform::ContextItem::try_from(item).map_err(|message| {
                ConfigError::new_err(format!("invalid items: [{index}]: {message}"))
            })
        })
        .collect()
}

#[pyclass(name = "ParseConfig", eq)]
#[derive(PartialEq, Eq)]
pub(crate) struct PyParseConfig {
    #[pyo3(get, set)]
    reject_unknown: bool,
    #[pyo3(get, set)]
    abort_on_error: bool,
    #[pyo3(get, set)]
    max_group_depth: usize,
}

#[pymethods]
impl PyParseConfig {
    #[new]
    #[pyo3(signature = (reject_unknown = false, abort_on_error = false, max_group_depth = 128))]
    fn new(reject_unknown: bool, abort_on_error: bool, max_group_depth: usize) -> Self {
        Self {
            reject_unknown,
            abort_on_error,
            max_group_depth,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ParseConfig(reject_unknown={}, abort_on_error={}, max_group_depth={})",
            py_bool_lit(self.reject_unknown),
            py_bool_lit(self.abort_on_error),
            self.max_group_depth
        )
    }
}

impl PyParseConfig {
    pub(crate) fn from_core(config: texform::ParseConfig) -> Self {
        Self {
            reject_unknown: config.reject_unknown,
            abort_on_error: config.abort_on_error,
            max_group_depth: config.max_group_depth,
        }
    }

    pub(crate) fn to_core(&self) -> texform::ParseConfig {
        texform::ParseConfig {
            reject_unknown: self.reject_unknown,
            abort_on_error: self.abort_on_error,
            max_group_depth: self.max_group_depth,
        }
    }
}

#[pyclass(name = "LowerAttributesConfig", eq)]
#[derive(PartialEq, Eq)]
pub(crate) struct PyLowerAttributesConfig {
    #[pyo3(get, set)]
    enabled: bool,
}

#[pymethods]
impl PyLowerAttributesConfig {
    #[new]
    #[pyo3(signature = (enabled = true))]
    fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn __repr__(&self) -> String {
        format!(
            "LowerAttributesConfig(enabled={})",
            py_bool_lit(self.enabled)
        )
    }
}

impl PyLowerAttributesConfig {
    pub(crate) fn from_core(config: texform::LowerAttributesConfig) -> Self {
        Self {
            enabled: config.enabled,
        }
    }

    pub(crate) fn to_core(&self) -> texform::LowerAttributesConfig {
        texform::LowerAttributesConfig {
            enabled: self.enabled,
        }
    }
}

#[pyclass(name = "RewriteConfig", eq)]
#[derive(PartialEq, Eq)]
pub(crate) struct PyRewriteConfig {
    #[pyo3(get, set)]
    enabled: bool,
    #[pyo3(get, set)]
    max_iterations: usize,
}

#[pymethods]
impl PyRewriteConfig {
    #[new]
    #[pyo3(signature = (enabled = true, max_iterations = 100))]
    fn new(enabled: bool, max_iterations: usize) -> Self {
        Self {
            enabled,
            max_iterations,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "RewriteConfig(enabled={}, max_iterations={})",
            py_bool_lit(self.enabled),
            self.max_iterations
        )
    }
}

impl PyRewriteConfig {
    pub(crate) fn from_core(config: texform::RewriteConfig) -> Self {
        Self {
            enabled: config.enabled,
            max_iterations: config.max_iterations,
        }
    }

    pub(crate) fn to_core(&self) -> texform::RewriteConfig {
        texform::RewriteConfig {
            enabled: self.enabled,
            max_iterations: self.max_iterations,
        }
    }
}

#[pyclass(name = "FinalizeAstConfig", eq)]
#[derive(PartialEq, Eq)]
pub(crate) struct PyFinalizeAstConfig {
    #[pyo3(get, set)]
    enabled: bool,
}

#[pymethods]
impl PyFinalizeAstConfig {
    #[new]
    #[pyo3(signature = (enabled = true))]
    fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn __repr__(&self) -> String {
        format!("FinalizeAstConfig(enabled={})", py_bool_lit(self.enabled))
    }
}

impl PyFinalizeAstConfig {
    pub(crate) fn from_core(config: texform::FinalizeAstConfig) -> Self {
        Self {
            enabled: config.enabled,
        }
    }

    pub(crate) fn to_core(&self) -> texform::FinalizeAstConfig {
        texform::FinalizeAstConfig {
            enabled: self.enabled,
        }
    }
}

#[pyclass(name = "FlattenGroupsConfig", eq)]
#[derive(PartialEq, Eq)]
pub(crate) struct PyFlattenGroupsConfig {
    #[pyo3(get, set)]
    enabled: bool,
    #[pyo3(get, set)]
    preserve_rendered_spacing: bool,
}

#[pymethods]
impl PyFlattenGroupsConfig {
    #[new]
    #[pyo3(signature = (enabled = true, preserve_rendered_spacing = true))]
    fn new(enabled: bool, preserve_rendered_spacing: bool) -> Self {
        Self {
            enabled,
            preserve_rendered_spacing,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "FlattenGroupsConfig(enabled={}, preserve_rendered_spacing={})",
            py_bool_lit(self.enabled),
            py_bool_lit(self.preserve_rendered_spacing)
        )
    }
}

impl PyFlattenGroupsConfig {
    pub(crate) fn from_core(config: texform::FlattenGroupsConfig) -> Self {
        Self {
            enabled: config.enabled,
            preserve_rendered_spacing: config.preserve_rendered_spacing,
        }
    }

    pub(crate) fn to_core(&self) -> texform::FlattenGroupsConfig {
        texform::FlattenGroupsConfig {
            enabled: self.enabled,
            preserve_rendered_spacing: self.preserve_rendered_spacing,
        }
    }
}

/// Holds shared references to the four phase configs so `cfg.rewrite.enabled = False` sticks.
#[pyclass(name = "TransformConfig")]
pub(crate) struct PyTransformConfig {
    #[pyo3(get, set)]
    lower_attributes: Py<PyLowerAttributesConfig>,
    #[pyo3(get, set)]
    rewrite: Py<PyRewriteConfig>,
    #[pyo3(get, set)]
    finalize_ast: Py<PyFinalizeAstConfig>,
    #[pyo3(get, set)]
    flatten_groups: Py<PyFlattenGroupsConfig>,
}

#[pymethods]
impl PyTransformConfig {
    #[new]
    #[pyo3(signature = (*, lower_attributes, rewrite, finalize_ast, flatten_groups))]
    fn new(
        lower_attributes: Py<PyLowerAttributesConfig>,
        rewrite: Py<PyRewriteConfig>,
        finalize_ast: Py<PyFinalizeAstConfig>,
        flatten_groups: Py<PyFlattenGroupsConfig>,
    ) -> Self {
        Self {
            lower_attributes,
            rewrite,
            finalize_ast,
            flatten_groups,
        }
    }

    #[classmethod]
    fn authoring(_cls: &Bound<'_, PyType>, py: Python<'_>) -> PyResult<Self> {
        Self::from_core(py, texform::Profile::Authoring.default_transform_config())
    }

    #[classmethod]
    fn faithful(_cls: &Bound<'_, PyType>, py: Python<'_>) -> PyResult<Self> {
        Self::from_core(py, texform::Profile::Faithful.default_transform_config())
    }

    #[classmethod]
    fn corpus(_cls: &Bound<'_, PyType>, py: Python<'_>) -> PyResult<Self> {
        Self::from_core(py, texform::Profile::Corpus.default_transform_config())
    }

    #[classmethod]
    fn equiv(_cls: &Bound<'_, PyType>, py: Python<'_>) -> PyResult<Self> {
        Self::from_core(py, texform::Profile::Equiv.default_transform_config())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "TransformConfig(lower_attributes={}, rewrite={}, finalize_ast={}, flatten_groups={})",
            self.lower_attributes.borrow(py).__repr__(),
            self.rewrite.borrow(py).__repr__(),
            self.finalize_ast.borrow(py).__repr__(),
            self.flatten_groups.borrow(py).__repr__()
        )
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> bool {
        let Ok(other) = other.extract::<PyRef<'_, Self>>() else {
            return false;
        };
        *self.lower_attributes.borrow(py) == *other.lower_attributes.borrow(py)
            && *self.rewrite.borrow(py) == *other.rewrite.borrow(py)
            && *self.finalize_ast.borrow(py) == *other.finalize_ast.borrow(py)
            && *self.flatten_groups.borrow(py) == *other.flatten_groups.borrow(py)
    }
}

impl PyTransformConfig {
    pub(crate) fn from_core(py: Python<'_>, config: texform::TransformConfig) -> PyResult<Self> {
        Ok(Self {
            lower_attributes: Py::new(
                py,
                PyLowerAttributesConfig::from_core(config.lower_attributes),
            )?,
            rewrite: Py::new(py, PyRewriteConfig::from_core(config.rewrite))?,
            finalize_ast: Py::new(py, PyFinalizeAstConfig::from_core(config.finalize_ast))?,
            flatten_groups: Py::new(py, PyFlattenGroupsConfig::from_core(config.flatten_groups))?,
        })
    }

    pub(crate) fn to_core(&self, py: Python<'_>) -> texform::TransformConfig {
        texform::TransformConfig {
            lower_attributes: self.lower_attributes.borrow(py).to_core(),
            rewrite: self.rewrite.borrow(py).to_core(),
            finalize_ast: self.finalize_ast.borrow(py).to_core(),
            flatten_groups: self.flatten_groups.borrow(py).to_core(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(py: Python<'_>, source: &str) -> serde_json::Value {
        let value = py
            .eval(&std::ffi::CString::new(source).unwrap(), None, None)
            .expect("eval");
        py_to_json(&value, "test", &mut Vec::new()).expect("json")
    }

    fn json_err(py: Python<'_>, source: &str) -> String {
        let value = py
            .eval(&std::ffi::CString::new(source).unwrap(), None, None)
            .expect("eval");
        py_to_json(&value, "test", &mut Vec::new())
            .expect_err("expected error")
            .to_string()
    }

    #[test]
    fn py_to_json_maps_exact_host_types() {
        Python::attach(|py| {
            assert_eq!(json(py, "None"), serde_json::Value::Null);
            assert_eq!(json(py, "True"), serde_json::Value::Bool(true));
            assert_eq!(json(py, "False"), serde_json::Value::Bool(false));
            assert_eq!(json(py, "7"), serde_json::json!(7));
            assert_eq!(json(py, "1.5"), serde_json::json!(1.5));
            assert_eq!(json(py, "'yes'"), serde_json::json!("yes"));
            assert_eq!(json(py, "{'a': False}"), serde_json::json!({"a": false}));
            assert_eq!(json(py, "[1, True]"), serde_json::json!([1, true]));
            assert_eq!(json(py, "(1, True)"), serde_json::json!([1, true]));
        });
    }

    #[test]
    fn py_to_json_rejects_sets_and_non_str_keys() {
        Python::attach(|py| {
            let set_err = json_err(py, "{1}");
            assert!(
                set_err.contains("unsupported value of type `set`"),
                "{set_err}"
            );
            let key_err = json_err(py, "{1: True}");
            assert!(key_err.contains("dict keys must be strings"), "{key_err}");
        });
    }

    #[test]
    fn from_python_rejects_string_bools_and_unknown_keys() {
        Python::attach(|py| {
            let value = py.eval(c"{'reject_unknown': 'yes'}", None, None).unwrap();
            let error = from_python::<ParseConfigInput>(&value, "parse overrides")
                .expect_err("string bool");
            let message = error.to_string();
            assert!(message.contains("reject_unknown"), "{message}");
            assert!(message.contains("parse overrides"), "{message}");

            let value = py
                .eval(c"{'rewrite': {'enabeld': False}}", None, None)
                .unwrap();
            let error = from_python::<TransformConfigInput>(&value, "transform overrides")
                .expect_err("unknown key");
            let message = error.to_string();
            assert!(message.contains("rewrite.enabeld"), "{message}");
        });
    }
}
