use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pythonize::pythonize;
use texform_core::api;

pyo3::create_exception!(pytexform, ParseError, PyException);

/// Parse a LaTeX formula.
///
/// Returns a dict with `node` and `span` keys on success.
/// Raises `pytexform.ParseError` when diagnostics are present.
/// The exception carries `diagnostics` (list[dict]) and `partial_result` (dict | None).
#[pyfunction]
#[pyo3(signature = (src, strict = false))]
fn parse(py: Python<'_>, src: &str, strict: bool) -> PyResult<Py<PyAny>> {
    let output = api::parse_latex(src, strict);

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

        let err = ParseError::new_err("parse error");
        let err_value = err.value(py);
        err_value.setattr("diagnostics", diagnostics)?;
        err_value.setattr("partial_result", partial_result)?;

        Err(err)
    }
}

/// Python module: pytexform
#[pymodule]
fn pytexform(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add("ParseError", m.py().get_type::<ParseError>())?;
    Ok(())
}
