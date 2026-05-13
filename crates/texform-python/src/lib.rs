use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pythonize::pythonize;
use texform_core::parse::ParseContext;
use texform_core::serialize;
use texform_core::target_counter::{TargetCounter, count_node};
use texform_core::transform::{TransformProfile, transform_ast};

pyo3::create_exception!(pytexform, ParseError, PyException);

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

fn profile_from_name(name: &str) -> PyResult<TransformProfile> {
    match name {
        "authoring" => Ok(TransformProfile::AUTHORING),
        "corpus" => Ok(TransformProfile::CORPUS),
        "corpus-drop" => Ok(TransformProfile::CORPUS_DROP),
        "equiv" => Ok(TransformProfile::EQUIV),
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
#[pyo3(signature = (src, strict = false, packages = None))]
fn parse(
    py: Python<'_>,
    src: &str,
    strict: bool,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let ctx = parse_context(packages)?;
    let output = ctx.parse(src, strict);

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
#[pyo3(signature = (src, profile = "authoring", strict = true, packages = None))]
fn normalize(
    py: Python<'_>,
    src: &str,
    profile: &str,
    strict: bool,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let ctx = parse_context(packages)?;
    let mut ast = ctx
        .parse_to_ast(src, strict)
        .map_err(|error| ParseError::new_err(error.to_string()))?;
    let transform_ctx = profile_from_name(profile)?
        .builder()
        .build_with(&ctx)
        .map_err(|error| ParseError::new_err(error.to_string()))?;
    let report = transform_ast(&mut ast, &ctx, &transform_ctx)
        .map_err(|error| ParseError::new_err(error.to_string()))?;
    let normalized = serialize::serialize(&ast);
    let applied = report
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
            "iterations": report.iterations,
            "applied": applied,
            "lower_declarative": {
                "eliminated_empty_segments": report.lower_declarative.eliminated_empty_segments,
            },
        },
    });
    Ok(pythonize(py, &out)?.unbind())
}

#[pyfunction]
#[pyo3(signature = (src, strict = false, packages = None))]
fn count_targets(
    py: Python<'_>,
    src: &str,
    strict: bool,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let ctx = parse_context(packages)?;
    let output = ctx.parse(src, strict);
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
