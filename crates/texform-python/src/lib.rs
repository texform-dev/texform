use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pythonize::pythonize;
use texform_core::parse::{ParseConfig as CoreParseConfig, ParseContext};
use texform_core::serialize;
use texform_core::target_counter::{TargetCounter, count_node};
use texform_transform::{TransformConfig, run as transform};

pyo3::create_exception!(pytexform, ParseError, PyException);

#[pyclass(name = "ParseConfig")]
#[derive(Clone, Debug)]
struct PyParseConfig {
    #[pyo3(get, set)]
    strict: bool,
    #[pyo3(get, set)]
    recover: bool,
    #[pyo3(get, set)]
    max_group_depth: usize,
}

#[pymethods]
impl PyParseConfig {
    #[new]
    #[pyo3(signature = (strict = false, recover = true, max_group_depth = 128))]
    fn new(strict: bool, recover: bool, max_group_depth: usize) -> Self {
        Self {
            strict,
            recover,
            max_group_depth,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ParseConfig(strict={}, recover={}, max_group_depth={})",
            self.strict, self.recover, self.max_group_depth
        )
    }
}

impl PyParseConfig {
    fn into_core(&self) -> CoreParseConfig {
        CoreParseConfig {
            strict: self.strict,
            recover: self.recover,
            max_group_depth: self.max_group_depth,
        }
    }
}

fn py_config_to_core(config: Option<PyRef<'_, PyParseConfig>>) -> CoreParseConfig {
    config
        .as_deref()
        .map(PyParseConfig::into_core)
        .unwrap_or_default()
}

fn parse_context(packages: Option<Vec<String>>) -> PyResult<ParseContext> {
    match packages {
        Some(packages) => {
            let refs = packages.iter().map(String::as_str).collect::<Vec<_>>();
            ParseContext::try_from_packages(refs.as_slice())
                .map_err(|error| ParseError::new_err(error.to_string()))
        }
        None => Ok(ParseContext::shared().clone()),
    }
}

fn config_from_profile_name(name: &str) -> PyResult<&'static TransformConfig> {
    match name {
        "authoring" => Ok(&TransformConfig::AUTHORING),
        "corpus" => Ok(&TransformConfig::CORPUS),
        "corpus-drop" => Ok(&TransformConfig::CORPUS_DROP),
        "equiv" => Ok(&TransformConfig::EQUIV),
        other => Err(ParseError::new_err(format!(
            "unknown transform profile: {other}"
        ))),
    }
}

/// Parse a LaTeX formula.
///
/// Returns a dict with `node` and `span` keys on success.
/// Raises `pytexform.ParseError` when diagnostics are present.
/// The exception carries `diagnostics` (list[dict]) and `partial_result` (dict | None).
#[pyfunction]
#[pyo3(signature = (src, config = None, packages = None))]
fn parse(
    py: Python<'_>,
    src: &str,
    config: Option<PyRef<'_, PyParseConfig>>,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let ctx = parse_context(packages)?;
    let config = py_config_to_core(config);
    let output = ctx.parse(src, &config);

    if output.diagnostics.is_empty() {
        match output.result {
            Some(result) => Ok(pythonize(py, &result)?.unbind()),
            None => Err(ParseError::new_err(
                "parse produced no output and no diagnostics",
            )),
        }
    } else {
        let diagnostics = pythonize(py, &output.diagnostics)?.unbind();
        let partial_result: Py<PyAny> = match &output.result {
            Some(r) => pythonize(py, r)?.unbind(),
            None => py.None(),
        };

        let first_msg = output.diagnostics[0].message.clone();
        let err = ParseError::new_err(first_msg);
        let err_value = err.value(py);
        err_value.setattr("diagnostics", diagnostics)?;
        err_value.setattr("partial_result", partial_result)?;

        Err(err)
    }
}

#[pyfunction]
#[pyo3(signature = (src, profile = "authoring", packages = None))]
fn normalize(
    py: Python<'_>,
    src: &str,
    profile: &str,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let ctx = parse_context(packages)?;
    let parse_config = CoreParseConfig::STRICT_NO_RECOVER;
    let mut ast = ctx
        .parse_to_ast(src, &parse_config)
        .map_err(|error| ParseError::new_err(error.to_string()))?;
    let config = config_from_profile_name(profile)?;
    let report = transform(&mut ast, &ctx, config)
        .map_err(|error| ParseError::new_err(error.to_string()))?;
    let normalized = serialize::serialize(&ast);
    let applied = report
        .rewrite
        .applied
        .iter()
        .map(|stat| {
            serde_json::json!({
                "key": stat.key.to_string(),
                "count": stat.count,
                "skipped_count": stat.skipped_count,
            })
        })
        .collect::<Vec<_>>();

    let out = serde_json::json!({
        "normalized": normalized,
        "report": {
            "iterations": report.rewrite.iterations,
            "applied": applied,
            "lower_attributes": {
                "eliminated_empty_segments": report.lower_attributes.eliminated_empty_segments,
            },
        },
    });
    Ok(pythonize(py, &out)?.unbind())
}

#[pyfunction]
#[pyo3(signature = (src, config = None, packages = None))]
fn count_targets(
    py: Python<'_>,
    src: &str,
    config: Option<PyRef<'_, PyParseConfig>>,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let ctx = parse_context(packages)?;
    let config = py_config_to_core(config);
    let output = ctx.parse(src, &config);
    if !output.diagnostics.is_empty() {
        let first_msg = output.diagnostics[0].message.clone();
        return Err(ParseError::new_err(first_msg));
    }
    let result = output
        .result
        .ok_or_else(|| ParseError::new_err("parse produced no output and no diagnostics"))?;
    let mut counter = TargetCounter::default();
    count_node(&result.node, &mut counter);
    Ok(pythonize(py, &counter.logical_counts())?.unbind())
}

/// Native extension module loaded as `pytexform._native`.
/// Symbols are re-exported from the Python package's `__init__.py`.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(normalize, m)?)?;
    m.add_function(wrap_pyfunction!(count_targets, m)?)?;
    m.add_class::<PyParseConfig>()?;
    m.add("ParseError", m.py().get_type::<ParseError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_module_normalizes_with_profile_and_packages() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs
                .set_item("packages", vec!["base", "physics"])
                .unwrap();
            kwargs.set_item("profile", "authoring").unwrap();

            let result = module
                .getattr("normalize")
                .unwrap()
                .call((r"\quantity{x}",), Some(&kwargs))
                .unwrap();
            let dict = result.downcast::<pyo3::types::PyDict>().unwrap();

            assert_eq!(
                dict.get_item("normalized")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"\qty { x }"
            );
        });
    }

    #[test]
    fn python_module_counts_targets() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let result = module
                .getattr("count_targets")
                .unwrap()
                .call1((r"\frac{a}{b} \le c",))
                .unwrap();
            let dict = result.downcast::<pyo3::types::PyDict>().unwrap();

            assert_eq!(
                dict.get_item("cmd:frac")
                    .unwrap()
                    .unwrap()
                    .extract::<u32>()
                    .unwrap(),
                1
            );
            assert_eq!(
                dict.get_item("char:le")
                    .unwrap()
                    .unwrap()
                    .extract::<u32>()
                    .unwrap(),
                1
            );
        });
    }
}
