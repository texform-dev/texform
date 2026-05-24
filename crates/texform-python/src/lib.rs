use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::{depythonize, pythonize};
use texform::{
    FlattenGroupsConfig as CoreFlattenGroupsConfig,
    LowerAttributesConfig as CoreLowerAttributesConfig, ParseConfig as CoreParseConfig,
    Profile as CoreProfile, TransformConfig as CoreTransformConfig,
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
    fn to_core(&self) -> CoreParseConfig {
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

    fn to_core(&self) -> CoreFlattenGroupsConfig {
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

    fn to_core(&self) -> CoreTransformConfig {
        CoreTransformConfig {
            lower_attributes_enabled: self.lower_attributes.enabled,
            rewrite_enabled: self.rewrite.enabled,
            flatten_groups: self.flatten_groups.to_core(),
            max_iterations: self.rewrite.max_iterations,
        }
    }
}

fn parse_context(packages: Option<Vec<String>>) -> PyResult<texform::Parser> {
    let mut builder = texform::Parser::builder();
    if let Some(packages) = packages {
        let refs = packages.iter().map(String::as_str).collect::<Vec<_>>();
        builder = if refs.is_empty() {
            builder.empty_knowledge()
        } else {
            builder.packages(refs.as_slice())
        };
    }
    builder
        .build()
        .map_err(|error| ParseError::new_err(error.to_string()))
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

fn py_optional_bool(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<bool>> {
    Ok(match dict.get_item(key)? {
        Some(value) if !value.is_none() => Some(value.extract::<bool>()?),
        _ => None,
    })
}

fn py_optional_usize(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<usize>> {
    Ok(match dict.get_item(key)? {
        Some(value) if !value.is_none() => Some(value.extract::<usize>()?),
        _ => None,
    })
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

fn allowed_mode_to_str(value: texform::AllowedMode) -> &'static str {
    match value {
        texform::AllowedMode::Math => "math",
        texform::AllowedMode::Text => "text",
        texform::AllowedMode::Both => "both",
    }
}

fn command_kind_to_str(value: texform::CommandKind) -> &'static str {
    match value {
        texform::CommandKind::Prefix => "prefix",
        texform::CommandKind::Infix => "infix",
        texform::CommandKind::Declarative => "declarative",
    }
}

fn content_mode_to_str(value: texform::ContentMode) -> &'static str {
    match value {
        texform::ContentMode::Math => "math",
        texform::ContentMode::Text => "text",
    }
}

fn command_record_to_json(record: &texform::ActiveCommandRecord) -> serde_json::Value {
    serde_json::json!({
        "name": record.name,
        "kind": command_kind_to_str(record.kind),
        "allowed_mode": allowed_mode_to_str(record.allowed_mode),
        "spec_string": record.argspec.source,
        "from_packages": record.from_packages,
        "tags": record.tags,
        "args": record.argspec.args.iter().map(|spec| serde_json::json!({
            "required": spec.required,
            "no_leading_space": spec.no_leading_space,
            "nullable": spec.nullable,
        })).collect::<Vec<_>>(),
    })
}

fn env_record_to_json(record: &texform::ActiveEnvironmentRecord) -> serde_json::Value {
    serde_json::json!({
        "name": record.name,
        "allowed_mode": allowed_mode_to_str(record.allowed_mode),
        "body_mode": content_mode_to_str(record.body_mode),
        "spec_string": record.argspec.source,
        "from_packages": record.from_packages,
        "tags": record.tags,
        "args": record.argspec.args.iter().map(|spec| serde_json::json!({
            "required": spec.required,
            "no_leading_space": spec.no_leading_space,
            "nullable": spec.nullable,
        })).collect::<Vec<_>>(),
    })
}

fn character_record_to_json(record: &texform::ActiveCharacterRecord) -> serde_json::Value {
    serde_json::json!({
        "name": record.name,
        "allowed_mode": allowed_mode_to_str(record.allowed_mode),
        "unicode_value": record.unicode_value,
        "attributes": {
            "mathvariant": record.attributes.mathvariant,
        },
        "package": record.package,
    })
}

fn py_context_item(py: Python<'_>, item: &Py<PyAny>) -> PyResult<texform::ContextItem> {
    let value = item.bind(py);
    let dict = value
        .cast::<PyDict>()
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

fn apply_parse_config_dict(config: &mut CoreParseConfig, dict: &Bound<'_, PyDict>) -> PyResult<()> {
    if let Some(strict) = py_optional_bool(dict, "strict")? {
        config.strict = strict;
    }
    if let Some(recover) = py_optional_bool(dict, "recover")? {
        config.recover = recover;
    }
    if let Some(max_group_depth) = py_optional_usize(dict, "max_group_depth")? {
        config.max_group_depth = max_group_depth;
    }
    Ok(())
}

fn parse_config_from_python(
    config: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
    default: CoreParseConfig,
) -> PyResult<Option<CoreParseConfig>> {
    if config.is_none() && kwargs.map(|dict| dict.len()).unwrap_or(0) == 0 {
        return Ok(None);
    }

    let mut parsed = default;
    if let Some(value) = config {
        if value.is_none() {
            // Keep the default.
        } else if let Ok(config) = value.extract::<PyRef<'_, PyParseConfig>>() {
            parsed = config.to_core();
        } else {
            let dict = value
                .cast::<PyDict>()
                .map_err(|_| ParseError::new_err("config must be a ParseConfig or dict"))?;
            apply_parse_config_dict(&mut parsed, dict)?;
        }
    }
    if let Some(kwargs) = kwargs {
        apply_parse_config_dict(&mut parsed, kwargs)?;
    }
    Ok(Some(parsed))
}

fn apply_flatten_groups_dict(
    config: &mut CoreFlattenGroupsConfig,
    dict: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if let Some(value) = py_optional_bool(dict, "enabled")? {
        config.enabled = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_containing_declarative_command")? {
        config.preserve_group_containing_declarative_command = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_in_script_base_slot")? {
        config.preserve_group_in_script_base_slot = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_inside_env_body")? {
        config.preserve_group_inside_env_body = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_containing_infix")? {
        config.preserve_group_containing_infix = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_adjacent_to_command_like")? {
        config.preserve_group_adjacent_to_command_like = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_as_argument_of_command")? {
        config.preserve_group_as_argument_of_command = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_after_scripted_command_like")? {
        config.preserve_group_after_scripted_command_like = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_empty_group")? {
        config.preserve_empty_group = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_with_lone_atom_spacing_char")? {
        config.preserve_group_with_lone_atom_spacing_char = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_starting_with_atom_spacing_char")? {
        config.preserve_group_starting_with_atom_spacing_char = value;
    }
    if let Some(value) = py_optional_bool(dict, "preserve_group_containing_delimited_pair")? {
        config.preserve_group_containing_delimited_pair = value;
    }
    Ok(())
}

fn apply_normalize_config_dict(
    config: &mut texform::NormalizeConfig,
    dict: &Bound<'_, PyDict>,
) -> PyResult<()> {
    apply_parse_config_dict(&mut config.parse, dict)?;
    if let Some(value) = py_optional_bool(dict, "rewrite_enabled")? {
        config.transform.rewrite_enabled = value;
    }
    if let Some(value) = py_optional_bool(dict, "lower_attributes_enabled")? {
        config.transform.lower_attributes_enabled = value;
    }
    if let Some(value) = py_optional_usize(dict, "max_iterations")? {
        config.transform.max_iterations = value;
    }
    if let Some(flatten_groups) = dict.get_item("flatten_groups")?
        && !flatten_groups.is_none()
    {
        if let Ok(flatten_groups) = flatten_groups.extract::<PyRef<'_, PyFlattenGroupsConfig>>() {
            config.transform.flatten_groups = flatten_groups.to_core();
        } else {
            let dict = flatten_groups.cast::<PyDict>().map_err(|_| {
                ParseError::new_err("flatten_groups must be a FlattenGroupsConfig or dict")
            })?;
            apply_flatten_groups_dict(&mut config.transform.flatten_groups, dict)?;
        }
    }
    if let Some(parse_config) = dict.get_item("parse_config")?
        && !parse_config.is_none()
        && let Some(parsed) =
            parse_config_from_python(Some(&parse_config), None, config.parse.clone())?
    {
        config.parse = parsed;
    }
    Ok(())
}

fn normalize_config_from_python(
    config: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
    default: texform::NormalizeConfig,
) -> PyResult<Option<texform::NormalizeConfig>> {
    if config.is_none() && kwargs.map(|dict| dict.len()).unwrap_or(0) == 0 {
        return Ok(None);
    }

    let mut parsed = default;
    if let Some(value) = config {
        if value.is_none() {
            // Keep the default.
        } else if let Ok(transform) = value.extract::<PyRef<'_, PyTransformConfig>>() {
            parsed.transform = transform.to_core();
        } else {
            let dict = value
                .cast::<PyDict>()
                .map_err(|_| ParseError::new_err("config must be a TransformConfig or dict"))?;
            apply_normalize_config_dict(&mut parsed, dict)?;
        }
    }
    if let Some(kwargs) = kwargs {
        apply_normalize_config_dict(&mut parsed, kwargs)?;
    }
    Ok(Some(parsed))
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
    #[allow(clippy::too_many_arguments)]
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

    #[pyo3(signature = (src, config = None, **kwargs))]
    fn parse(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let output = match parse_config_from_python(config, kwargs, CoreParseConfig::default())? {
            Some(config) => self.inner.parse_with(src, &config),
            None => self.inner.parse(src),
        };
        parse_output_to_python(py, output)
    }

    fn lookup_command(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self.inner.lookup_command(name, py_content_mode(mode)?) {
                Some(record) => pythonize(py, &command_record_to_json(record))?.unbind(),
                None => py.None(),
            },
        )
    }

    fn lookup_explicit_command(
        &self,
        py: Python<'_>,
        name: &str,
        mode: &str,
    ) -> PyResult<Py<PyAny>> {
        Ok(
            match self
                .inner
                .lookup_explicit_command(name, py_content_mode(mode)?)
            {
                Some(record) => pythonize(py, &command_record_to_json(record))?.unbind(),
                None => py.None(),
            },
        )
    }

    fn lookup_character(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self.inner.lookup_character(name, py_content_mode(mode)?) {
                Some(record) => pythonize(py, &character_record_to_json(record))?.unbind(),
                None => py.None(),
            },
        )
    }

    fn lookup_env(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(match self.inner.lookup_env(name, py_content_mode(mode)?) {
            Some(record) => pythonize(py, &env_record_to_json(record))?.unbind(),
            None => py.None(),
        })
    }

    fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    fn knows_command_name(&self, name: &str) -> bool {
        self.inner.knows_command_name(name)
    }

    fn knows_env_name(&self, name: &str) -> bool {
        self.inner.knows_env_name(name)
    }

    fn knows_character_name(&self, name: &str) -> bool {
        self.inner.knows_character_name(name)
    }
}

#[pyclass(name = "Engine")]
struct PyEngine {
    inner: texform::Engine,
}

#[pymethods]
impl PyEngine {
    #[allow(clippy::too_many_arguments)]
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
            builder = builder
                .disable_rule_by_name(&rule)
                .map_err(|error| ParseError::new_err(error.to_string()))?;
        }
        Ok(Self {
            inner: builder
                .build()
                .map_err(|error| ParseError::new_err(error.to_string()))?,
        })
    }

    #[pyo3(signature = (src, config = None, **kwargs))]
    fn normalize(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let default = texform::NormalizeConfig {
            parse: CoreParseConfig::STRICT_NO_RECOVER,
            transform: *self.inner.default_transform_config(),
        };
        let result = match normalize_config_from_python(config, kwargs, default)? {
            Some(config) => self.inner.normalize_with(src, &config),
            None => self.inner.normalize(src),
        }
        .map_err(|error| ParseError::new_err(error.to_string()))?;
        transform_result_to_python(py, result.normalized, &result.report)
    }

    #[pyo3(signature = (src, config = None, **kwargs))]
    fn parse(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let output = match parse_config_from_python(config, kwargs, CoreParseConfig::default())? {
            Some(config) => self.inner.parse_with(src, &config),
            None => self.inner.parse(src),
        };
        parse_output_to_python(py, output)
    }

    fn lookup_command(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self.inner.lookup_command(name, py_content_mode(mode)?) {
                Some(record) => pythonize(py, &command_record_to_json(record))?.unbind(),
                None => py.None(),
            },
        )
    }

    fn lookup_explicit_command(
        &self,
        py: Python<'_>,
        name: &str,
        mode: &str,
    ) -> PyResult<Py<PyAny>> {
        Ok(
            match self
                .inner
                .lookup_explicit_command(name, py_content_mode(mode)?)
            {
                Some(record) => pythonize(py, &command_record_to_json(record))?.unbind(),
                None => py.None(),
            },
        )
    }

    fn lookup_character(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self.inner.lookup_character(name, py_content_mode(mode)?) {
                Some(record) => pythonize(py, &character_record_to_json(record))?.unbind(),
                None => py.None(),
            },
        )
    }

    fn lookup_env(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(match self.inner.lookup_env(name, py_content_mode(mode)?) {
            Some(record) => pythonize(py, &env_record_to_json(record))?.unbind(),
            None => py.None(),
        })
    }

    fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    fn knows_command_name(&self, name: &str) -> bool {
        self.inner.knows_command_name(name)
    }

    fn knows_env_name(&self, name: &str) -> bool {
        self.inner.knows_env_name(name)
    }

    fn knows_character_name(&self, name: &str) -> bool {
        self.inner.knows_character_name(name)
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

fn value_object<'a>(
    value: &'a serde_json::Value,
    context: &str,
) -> PyResult<&'a serde_json::Map<String, serde_json::Value>> {
    value
        .as_object()
        .ok_or_else(|| ParseError::new_err(format!("{context} must be an object")))
}

fn single_variant<'a>(
    value: &'a serde_json::Value,
    context: &str,
) -> PyResult<(&'a str, &'a serde_json::Value)> {
    let object = value_object(value, context)?;
    if object.len() != 1 {
        return Err(ParseError::new_err(format!(
            "{context} must contain exactly one variant"
        )));
    }
    let (key, value) = object.iter().next().expect("object is non-empty");
    Ok((key.as_str(), value))
}

fn json_string(value: &serde_json::Value, context: &str) -> PyResult<String> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| ParseError::new_err(format!("{context} must be a string")))
}

fn json_bool(value: &serde_json::Value, context: &str) -> PyResult<bool> {
    value
        .as_bool()
        .ok_or_else(|| ParseError::new_err(format!("{context} must be a boolean")))
}

fn json_array<'a>(
    value: &'a serde_json::Value,
    context: &str,
) -> PyResult<&'a Vec<serde_json::Value>> {
    value
        .as_array()
        .ok_or_else(|| ParseError::new_err(format!("{context} must be an array")))
}

fn json_field<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    context: &str,
) -> PyResult<&'a serde_json::Value> {
    object
        .get(key)
        .ok_or_else(|| ParseError::new_err(format!("{context} missing `{key}`")))
}

fn syntax_mode_from_json(value: &serde_json::Value) -> PyResult<texform::ContentMode> {
    match json_string(value, "mode")?.as_str() {
        "Math" | "math" => Ok(texform::ContentMode::Math),
        "Text" | "text" => Ok(texform::ContentMode::Text),
        other => Err(ParseError::new_err(format!(
            "unsupported content mode: {other}"
        ))),
    }
}

fn delimiter_from_json(value: &serde_json::Value) -> PyResult<texform::Delimiter> {
    if let Some(text) = value.as_str() {
        return match text {
            "None" => Ok(texform::Delimiter::None),
            other => Err(ParseError::new_err(format!(
                "unsupported delimiter: {other}"
            ))),
        };
    }
    let (variant, data) = single_variant(value, "delimiter")?;
    match variant {
        "Char" => {
            let text = json_string(data, "delimiter char")?;
            let ch = text
                .chars()
                .next()
                .ok_or_else(|| ParseError::new_err("delimiter char cannot be empty"))?;
            Ok(texform::Delimiter::Char(ch))
        }
        "Control" => {
            let text = json_string(data, "delimiter control")?;
            let leaked: &'static str = Box::leak(text.into_boxed_str());
            Ok(texform::Delimiter::Control(leaked))
        }
        other => Err(ParseError::new_err(format!(
            "unsupported delimiter: {other}"
        ))),
    }
}

fn group_kind_from_json(value: &serde_json::Value) -> PyResult<texform::GroupKind> {
    if let Some(text) = value.as_str() {
        return match text {
            "Explicit" => Ok(texform::GroupKind::Explicit),
            "Implicit" => Ok(texform::GroupKind::Implicit),
            "InlineMath" => Ok(texform::GroupKind::InlineMath),
            other => Err(ParseError::new_err(format!(
                "unsupported group kind: {other}"
            ))),
        };
    }
    let (variant, data) = single_variant(value, "group kind")?;
    match variant {
        "Delimited" => {
            let object = value_object(data, "delimited group kind")?;
            Ok(texform::GroupKind::Delimited {
                left: delimiter_from_json(json_field(object, "left", "delimited group kind")?)?,
                right: delimiter_from_json(json_field(object, "right", "delimited group kind")?)?,
            })
        }
        other => Err(ParseError::new_err(format!(
            "unsupported group kind: {other}"
        ))),
    }
}

fn argument_kind_from_json(value: &serde_json::Value) -> PyResult<texform::ArgumentKind> {
    if let Some(text) = value.as_str() {
        return match text {
            "Mandatory" => Ok(texform::ArgumentKind::Mandatory),
            "Optional" => Ok(texform::ArgumentKind::Optional),
            "Star" => Ok(texform::ArgumentKind::Star),
            "Group" => Ok(texform::ArgumentKind::Group),
            other => Err(ParseError::new_err(format!(
                "unsupported argument kind: {other}"
            ))),
        };
    }
    let (variant, data) = single_variant(value, "argument kind")?;
    let object = value_object(data, "argument delimiter kind")?;
    match variant {
        "Delimited" => Ok(texform::ArgumentKind::Delimited {
            open: delimiter_from_json(json_field(object, "open", "argument kind")?)?,
            close: delimiter_from_json(json_field(object, "close", "argument kind")?)?,
        }),
        "Paired" => Ok(texform::ArgumentKind::Paired {
            open: delimiter_from_json(json_field(object, "open", "argument kind")?)?,
            close: delimiter_from_json(json_field(object, "close", "argument kind")?)?,
        }),
        other => Err(ParseError::new_err(format!(
            "unsupported argument kind: {other}"
        ))),
    }
}

fn argument_value_from_json(value: &serde_json::Value) -> PyResult<texform::ArgumentValue> {
    let (variant, data) = single_variant(value, "argument value")?;
    match variant {
        "MathContent" => Ok(texform::ArgumentValue::MathContent(syntax_node_from_json(
            data,
        )?)),
        "TextContent" => Ok(texform::ArgumentValue::TextContent(syntax_node_from_json(
            data,
        )?)),
        "Delimiter" => Ok(texform::ArgumentValue::Delimiter(delimiter_from_json(
            data,
        )?)),
        "CSName" => Ok(texform::ArgumentValue::CSName(json_string(data, "csname")?)),
        "Dimension" => Ok(texform::ArgumentValue::Dimension(json_string(
            data,
            "dimension",
        )?)),
        "Integer" => Ok(texform::ArgumentValue::Integer(json_string(
            data, "integer",
        )?)),
        "KeyVal" => Ok(texform::ArgumentValue::KeyVal(json_string(data, "keyval")?)),
        "Column" => Ok(texform::ArgumentValue::Column(json_string(data, "column")?)),
        "Boolean" => Ok(texform::ArgumentValue::Boolean(json_bool(data, "boolean")?)),
        other => Err(ParseError::new_err(format!(
            "unsupported argument value: {other}"
        ))),
    }
}

fn argument_from_json(value: &serde_json::Value) -> PyResult<texform::Argument> {
    let object = value_object(value, "argument")?;
    Ok(texform::Argument::from_value(
        argument_kind_from_json(json_field(object, "kind", "argument")?)?,
        argument_value_from_json(json_field(object, "value", "argument")?)?,
    ))
}

fn argument_slots_from_json(value: &serde_json::Value) -> PyResult<Vec<texform::ArgumentSlot>> {
    json_array(value, "argument slots")?
        .iter()
        .map(|slot| {
            if slot.is_null() {
                Ok(None)
            } else {
                Ok(Some(argument_from_json(slot)?))
            }
        })
        .collect()
}

fn syntax_node_from_json(value: &serde_json::Value) -> PyResult<texform::SyntaxNode> {
    if value.as_str() == Some("ActiveSpace") {
        return Ok(texform::SyntaxNode::ActiveSpace);
    }
    let (variant, data) = single_variant(value, "syntax node")?;
    match variant {
        "Root" => {
            let object = value_object(data, "root node")?;
            Ok(texform::SyntaxNode::Root {
                mode: syntax_mode_from_json(json_field(object, "mode", "root node")?)?,
                children: json_array(json_field(object, "children", "root node")?, "children")?
                    .iter()
                    .map(syntax_node_from_json)
                    .collect::<PyResult<_>>()?,
            })
        }
        "Group" => {
            let object = value_object(data, "group node")?;
            Ok(texform::SyntaxNode::Group {
                mode: syntax_mode_from_json(json_field(object, "mode", "group node")?)?,
                kind: group_kind_from_json(json_field(object, "kind", "group node")?)?,
                children: json_array(json_field(object, "children", "group node")?, "children")?
                    .iter()
                    .map(syntax_node_from_json)
                    .collect::<PyResult<_>>()?,
            })
        }
        "Command" => {
            let object = value_object(data, "command node")?;
            Ok(texform::SyntaxNode::Command {
                name: json_string(json_field(object, "name", "command node")?, "command name")?,
                args: argument_slots_from_json(json_field(object, "args", "command node")?)?,
                known: json_bool(
                    json_field(object, "known", "command node")?,
                    "command known",
                )?,
            })
        }
        "Infix" => {
            let object = value_object(data, "infix node")?;
            Ok(texform::SyntaxNode::Infix {
                name: json_string(json_field(object, "name", "infix node")?, "infix name")?,
                args: argument_slots_from_json(json_field(object, "args", "infix node")?)?,
                left: Box::new(syntax_node_from_json(json_field(
                    object,
                    "left",
                    "infix node",
                )?)?),
                right: Box::new(syntax_node_from_json(json_field(
                    object,
                    "right",
                    "infix node",
                )?)?),
            })
        }
        "Declarative" => {
            let object = value_object(data, "declarative node")?;
            Ok(texform::SyntaxNode::Declarative {
                name: json_string(
                    json_field(object, "name", "declarative node")?,
                    "declarative name",
                )?,
                args: argument_slots_from_json(json_field(object, "args", "declarative node")?)?,
            })
        }
        "Environment" => {
            let object = value_object(data, "environment node")?;
            Ok(texform::SyntaxNode::Environment {
                name: json_string(
                    json_field(object, "name", "environment node")?,
                    "environment name",
                )?,
                args: argument_slots_from_json(json_field(object, "args", "environment node")?)?,
                known: json_bool(
                    json_field(object, "known", "environment node")?,
                    "environment known",
                )?,
                body: Box::new(syntax_node_from_json(json_field(
                    object,
                    "body",
                    "environment node",
                )?)?),
            })
        }
        "Scripted" => {
            let object = value_object(data, "scripted node")?;
            Ok(texform::SyntaxNode::Scripted {
                base: Box::new(syntax_node_from_json(json_field(
                    object,
                    "base",
                    "scripted node",
                )?)?),
                subscript: match object.get("subscript") {
                    Some(value) if !value.is_null() => {
                        Some(Box::new(syntax_node_from_json(value)?))
                    }
                    _ => None,
                },
                superscript: match object.get("superscript") {
                    Some(value) if !value.is_null() => {
                        Some(Box::new(syntax_node_from_json(value)?))
                    }
                    _ => None,
                },
            })
        }
        "Error" => {
            let object = value_object(data, "error node")?;
            Ok(texform::SyntaxNode::Error {
                message: json_string(
                    json_field(object, "message", "error node")?,
                    "error message",
                )?,
                snippet: json_string(
                    json_field(object, "snippet", "error node")?,
                    "error snippet",
                )?,
            })
        }
        "Text" => Ok(texform::SyntaxNode::Text(json_string(data, "text node")?)),
        "Char" => {
            let text = json_string(data, "char node")?;
            let ch = text
                .chars()
                .next()
                .ok_or_else(|| ParseError::new_err("char node cannot be empty"))?;
            Ok(texform::SyntaxNode::Char(ch))
        }
        other => Err(ParseError::new_err(format!(
            "unsupported syntax node: {other}"
        ))),
    }
}

fn serialize_options_from_python(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<texform::SerializeOptions> {
    match options {
        Some(value) if !value.is_none() => depythonize(value)
            .map_err(|error| ParseError::new_err(format!("invalid serialize options: {error}"))),
        _ => Ok(texform::SerializeOptions::default()),
    }
}

#[pyfunction]
#[pyo3(signature = (node, options = None))]
fn serialize(node: &Bound<'_, PyAny>, options: Option<&Bound<'_, PyAny>>) -> PyResult<String> {
    let value: serde_json::Value = depythonize(node)
        .map_err(|error| ParseError::new_err(format!("invalid syntax node: {error}")))?;
    let node = syntax_node_from_json(&value)?;
    let options = serialize_options_from_python(options)?;
    texform::serialize_with(&node, &options).map_err(|error| ParseError::new_err(error.to_string()))
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
    let counts = match config {
        Some(config) => texform::analysis::count_targets_with(&ctx, src, &config.to_core()),
        None => texform::analysis::count_targets(&ctx, src),
    }
    .map_err(|error| ParseError::new_err(error.to_string()))?;
    Ok(pythonize(py, &counts)?.unbind())
}

/// Native extension module loaded as `texform._native`.
/// Symbols are re-exported from the Python package's `__init__.py`.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(count_targets, m)?)?;
    m.add_function(wrap_pyfunction!(serialize, m)?)?;
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
        Python::attach(|py| {
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
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();

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
        Python::attach(|py| {
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
        Python::attach(|py| {
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
    fn python_parser_none_config_uses_facade_default() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser_cls = module.getattr("Parser").unwrap();
            let parser = parser_cls.call0().unwrap();

            parser
                .call_method1("parse", (r"\unknowncmd",))
                .expect("non-strict facade default should preserve unknown commands");
        });
    }

    #[test]
    fn python_parser_supports_dict_and_kwarg_config_overrides() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser_cls = module.getattr("Parser").unwrap();
            let parser = parser_cls.call0().unwrap();

            let config = pyo3::types::PyDict::new(py);
            config.set_item("strict", true).unwrap();
            let error = parser
                .call_method1("parse", (r"\unknowncmd", config))
                .expect_err("strict dict config should reject unknown command");
            assert!(error.is_instance_of::<ParseError>(py));

            let config = pyo3::types::PyDict::new(py);
            config.set_item("strict", true).unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("config", config).unwrap();
            kwargs.set_item("strict", false).unwrap();
            parser
                .call_method("parse", (r"\unknowncmd",), Some(&kwargs))
                .expect("kwargs should override config dict");
        });
    }

    #[test]
    fn python_parser_and_engine_expose_metadata_queries() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            assert!(
                parser
                    .call_method1("lookup_command", ("frac", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method1("lookup_explicit_command", ("frac", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method1("lookup_character", ("le", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method1("lookup_env", ("array", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method1("is_delimiter_control", ("lbrace",))
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert!(
                parser
                    .call_method1("knows_env_name", ("array",))
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert!(
                parser
                    .call_method1("knows_character_name", ("le",))
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "authoring").unwrap();
            let engine = module
                .getattr("Engine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();
            assert!(
                engine
                    .call_method1("lookup_command", ("frac", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                engine
                    .call_method1("knows_command_name", ("frac",))
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
        });
    }

    #[test]
    fn python_engine_parse_and_normalize_use_facade_defaults_without_config() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "authoring").unwrap();
            kwargs
                .set_item("packages", vec!["base", "physics"])
                .unwrap();
            let engine = module
                .getattr("Engine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            engine
                .call_method1("parse", (r"\unknowncmd",))
                .expect("parse should use non-strict facade default");
            let result = engine
                .call_method1("normalize", (r"\quantity{x}",))
                .expect("normalize should use facade default");
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
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
    fn python_engine_normalize_kwargs_override_config_dict() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "authoring").unwrap();
            kwargs
                .set_item("packages", vec!["base", "physics"])
                .unwrap();
            let engine = module
                .getattr("Engine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let config = pyo3::types::PyDict::new(py);
            config.set_item("rewrite_enabled", true).unwrap();
            let call_kwargs = pyo3::types::PyDict::new(py);
            call_kwargs.set_item("config", config).unwrap();
            call_kwargs.set_item("rewrite_enabled", false).unwrap();
            call_kwargs
                .set_item("lower_attributes_enabled", false)
                .unwrap();
            let flatten_groups = pyo3::types::PyDict::new(py);
            flatten_groups.set_item("enabled", false).unwrap();
            call_kwargs
                .set_item("flatten_groups", flatten_groups)
                .unwrap();

            let result = engine
                .call_method("normalize", (r"\quantity{x}",), Some(&call_kwargs))
                .expect("normalize should accept kwargs");
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
            assert_eq!(
                dict.get_item("normalized")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"\quantity { x }"
            );
        });
    }

    #[test]
    fn python_module_serializes_parsed_node() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let parsed = parser.call_method1("parse", (r"\frac{a}{b}",)).unwrap();
            let node = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("node")
                .unwrap()
                .unwrap();
            let text = module
                .getattr("serialize")
                .unwrap()
                .call1((node,))
                .unwrap()
                .extract::<String>()
                .unwrap();
            assert_eq!(text, r"\frac { a } { b }");
        });
    }

    #[test]
    fn python_module_corpus_drop_disables_spacing_guards() {
        Python::attach(|py| {
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
        Python::attach(|py| {
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
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let result = module
                .getattr("count_targets")
                .unwrap()
                .call1((r"\frac{a}{b} \le c",))
                .unwrap();
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();

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
