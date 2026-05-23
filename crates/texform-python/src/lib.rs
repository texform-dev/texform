use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::pythonize;
use texform_core::parse::{ParseConfig as CoreParseConfig, Parser as CoreParser};
use texform_core::target_counter::{TargetCounter, count_node};
use texform_transform::{
    FlattenGroupsConfig as CoreFlattenGroupsConfig,
    LowerAttributesConfig as CoreLowerAttributesConfig, Profile as CoreProfile,
    TransformConfig as CoreTransformConfig,
};

pyo3::create_exception!(texform, ParseError, PyException);

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
            self.enabled, self.max_iterations
        )
    }
}

impl PyRewriteConfig {
    fn from_core(enabled: bool, max_iterations: usize) -> Self {
        Self {
            enabled,
            max_iterations,
        }
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
    preserve_group_as_argument_of_command: bool,
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
        preserve_group_as_argument_of_command = true,
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
        preserve_group_as_argument_of_command: bool,
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
            preserve_group_as_argument_of_command,
            preserve_group_after_scripted_command_like,
            preserve_empty_group,
            preserve_group_with_lone_atom_spacing_char,
            preserve_group_starting_with_atom_spacing_char,
            preserve_group_containing_delimited_pair,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "FlattenGroupsConfig(enabled={}, preserve_group_containing_declarative_command={}, preserve_group_in_script_base_slot={}, preserve_group_inside_env_body={}, preserve_group_containing_infix={}, preserve_group_adjacent_to_command_like={}, preserve_group_as_argument_of_command={}, preserve_group_after_scripted_command_like={}, preserve_empty_group={}, preserve_group_with_lone_atom_spacing_char={}, preserve_group_starting_with_atom_spacing_char={}, preserve_group_containing_delimited_pair={})",
            self.enabled,
            self.preserve_group_containing_declarative_command,
            self.preserve_group_in_script_base_slot,
            self.preserve_group_inside_env_body,
            self.preserve_group_containing_infix,
            self.preserve_group_adjacent_to_command_like,
            self.preserve_group_as_argument_of_command,
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
            preserve_group_as_argument_of_command: config.preserve_group_as_argument_of_command,
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
            preserve_group_as_argument_of_command: self.preserve_group_as_argument_of_command,
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
                PyLowerAttributesConfig::from_core(CoreLowerAttributesConfig::ENABLED)
            }),
            rewrite: rewrite.unwrap_or_else(|| {
                let profile = CoreProfile::Authoring;
                PyRewriteConfig::from_core(
                    profile.default_transform_config().rewrite_enabled,
                    profile.default_transform_config().max_iterations,
                )
            }),
            flatten_groups: flatten_groups.unwrap_or_else(|| {
                PyFlattenGroupsConfig::from_core(
                    CoreProfile::Authoring
                        .default_transform_config()
                        .flatten_groups,
                )
            }),
        }
    }

    #[classmethod]
    fn authoring(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::Authoring)
    }

    #[classmethod]
    fn corpus(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::Corpus)
    }

    #[classmethod]
    fn corpus_drop(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::CorpusDrop)
    }

    #[classmethod]
    fn equiv(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::Equiv)
    }

    fn __repr__(&self) -> String {
        format!(
            "TransformConfig(lower_attributes={:?}, rewrite={:?}, flatten_groups={:?})",
            self.lower_attributes, self.rewrite, self.flatten_groups
        )
    }
}

impl PyTransformConfig {
    fn from_profile(profile: CoreProfile) -> Self {
        let config = profile.default_transform_config();
        Self {
            lower_attributes: PyLowerAttributesConfig {
                enabled: config.lower_attributes_enabled,
            },
            rewrite: PyRewriteConfig::from_core(config.rewrite_enabled, config.max_iterations),
            flatten_groups: PyFlattenGroupsConfig::from_core(config.flatten_groups),
        }
    }

    fn into_core(&self) -> CoreTransformConfig {
        CoreTransformConfig {
            lower_attributes_enabled: self.lower_attributes.enabled,
            rewrite_enabled: self.rewrite.enabled,
            flatten_groups: self.flatten_groups.into_core(),
            max_iterations: self.rewrite.max_iterations,
        }
    }
}

fn py_parse_config_to_core(config: Option<PyRef<'_, PyParseConfig>>) -> CoreParseConfig {
    config
        .as_deref()
        .map(PyParseConfig::into_core)
        .unwrap_or_default()
}

fn parse_context(packages: Option<Vec<String>>) -> PyResult<CoreParser> {
    match packages {
        Some(packages) => {
            let refs = packages.iter().map(String::as_str).collect::<Vec<_>>();
            CoreParser::try_from_packages(refs.as_slice())
                .map_err(|error| ParseError::new_err(error.to_string()))
        }
        None => Ok(CoreParser::shared().clone()),
    }
}

fn profile_from_name(name: &str) -> PyResult<texform::Profile> {
    match name {
        "authoring" => Ok(texform::Profile::Authoring),
        "corpus" => Ok(texform::Profile::Corpus),
        "corpus-drop" => Ok(texform::Profile::CorpusDrop),
        "equiv" => Ok(texform::Profile::Equiv),
        other => Err(ParseError::new_err(format!(
            "unknown transform profile: {other}"
        ))),
    }
}

fn rule_key_from_string(value: &str) -> PyResult<texform_transform::RuleKey> {
    texform_transform::rewrite::all_rules()
        .iter()
        .find_map(|rule| {
            let key = rule.meta().key;
            (key.to_string() == value).then_some(key)
        })
        .ok_or_else(|| ParseError::new_err(format!("unknown transform rule: {value}")))
}

fn py_required_string(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    dict.get_item(key)?
        .ok_or_else(|| ParseError::new_err(format!("context item missing `{key}`")))?
        .extract::<String>()
}

fn py_optional_strings(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Vec<String>> {
    Ok(match dict.get_item(key)? {
        Some(value) if !value.is_none() => value.extract::<Vec<String>>()?,
        _ => Vec::new(),
    })
}

fn py_command_kind(value: &str) -> PyResult<texform::CommandKind> {
    match value {
        "prefix" => Ok(texform::CommandKind::Prefix),
        "infix" => Ok(texform::CommandKind::Infix),
        "declarative" => Ok(texform::CommandKind::Declarative),
        other => Err(ParseError::new_err(format!(
            "unsupported command kind: {other}"
        ))),
    }
}

fn py_allowed_mode(value: &str) -> PyResult<texform::AllowedMode> {
    match value {
        "math" => Ok(texform::AllowedMode::Math),
        "text" => Ok(texform::AllowedMode::Text),
        "both" => Ok(texform::AllowedMode::Both),
        other => Err(ParseError::new_err(format!(
            "unsupported allowed mode: {other}"
        ))),
    }
}

fn py_content_mode(value: &str) -> PyResult<texform::ContentMode> {
    match value {
        "math" => Ok(texform::ContentMode::Math),
        "text" => Ok(texform::ContentMode::Text),
        other => Err(ParseError::new_err(format!(
            "unsupported content mode: {other}"
        ))),
    }
}

fn py_context_item(py: Python<'_>, item: &Py<PyAny>) -> PyResult<texform::ContextItem> {
    let value = item.bind(py);
    let dict = value
        .downcast::<PyDict>()
        .map_err(|_| ParseError::new_err("context item must be a dict"))?;
    match py_required_string(dict, "target")?.as_str() {
        "command" => Ok(texform::CommandItem::new(
            py_required_string(dict, "name")?,
            py_command_kind(&py_required_string(dict, "kind")?)?,
            py_allowed_mode(&py_required_string(dict, "allowed_mode")?)?,
            py_required_string(dict, "argspec")?,
        )
        .with_tags(py_optional_strings(dict, "tags")?)
        .into()),
        "environment" => Ok(texform::EnvironmentItem::new(
            py_required_string(dict, "name")?,
            py_allowed_mode(&py_required_string(dict, "allowed_mode")?)?,
            py_content_mode(&py_required_string(dict, "body_mode")?)?,
            py_required_string(dict, "argspec")?,
        )
        .with_tags(py_optional_strings(dict, "tags")?)
        .into()),
        "delimiter" => {
            Ok(texform::DelimiterControlItem::new(py_required_string(dict, "name")?).into())
        }
        other => Err(ParseError::new_err(format!(
            "unsupported context item target: {other}"
        ))),
    }
}

fn parser_builder_with_options(
    py: Python<'_>,
    packages: Option<Vec<String>>,
    items: Option<Vec<Py<PyAny>>>,
    remove_commands: Option<Vec<String>>,
    remove_environments: Option<Vec<String>>,
    remove_delimiter_controls: Option<Vec<String>>,
) -> PyResult<texform::ParserBuilder> {
    let mut builder = texform::Parser::builder();
    if let Some(packages) = packages {
        let refs = packages.iter().map(String::as_str).collect::<Vec<_>>();
        builder = if refs.is_empty() {
            builder.empty_knowledge()
        } else {
            builder.packages(refs.as_slice())
        };
    }
    for item in items.unwrap_or_default() {
        builder = builder.item(py_context_item(py, &item)?);
    }
    for name in remove_commands.unwrap_or_default() {
        builder = builder.remove_command(name);
    }
    for name in remove_environments.unwrap_or_default() {
        builder = builder.remove_environment(name);
    }
    for name in remove_delimiter_controls.unwrap_or_default() {
        builder = builder.remove_delimiter_control(name);
    }
    Ok(builder)
}

fn engine_builder_with_options(
    py: Python<'_>,
    mut builder: texform::EngineBuilder,
    packages: Option<Vec<String>>,
    items: Option<Vec<Py<PyAny>>>,
    remove_commands: Option<Vec<String>>,
    remove_environments: Option<Vec<String>>,
    remove_delimiter_controls: Option<Vec<String>>,
) -> PyResult<texform::EngineBuilder> {
    if let Some(packages) = packages {
        let refs = packages.iter().map(String::as_str).collect::<Vec<_>>();
        builder = if refs.is_empty() {
            builder.empty_knowledge()
        } else {
            builder.packages(refs.as_slice())
        };
    }
    for item in items.unwrap_or_default() {
        builder = builder.item(py_context_item(py, &item)?);
    }
    for name in remove_commands.unwrap_or_default() {
        builder = builder.remove_command(name);
    }
    for name in remove_environments.unwrap_or_default() {
        builder = builder.remove_environment(name);
    }
    for name in remove_delimiter_controls.unwrap_or_default() {
        builder = builder.remove_delimiter_control(name);
    }
    Ok(builder)
}

fn parse_output_to_python(py: Python<'_>, output: texform::ParseOutput) -> PyResult<Py<PyAny>> {
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

#[pyclass(name = "Parser")]
struct PyParser {
    inner: texform::Parser,
}

#[pymethods]
impl PyParser {
    #[new]
    #[pyo3(signature = (
        packages = None,
        items = None,
        remove_commands = None,
        remove_environments = None,
        remove_delimiter_controls = None,
    ))]
    fn new(
        py: Python<'_>,
        packages: Option<Vec<String>>,
        items: Option<Vec<Py<PyAny>>>,
        remove_commands: Option<Vec<String>>,
        remove_environments: Option<Vec<String>>,
        remove_delimiter_controls: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let builder = parser_builder_with_options(
            py,
            packages,
            items,
            remove_commands,
            remove_environments,
            remove_delimiter_controls,
        )?;
        Ok(Self {
            inner: builder
                .build()
                .map_err(|error| ParseError::new_err(error.to_string()))?,
        })
    }

    #[pyo3(signature = (src, config = None))]
    fn parse(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<PyRef<'_, PyParseConfig>>,
    ) -> PyResult<Py<PyAny>> {
        parse_output_to_python(
            py,
            self.inner.parse_with(src, &py_parse_config_to_core(config)),
        )
    }

    fn knows_command_name(&self, name: &str) -> bool {
        self.inner.knows_command_name(name)
    }
}

#[pyclass(name = "Engine")]
struct PyEngine {
    inner: texform::Engine,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (
        profile,
        packages = None,
        items = None,
        remove_commands = None,
        remove_environments = None,
        remove_delimiter_controls = None,
        disable_rules = None,
    ))]
    fn new(
        py: Python<'_>,
        profile: &str,
        packages: Option<Vec<String>>,
        items: Option<Vec<Py<PyAny>>>,
        remove_commands: Option<Vec<String>>,
        remove_environments: Option<Vec<String>>,
        remove_delimiter_controls: Option<Vec<String>>,
        disable_rules: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let mut builder = engine_builder_with_options(
            py,
            texform::Engine::builder().profile(profile_from_name(profile)?),
            packages,
            items,
            remove_commands,
            remove_environments,
            remove_delimiter_controls,
        )?;
        for rule in disable_rules.unwrap_or_default() {
            builder = builder.disable_rule(rule_key_from_string(&rule)?);
        }
        Ok(Self {
            inner: builder
                .build()
                .map_err(|error| ParseError::new_err(error.to_string()))?,
        })
    }

    #[pyo3(signature = (src, config = None, parse_config = None))]
    fn normalize(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<PyRef<'_, PyTransformConfig>>,
        parse_config: Option<PyRef<'_, PyParseConfig>>,
    ) -> PyResult<Py<PyAny>> {
        let config = texform::NormalizeConfig {
            parse: parse_config
                .as_deref()
                .map(PyParseConfig::into_core)
                .unwrap_or(CoreParseConfig::STRICT_NO_RECOVER),
            transform: config
                .as_deref()
                .map(PyTransformConfig::into_core)
                .unwrap_or(*self.inner.default_transform_config()),
        };
        let result = self
            .inner
            .normalize_with(src, &config)
            .map_err(|error| ParseError::new_err(error.to_string()))?;
        transform_result_to_python(py, result.normalized, &result.report)
    }
}

fn transform_result_to_python(
    py: Python<'_>,
    normalized: String,
    report: &texform::TransformReport,
) -> PyResult<Py<PyAny>> {
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
                "preserved_group_as_argument_of_command": report.flatten_groups.preserved_group_as_argument_of_command,
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
fn validate_argspec(py: Python<'_>, spec: &str) -> PyResult<Py<PyAny>> {
    Ok(pythonize(py, &texform::validate_argspec(spec))?.unbind())
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

/// Native extension module loaded as `texform._native`.
/// Symbols are re-exported from the Python package's `__init__.py`.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(count_targets, m)?)?;
    m.add_function(wrap_pyfunction!(validate_argspec, m)?)?;
    m.add_class::<PyParseConfig>()?;
    m.add_class::<PyLowerAttributesConfig>()?;
    m.add_class::<PyRewriteConfig>()?;
    m.add_class::<PyFlattenGroupsConfig>()?;
    m.add_class::<PyTransformConfig>()?;
    m.add_class::<PyParser>()?;
    m.add_class::<PyEngine>()?;
    m.add("ParseError", m.py().get_type::<ParseError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_engine_normalizes_with_profile_and_packages() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let engine_cls = module.getattr("Engine").unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs
                .set_item("packages", vec!["base", "physics"])
                .unwrap();
            kwargs.set_item("profile", "authoring").unwrap();
            let engine = engine_cls.call((), Some(&kwargs)).unwrap();

            let result = engine
                .call_method1("normalize", (r"\quantity{x}",))
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
    fn python_parser_empty_packages_means_empty_knowledge() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser_cls = module.getattr("Parser").unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("packages", Vec::<String>::new()).unwrap();
            let parser = parser_cls.call((), Some(&kwargs)).unwrap();

            let knows_frac = parser
                .call_method1("knows_command_name", ("frac",))
                .unwrap()
                .extract::<bool>()
                .unwrap();
            assert!(!knows_frac);
        });
    }

    #[test]
    fn python_parser_accepts_context_items() {
        Python::with_gil(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let item = pyo3::types::PyDict::new(py);
            item.set_item("target", "command").unwrap();
            item.set_item("name", "probe").unwrap();
            item.set_item("kind", "prefix").unwrap();
            item.set_item("allowed_mode", "math").unwrap();
            item.set_item("argspec", "m").unwrap();

            let parser_cls = module.getattr("Parser").unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("packages", Vec::<String>::new()).unwrap();
            kwargs.set_item("items", vec![item]).unwrap();
            let parser = parser_cls.call((), Some(&kwargs)).unwrap();

            let config_cls = module.getattr("ParseConfig").unwrap();
            let config_kwargs = pyo3::types::PyDict::new(py);
            config_kwargs.set_item("strict", true).unwrap();
            config_kwargs.set_item("recover", false).unwrap();
            let config = config_cls.call((), Some(&config_kwargs)).unwrap();

            parser
                .call_method1("parse", (r"\probe{x}", config))
                .expect("custom command should parse");
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
