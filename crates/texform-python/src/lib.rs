use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pythonize::pythonize;
use texform_core::parse::{ParseConfig as CoreParseConfig, ParseContext};
use texform_core::serialize;
use texform_core::target_counter::{TargetCounter, count_node};
use texform_transform::{
    FlattenGroupsConfig as CoreFlattenGroupsConfig,
    LowerAttributesConfig as CoreLowerAttributesConfig, RewriteConfig as CoreRewriteConfig,
    RuleClassSet, RuleSelection, TransformConfig as CoreTransformConfig, run as transform_run,
};

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

#[pyclass(name = "LowerAttributesConfig")]
#[derive(Clone, Debug)]
struct PyLowerAttributesConfig {
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
        format!("LowerAttributesConfig(enabled={})", self.enabled)
    }
}

impl PyLowerAttributesConfig {
    fn from_core(config: CoreLowerAttributesConfig) -> Self {
        Self {
            enabled: config.enabled,
        }
    }

    fn into_core(&self) -> CoreLowerAttributesConfig {
        CoreLowerAttributesConfig {
            enabled: self.enabled,
        }
    }
}

#[pyclass(name = "RewriteConfig")]
#[derive(Clone, Debug)]
struct PyRewriteConfig {
    #[pyo3(get, set)]
    enabled: bool,
    #[pyo3(get, set)]
    classes: Vec<String>,
    #[pyo3(get, set)]
    max_iterations: usize,
}

#[pymethods]
impl PyRewriteConfig {
    #[new]
    #[pyo3(signature = (enabled = true, classes = None, max_iterations = 100))]
    fn new(enabled: bool, classes: Option<Vec<String>>, max_iterations: usize) -> Self {
        Self {
            enabled,
            classes: classes.unwrap_or_default(),
            max_iterations,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "RewriteConfig(enabled={}, classes={:?}, max_iterations={})",
            self.enabled, self.classes, self.max_iterations
        )
    }
}

impl PyRewriteConfig {
    fn from_core(config: CoreRewriteConfig) -> Self {
        Self {
            enabled: config.enabled,
            classes: class_names(config.classes),
            max_iterations: config.max_iterations,
        }
    }

    fn into_core(&self) -> PyResult<CoreRewriteConfig> {
        Ok(CoreRewriteConfig {
            enabled: self.enabled,
            classes: class_set_from_names(&self.classes)?,
            max_iterations: self.max_iterations,
            selection: RuleSelection::All,
        })
    }
}

#[pyclass(name = "FlattenGroupsConfig")]
#[derive(Clone, Debug)]
struct PyFlattenGroupsConfig {
    #[pyo3(get, set)]
    enabled: bool,
    #[pyo3(get, set)]
    preserve_group_containing_declarative_command: bool,
    #[pyo3(get, set)]
    preserve_group_in_script_base_slot: bool,
    #[pyo3(get, set)]
    preserve_group_inside_env_body: bool,
    #[pyo3(get, set)]
    preserve_group_containing_infix: bool,
    #[pyo3(get, set)]
    preserve_group_adjacent_to_command_like: bool,
    #[pyo3(get, set)]
    preserve_group_after_scripted_command_like: bool,
    #[pyo3(get, set)]
    preserve_empty_group: bool,
    #[pyo3(get, set)]
    preserve_group_with_lone_atom_spacing_char: bool,
    #[pyo3(get, set)]
    preserve_group_starting_with_atom_spacing_char: bool,
    #[pyo3(get, set)]
    preserve_group_containing_delimited_pair: bool,
}

#[pymethods]
impl PyFlattenGroupsConfig {
    #[new]
    #[pyo3(signature = (
        enabled = true,
        preserve_group_containing_declarative_command = true,
        preserve_group_in_script_base_slot = true,
        preserve_group_inside_env_body = true,
        preserve_group_containing_infix = true,
        preserve_group_adjacent_to_command_like = true,
        preserve_group_after_scripted_command_like = true,
        preserve_empty_group = true,
        preserve_group_with_lone_atom_spacing_char = true,
        preserve_group_starting_with_atom_spacing_char = true,
        preserve_group_containing_delimited_pair = true
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        enabled: bool,
        preserve_group_containing_declarative_command: bool,
        preserve_group_in_script_base_slot: bool,
        preserve_group_inside_env_body: bool,
        preserve_group_containing_infix: bool,
        preserve_group_adjacent_to_command_like: bool,
        preserve_group_after_scripted_command_like: bool,
        preserve_empty_group: bool,
        preserve_group_with_lone_atom_spacing_char: bool,
        preserve_group_starting_with_atom_spacing_char: bool,
        preserve_group_containing_delimited_pair: bool,
    ) -> Self {
        Self {
            enabled,
            preserve_group_containing_declarative_command,
            preserve_group_in_script_base_slot,
            preserve_group_inside_env_body,
            preserve_group_containing_infix,
            preserve_group_adjacent_to_command_like,
            preserve_group_after_scripted_command_like,
            preserve_empty_group,
            preserve_group_with_lone_atom_spacing_char,
            preserve_group_starting_with_atom_spacing_char,
            preserve_group_containing_delimited_pair,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "FlattenGroupsConfig(enabled={}, preserve_group_containing_declarative_command={}, preserve_group_in_script_base_slot={}, preserve_group_inside_env_body={}, preserve_group_containing_infix={}, preserve_group_adjacent_to_command_like={}, preserve_group_after_scripted_command_like={}, preserve_empty_group={}, preserve_group_with_lone_atom_spacing_char={}, preserve_group_starting_with_atom_spacing_char={}, preserve_group_containing_delimited_pair={})",
            self.enabled,
            self.preserve_group_containing_declarative_command,
            self.preserve_group_in_script_base_slot,
            self.preserve_group_inside_env_body,
            self.preserve_group_containing_infix,
            self.preserve_group_adjacent_to_command_like,
            self.preserve_group_after_scripted_command_like,
            self.preserve_empty_group,
            self.preserve_group_with_lone_atom_spacing_char,
            self.preserve_group_starting_with_atom_spacing_char,
            self.preserve_group_containing_delimited_pair
        )
    }
}

impl PyFlattenGroupsConfig {
    fn from_core(config: CoreFlattenGroupsConfig) -> Self {
        Self {
            enabled: config.enabled,
            preserve_group_containing_declarative_command: config
                .preserve_group_containing_declarative_command,
            preserve_group_in_script_base_slot: config.preserve_group_in_script_base_slot,
            preserve_group_inside_env_body: config.preserve_group_inside_env_body,
            preserve_group_containing_infix: config.preserve_group_containing_infix,
            preserve_group_adjacent_to_command_like: config.preserve_group_adjacent_to_command_like,
            preserve_group_after_scripted_command_like: config
                .preserve_group_after_scripted_command_like,
            preserve_empty_group: config.preserve_empty_group,
            preserve_group_with_lone_atom_spacing_char: config
                .preserve_group_with_lone_atom_spacing_char,
            preserve_group_starting_with_atom_spacing_char: config
                .preserve_group_starting_with_atom_spacing_char,
            preserve_group_containing_delimited_pair: config
                .preserve_group_containing_delimited_pair,
        }
    }

    fn into_core(&self) -> CoreFlattenGroupsConfig {
        CoreFlattenGroupsConfig {
            enabled: self.enabled,
            preserve_group_containing_declarative_command: self
                .preserve_group_containing_declarative_command,
            preserve_group_in_script_base_slot: self.preserve_group_in_script_base_slot,
            preserve_group_inside_env_body: self.preserve_group_inside_env_body,
            preserve_group_containing_infix: self.preserve_group_containing_infix,
            preserve_group_adjacent_to_command_like: self.preserve_group_adjacent_to_command_like,
            preserve_group_after_scripted_command_like: self
                .preserve_group_after_scripted_command_like,
            preserve_empty_group: self.preserve_empty_group,
            preserve_group_with_lone_atom_spacing_char: self
                .preserve_group_with_lone_atom_spacing_char,
            preserve_group_starting_with_atom_spacing_char: self
                .preserve_group_starting_with_atom_spacing_char,
            preserve_group_containing_delimited_pair: self.preserve_group_containing_delimited_pair,
        }
    }
}

#[pyclass(name = "TransformConfig")]
#[derive(Clone, Debug)]
struct PyTransformConfig {
    #[pyo3(get, set)]
    lower_attributes: PyLowerAttributesConfig,
    #[pyo3(get, set)]
    rewrite: PyRewriteConfig,
    #[pyo3(get, set)]
    flatten_groups: PyFlattenGroupsConfig,
}

#[pymethods]
impl PyTransformConfig {
    #[new]
    #[pyo3(signature = (lower_attributes = None, rewrite = None, flatten_groups = None))]
    fn new(
        lower_attributes: Option<PyLowerAttributesConfig>,
        rewrite: Option<PyRewriteConfig>,
        flatten_groups: Option<PyFlattenGroupsConfig>,
    ) -> Self {
        Self {
            lower_attributes: lower_attributes.unwrap_or_else(|| {
                PyLowerAttributesConfig::from_core(CoreTransformConfig::AUTHORING.lower_attributes)
            }),
            rewrite: rewrite.unwrap_or_else(|| {
                PyRewriteConfig::from_core(CoreTransformConfig::AUTHORING.rewrite.clone())
            }),
            flatten_groups: flatten_groups.unwrap_or_else(|| {
                PyFlattenGroupsConfig::from_core(CoreTransformConfig::AUTHORING.flatten_groups)
            }),
        }
    }

    #[classmethod]
    fn authoring(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_core(CoreTransformConfig::AUTHORING.clone())
    }

    #[classmethod]
    fn corpus(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_core(CoreTransformConfig::CORPUS.clone())
    }

    #[classmethod]
    fn corpus_drop(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_core(CoreTransformConfig::CORPUS_DROP.clone())
    }

    #[classmethod]
    fn equiv(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_core(CoreTransformConfig::EQUIV.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "TransformConfig(lower_attributes={:?}, rewrite={:?}, flatten_groups={:?})",
            self.lower_attributes, self.rewrite, self.flatten_groups
        )
    }
}

impl PyTransformConfig {
    fn from_core(config: CoreTransformConfig) -> Self {
        Self {
            lower_attributes: PyLowerAttributesConfig::from_core(config.lower_attributes),
            rewrite: PyRewriteConfig::from_core(config.rewrite),
            flatten_groups: PyFlattenGroupsConfig::from_core(config.flatten_groups),
        }
    }

    fn into_core(&self) -> PyResult<CoreTransformConfig> {
        Ok(CoreTransformConfig {
            lower_attributes: self.lower_attributes.into_core(),
            rewrite: self.rewrite.into_core()?,
            flatten_groups: self.flatten_groups.into_core(),
        })
    }
}

fn py_parse_config_to_core(config: Option<PyRef<'_, PyParseConfig>>) -> CoreParseConfig {
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

fn config_from_profile_name(name: &str) -> PyResult<CoreTransformConfig> {
    match name {
        "authoring" => Ok(CoreTransformConfig::AUTHORING.clone()),
        "corpus" => Ok(CoreTransformConfig::CORPUS.clone()),
        "corpus-drop" => Ok(CoreTransformConfig::CORPUS_DROP.clone()),
        "equiv" => Ok(CoreTransformConfig::EQUIV.clone()),
        other => Err(ParseError::new_err(format!(
            "unknown transform profile: {other}"
        ))),
    }
}

fn class_names(classes: RuleClassSet) -> Vec<String> {
    classes
        .iter()
        .map(|class| class.as_str().to_string())
        .collect()
}

fn class_set_from_names(names: &[String]) -> PyResult<RuleClassSet> {
    let mut set = RuleClassSet::empty();
    for name in names {
        let class = match name.as_str() {
            "standard" => RuleClassSet::STANDARD,
            "expand" => RuleClassSet::EXPAND,
            "drop" => RuleClassSet::DROP,
            "equiv" => RuleClassSet::EQUIV,
            other => {
                return Err(ParseError::new_err(format!(
                    "unknown rewrite rule class: {other}"
                )));
            }
        };
        set |= class;
    }
    Ok(set)
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
    let config = py_parse_config_to_core(config);
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
    let config = config_from_profile_name(profile)?;
    transform_with_core_config(py, src, &config, packages)
}

#[pyfunction]
#[pyo3(signature = (src, config = None, packages = None))]
fn transform(
    py: Python<'_>,
    src: &str,
    config: Option<PyRef<'_, PyTransformConfig>>,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let config = config
        .as_deref()
        .map(PyTransformConfig::into_core)
        .transpose()?
        .unwrap_or_else(|| CoreTransformConfig::AUTHORING.clone());
    transform_with_core_config(py, src, &config, packages)
}

fn transform_with_core_config(
    py: Python<'_>,
    src: &str,
    config: &CoreTransformConfig,
    packages: Option<Vec<String>>,
) -> PyResult<Py<PyAny>> {
    let ctx = parse_context(packages)?;
    let parse_config = CoreParseConfig::STRICT_NO_RECOVER;
    let mut ast = ctx
        .parse_to_ast(src, &parse_config)
        .map_err(|error| ParseError::new_err(error.to_string()))?;
    let report = transform_run(&mut ast, &ctx, config)
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
            "flatten_groups": {
                "removed_empty": report.flatten_groups.removed_empty,
                "replaced_single_child": report.flatten_groups.replaced_single_child,
                "inlined_multi_child": report.flatten_groups.inlined_multi_child,
                "unwrapped_slot": report.flatten_groups.unwrapped_slot,
                "preserved_group_containing_declarative_command": report.flatten_groups.preserved_group_containing_declarative_command,
                "preserved_group_in_script_base_slot": report.flatten_groups.preserved_group_in_script_base_slot,
                "preserved_group_inside_env_body": report.flatten_groups.preserved_group_inside_env_body,
                "preserved_group_containing_infix": report.flatten_groups.preserved_group_containing_infix,
                "preserved_group_adjacent_to_command_like": report.flatten_groups.preserved_group_adjacent_to_command_like,
                "preserved_group_after_scripted_command_like": report.flatten_groups.preserved_group_after_scripted_command_like,
                "preserved_empty_group": report.flatten_groups.preserved_empty_group,
                "preserved_group_with_lone_atom_spacing_char": report.flatten_groups.preserved_group_with_lone_atom_spacing_char,
                "preserved_group_starting_with_atom_spacing_char": report.flatten_groups.preserved_group_starting_with_atom_spacing_char,
                "preserved_group_containing_delimited_pair": report.flatten_groups.preserved_group_containing_delimited_pair,
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
    let config = py_parse_config_to_core(config);
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
    m.add_function(wrap_pyfunction!(transform, m)?)?;
    m.add_function(wrap_pyfunction!(count_targets, m)?)?;
    m.add_class::<PyParseConfig>()?;
    m.add_class::<PyLowerAttributesConfig>()?;
    m.add_class::<PyRewriteConfig>()?;
    m.add_class::<PyFlattenGroupsConfig>()?;
    m.add_class::<PyTransformConfig>()?;
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
    fn python_module_transforms_with_per_guard_override() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let flatten_groups_cls = module.getattr("FlattenGroupsConfig").unwrap();
            let flatten_groups_kwargs = pyo3::types::PyDict::new(py);
            flatten_groups_kwargs
                .set_item("preserve_group_adjacent_to_command_like", false)
                .unwrap();
            let flatten_groups = flatten_groups_cls
                .call((), Some(&flatten_groups_kwargs))
                .unwrap();

            let config_cls = module.getattr("TransformConfig").unwrap();
            let config_kwargs = pyo3::types::PyDict::new(py);
            config_kwargs
                .set_item("flatten_groups", flatten_groups)
                .unwrap();
            let config = config_cls.call((), Some(&config_kwargs)).unwrap();

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("config", config).unwrap();
            let result = module
                .getattr("transform")
                .unwrap()
                .call((r"\cos{A}",), Some(&kwargs))
                .unwrap();
            let dict = result.downcast::<pyo3::types::PyDict>().unwrap();

            assert_eq!(
                dict.get_item("normalized")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"\cos A"
            );
        });
    }

    #[test]
    fn python_module_corpus_drop_disables_spacing_guards() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let config_cls = module.getattr("TransformConfig").unwrap();
            let config = config_cls.call_method0("corpus_drop").unwrap();
            let flatten_groups = config.getattr("flatten_groups").unwrap();

            assert!(
                !flatten_groups
                    .getattr("preserve_group_adjacent_to_command_like")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert!(
                flatten_groups
                    .getattr("preserve_group_containing_infix")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
        });
    }

    #[test]
    fn python_module_transform_config_repr_mentions_children() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let config_cls = module.getattr("TransformConfig").unwrap();
            let config = config_cls.call_method0("authoring").unwrap();
            let repr = config.call_method0("__repr__").unwrap();
            let repr = repr.extract::<String>().unwrap();

            assert!(repr.contains("lower_attributes"));
            assert!(repr.contains("flatten_groups"));
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
