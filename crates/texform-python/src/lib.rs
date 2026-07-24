use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pythonize::{depythonize, pythonize};
use texform::{
    FinalizeAstConfig as CoreFinalizeAstConfig, FlattenGroupsConfig as CoreFlattenGroupsConfig,
    LowerAttributesConfig as CoreLowerAttributesConfig, ParseConfig as CoreParseConfig,
    Profile as CoreProfile, TransformConfig as CoreTransformConfig,
};

pyo3::create_exception!(texform, TexformError, PyException);
pyo3::create_exception!(texform, ParseError, TexformError);
pyo3::create_exception!(texform, EditError, TexformError);
pyo3::create_exception!(texform, ConfigError, TexformError);
pyo3::create_exception!(texform, TransformError, TexformError);

#[pyclass(name = "ParseConfig")]
#[derive(Clone, Debug)]
struct PyParseConfig {
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
            self.reject_unknown, self.abort_on_error, self.max_group_depth
        )
    }
}

impl PyParseConfig {
    fn to_core(&self) -> CoreParseConfig {
        CoreParseConfig {
            reject_unknown: self.reject_unknown,
            abort_on_error: self.abort_on_error,
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

#[pyclass(name = "FinalizeAstConfig")]
#[derive(Clone, Debug)]
struct PyFinalizeAstConfig {
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
        format!("FinalizeAstConfig(enabled={})", self.enabled)
    }
}

impl PyFinalizeAstConfig {
    fn from_core(config: CoreFinalizeAstConfig) -> Self {
        Self {
            enabled: config.enabled,
        }
    }

    fn to_core(&self) -> CoreFinalizeAstConfig {
        CoreFinalizeAstConfig {
            enabled: self.enabled,
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
    finalize_ast: PyFinalizeAstConfig,
    #[pyo3(get, set)]
    flatten_groups: PyFlattenGroupsConfig,
}

#[pymethods]
impl PyTransformConfig {
    #[new]
    #[pyo3(signature = (lower_attributes = None, rewrite = None, finalize_ast = None, flatten_groups = None))]
    fn new(
        lower_attributes: Option<PyLowerAttributesConfig>,
        rewrite: Option<PyRewriteConfig>,
        finalize_ast: Option<PyFinalizeAstConfig>,
        flatten_groups: Option<PyFlattenGroupsConfig>,
    ) -> Self {
        let default = CoreProfile::Authoring.default_transform_config();
        Self {
            lower_attributes: lower_attributes.unwrap_or_else(|| {
                PyLowerAttributesConfig::from_core(CoreLowerAttributesConfig::ENABLED)
            }),
            rewrite: rewrite.unwrap_or_else(|| {
                PyRewriteConfig::from_core(default.rewrite_enabled, default.max_iterations)
            }),
            finalize_ast: finalize_ast
                .unwrap_or_else(|| PyFinalizeAstConfig::from_core(default.finalize_ast)),
            flatten_groups: flatten_groups
                .unwrap_or_else(|| PyFlattenGroupsConfig::from_core(default.flatten_groups)),
        }
    }

    #[classmethod]
    fn authoring(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::Authoring)
    }

    #[classmethod]
    fn faithful(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::Faithful)
    }

    #[classmethod]
    fn corpus(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::Corpus)
    }

    #[classmethod]
    fn equiv(_cls: &Bound<'_, pyo3::types::PyType>) -> Self {
        Self::from_profile(CoreProfile::Equiv)
    }

    fn __repr__(&self) -> String {
        format!(
            "TransformConfig(lower_attributes={:?}, rewrite={:?}, finalize_ast={:?}, flatten_groups={:?})",
            self.lower_attributes, self.rewrite, self.finalize_ast, self.flatten_groups
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
            finalize_ast: PyFinalizeAstConfig::from_core(config.finalize_ast),
            flatten_groups: PyFlattenGroupsConfig::from_core(config.flatten_groups),
        }
    }

    fn to_core(&self) -> CoreTransformConfig {
        let mut config = CoreProfile::Authoring.default_transform_config();
        config.lower_attributes_enabled = self.lower_attributes.enabled;
        config.rewrite_enabled = self.rewrite.enabled;
        config.finalize_ast = self.finalize_ast.to_core();
        config.flatten_groups = self.flatten_groups.to_core();
        config.max_iterations = self.rewrite.max_iterations;
        config
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
        .map_err(|error| ConfigError::new_err(error.to_string()))
}

fn profile_from_name(name: &str) -> PyResult<texform::Profile> {
    match name {
        "authoring" => Ok(texform::Profile::Authoring),
        "faithful" => Ok(texform::Profile::Faithful),
        "corpus" => Ok(texform::Profile::Corpus),
        "equiv" => Ok(texform::Profile::Equiv),
        other => Err(ConfigError::new_err(format!(
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
        other => Err(ConfigError::new_err(format!(
            "unsupported command kind: {other}"
        ))),
    }
}

fn py_allowed_mode(value: &str) -> PyResult<texform::AllowedMode> {
    match value {
        "math" => Ok(texform::AllowedMode::Math),
        "text" => Ok(texform::AllowedMode::Text),
        "both" => Ok(texform::AllowedMode::Both),
        other => Err(ConfigError::new_err(format!(
            "unsupported allowed mode: {other}"
        ))),
    }
}

fn py_content_mode(value: &str) -> PyResult<texform::ContentMode> {
    match value {
        "math" => Ok(texform::ContentMode::Math),
        "text" => Ok(texform::ContentMode::Text),
        other => Err(ConfigError::new_err(format!(
            "unsupported content mode: {other}"
        ))),
    }
}

fn content_mode_to_str(value: texform::ContentMode) -> &'static str {
    match value {
        texform::ContentMode::Math => "math",
        texform::ContentMode::Text => "text",
    }
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
    mut builder: texform::TransformEngineBuilder,
    packages: Option<Vec<String>>,
    items: Option<Vec<Py<PyAny>>>,
    remove_commands: Option<Vec<String>>,
    remove_environments: Option<Vec<String>>,
    remove_delimiter_controls: Option<Vec<String>>,
) -> PyResult<texform::TransformEngineBuilder> {
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

fn borrow_error(error: impl std::fmt::Display) -> PyErr {
    EditError::new_err(format!("document borrow conflict: {error}"))
}

fn edit_error(error: impl std::fmt::Display) -> PyErr {
    EditError::new_err(error.to_string())
}

fn binding_error_to_py(
    py: Python<'_>,
    error: texform::bindings::BindingErrorDto,
) -> PyResult<PyErr> {
    match error.kind {
        "parse" => parse_error_to_py(py, error, None),
        "edit" => Ok(EditError::new_err(error.message)),
        "config" => Ok(ConfigError::new_err(error.message)),
        "transform" => Ok(TransformError::new_err(error.message)),
        _ => Ok(TexformError::new_err(error.message)),
    }
}

fn binding_error_parts_to_py(
    py: Python<'_>,
    parts: texform::bindings::BindingErrorParts,
) -> PyResult<PyErr> {
    match parts.error.kind {
        "parse" => parse_error_to_py(py, parts.error, parts.document),
        "edit" => Ok(EditError::new_err(parts.error.message)),
        "config" => Ok(ConfigError::new_err(parts.error.message)),
        "transform" => Ok(TransformError::new_err(parts.error.message)),
        _ => Ok(TexformError::new_err(parts.error.message)),
    }
}

fn parse_error_to_py(
    py: Python<'_>,
    error: texform::bindings::BindingErrorDto,
    document: Option<texform::Document>,
) -> PyResult<PyErr> {
    let py_error = ParseError::new_err(error.message);
    let value = py_error.value(py);
    value.setattr("diagnostics", pythonize(py, &error.diagnostics)?)?;
    let document = match document {
        Some(inner) => Py::new(py, PyDocument { inner })?.into_any(),
        None => py.None(),
    };
    value.setattr("document", document)?;
    Ok(py_error)
}

fn parse_result_to_python(py: Python<'_>, result: texform::ParseResult) -> PyResult<Py<PyAny>> {
    let (document, diagnostics) = result.into_parts();
    let out = PyDict::new(py);
    let document = match document {
        Some(inner) => Py::new(py, PyDocument { inner })?.into_any(),
        None => py.None(),
    };
    out.set_item("document", document)?;
    out.set_item("diagnostics", pythonize(py, &diagnostics)?)?;
    Ok(out.unbind().into_any())
}

fn py_node(py: Python<'_>, doc: Py<PyDocument>, id: texform::NodeId) -> PyResult<Py<PyNode>> {
    Py::new(py, PyNode { doc, id })
}

fn py_optional_node(
    py: Python<'_>,
    doc: Py<PyDocument>,
    id: Option<texform::NodeId>,
) -> PyResult<Py<PyAny>> {
    match id {
        Some(id) => Ok(py_node(py, doc, id)?.into_any()),
        None => Ok(py.None()),
    }
}

fn py_nodes_list(
    py: Python<'_>,
    doc: &Py<PyDocument>,
    ids: Vec<texform::NodeId>,
) -> PyResult<Py<PyAny>> {
    let out = PyList::empty(py);
    for id in ids {
        out.append(py_node(py, doc.clone_ref(py), id)?)?;
    }
    Ok(out.unbind().into_any())
}

fn parse_char(value: &str) -> PyResult<char> {
    let mut chars = value.chars();
    let Some(ch) = chars.next() else {
        return Err(ParseError::new_err("character cannot be empty"));
    };
    if chars.next().is_some() {
        return Err(ParseError::new_err(
            "character must contain exactly one scalar",
        ));
    }
    Ok(ch)
}

fn same_py_document(py: Python<'_>, left: &Py<PyDocument>, right: &Py<PyDocument>) -> bool {
    left.bind(py).is(right.bind(py))
}

fn ensure_same_py_document(
    py: Python<'_>,
    left: &Py<PyDocument>,
    right: &Py<PyDocument>,
) -> PyResult<()> {
    if same_py_document(py, left, right) {
        Ok(())
    } else {
        Err(ParseError::new_err("node belongs to a different document"))
    }
}

fn ensure_node_owner(py: Python<'_>, owner: &Py<PyDocument>, node: &PyNode) -> PyResult<()> {
    ensure_same_py_document(py, owner, &node.doc)
}

fn py_string_property(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    dict.get_item(key)?
        .ok_or_else(|| ParseError::new_err(format!("ArgValue missing `{key}`")))?
        .extract::<String>()
}

fn py_arg_values(
    py: Python<'_>,
    owner: &Py<PyDocument>,
    args: Option<Vec<Py<PyAny>>>,
) -> PyResult<Vec<texform::ArgValue>> {
    args.unwrap_or_default()
        .iter()
        .map(|arg| py_arg_value(py, owner, arg.bind(py)))
        .collect()
}

fn py_arg_value(
    py: Python<'_>,
    owner: &Py<PyDocument>,
    value: &Bound<'_, PyAny>,
) -> PyResult<texform::ArgValue> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| ParseError::new_err("ArgValue must be a dict"))?;
    match py_string_property(dict, "kind")?.as_str() {
        "Math" => Ok(texform::ArgValue::math(py_arg_node(py, owner, dict)?.id)),
        "Text" => Ok(texform::ArgValue::text(py_arg_node(py, owner, dict)?.id)),
        "Delimiter" => Ok(texform::ArgValue::delimiter(py_delimiter_value(
            &dict
                .get_item("value")?
                .ok_or_else(|| ParseError::new_err("Delimiter ArgValue missing `value`"))?,
        )?)),
        "CSName" => Ok(texform::ArgValue::cs_name(py_string_property(
            dict, "value",
        )?)),
        "Dimension" => Ok(texform::ArgValue::dimension(py_string_property(
            dict, "value",
        )?)),
        "Integer" => Ok(texform::ArgValue::integer(py_string_property(
            dict, "value",
        )?)),
        "KeyVal" => Ok(texform::ArgValue::key_val(py_string_property(
            dict, "value",
        )?)),
        "Column" => Ok(texform::ArgValue::column(py_string_property(
            dict, "value",
        )?)),
        "Boolean" => Ok(texform::ArgValue::boolean(
            dict.get_item("value")?
                .ok_or_else(|| ParseError::new_err("Boolean ArgValue missing `value`"))?
                .extract::<bool>()?,
        )),
        other => Err(ParseError::new_err(format!(
            "unsupported ArgValue kind: {other}"
        ))),
    }
}

fn py_arg_node<'py>(
    py: Python<'py>,
    owner: &Py<PyDocument>,
    dict: &Bound<'py, PyDict>,
) -> PyResult<PyRef<'py, PyNode>> {
    let node = dict
        .get_item("node")?
        .ok_or_else(|| ParseError::new_err("content ArgValue missing `node`"))?;
    let node = node.extract::<PyRef<'py, PyNode>>()?;
    ensure_node_owner(py, owner, &node)?;
    Ok(node)
}

fn py_delimiter_value(value: &Bound<'_, PyAny>) -> PyResult<texform::DelimiterValue> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| ParseError::new_err("Delimiter value must be a dict"))?;
    match py_string_property(dict, "kind")?.as_str() {
        "None" => Ok(texform::DelimiterValue::None),
        "Char" => Ok(texform::DelimiterValue::Char(parse_char(
            &py_string_property(dict, "value")?,
        )?)),
        "Control" => Ok(texform::DelimiterValue::Control(py_string_property(
            dict, "value",
        )?)),
        other => Err(ParseError::new_err(format!(
            "unsupported delimiter kind: {other}"
        ))),
    }
}

fn py_delimiter_ref(delimiter: texform::DelimiterRef<'_>) -> serde_json::Value {
    match delimiter {
        texform::DelimiterRef::None => serde_json::json!({ "kind": "None" }),
        texform::DelimiterRef::Char(ch) => {
            serde_json::json!({ "kind": "Char", "value": ch.to_string() })
        }
        texform::DelimiterRef::Control(name) => {
            serde_json::json!({ "kind": "Control", "value": name })
        }
    }
}

fn py_group_kind(kind: texform::GroupKindRef<'_>) -> serde_json::Value {
    match kind {
        texform::GroupKindRef::Explicit => serde_json::json!({ "kind": "Explicit" }),
        texform::GroupKindRef::Implicit => serde_json::json!({ "kind": "Implicit" }),
        texform::GroupKindRef::Delimited { left, right } => serde_json::json!({
            "kind": "Delimited",
            "left": py_delimiter_ref(left),
            "right": py_delimiter_ref(right),
        }),
        texform::GroupKindRef::InlineMath => serde_json::json!({ "kind": "InlineMath" }),
    }
}

fn py_arg_ref(
    py: Python<'_>,
    owner: &Py<PyDocument>,
    arg: texform::ArgRef<'_>,
) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    match arg {
        texform::ArgRef::Math(node) => {
            out.set_item("kind", "Math")?;
            out.set_item("node", py_node(py, owner.clone_ref(py), node.id())?)?;
        }
        texform::ArgRef::Text(node) => {
            out.set_item("kind", "Text")?;
            out.set_item("node", py_node(py, owner.clone_ref(py), node.id())?)?;
        }
        texform::ArgRef::Delimiter(delimiter) => {
            out.set_item("kind", "Delimiter")?;
            out.set_item("value", pythonize(py, &py_delimiter_ref(delimiter))?)?;
        }
        texform::ArgRef::CSName(value) => {
            out.set_item("kind", "CSName")?;
            out.set_item("value", value)?;
        }
        texform::ArgRef::Dimension(value) => {
            out.set_item("kind", "Dimension")?;
            out.set_item("value", value)?;
        }
        texform::ArgRef::Integer(value) => {
            out.set_item("kind", "Integer")?;
            out.set_item("value", value)?;
        }
        texform::ArgRef::KeyVal(value) => {
            out.set_item("kind", "KeyVal")?;
            out.set_item("value", value)?;
        }
        texform::ArgRef::Column(value) => {
            out.set_item("kind", "Column")?;
            out.set_item("value", value)?;
        }
        texform::ArgRef::Boolean(value) => {
            out.set_item("kind", "Boolean")?;
            out.set_item("value", value)?;
        }
    }
    Ok(out.unbind().into_any())
}

#[pyclass(name = "Document")]
struct PyDocument {
    inner: texform::Document,
}

#[pymethods]
impl PyDocument {
    #[new]
    fn new() -> Self {
        Self {
            inner: texform::Document::new(),
        }
    }

    #[staticmethod]
    fn from_syntax(py: Python<'_>, node: &Bound<'_, PyAny>) -> PyResult<Self> {
        let node = depythonize::<texform::SyntaxNode>(node).map_err(|error| {
            parse_error_to_py(
                py,
                texform::bindings::BindingErrorDto {
                    kind: "parse",
                    message: format!("invalid syntax node: {error}"),
                    diagnostics: Vec::new(),
                },
                None,
            )
            .unwrap_or_else(|error| error)
        })?;
        Ok(Self {
            inner: texform::Document::from_syntax(&node).map_err(|error| {
                binding_error_to_py(py, texform::bindings::from_syntax_error_to_dto(error))
                    .unwrap_or_else(|error| error)
            })?,
        })
    }

    fn root(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyNode>> {
        let id = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            document.inner.root().id()
        };
        py_node(py, slf.clone().unbind(), id)
    }

    fn has_errors(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let document = slf.try_borrow().map_err(borrow_error)?;
        Ok(document.inner.has_errors())
    }

    fn is_read_only(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let document = slf.try_borrow().map_err(borrow_error)?;
        Ok(document.inner.is_read_only())
    }

    fn errors(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ids = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            document
                .inner
                .errors()
                .map(|node| node.id())
                .collect::<Vec<_>>()
        };
        py_nodes_list(py, &slf.clone().unbind(), ids)
    }

    fn find_commands(slf: &Bound<'_, Self>, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        let ids = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            document
                .inner
                .find_commands(name)
                .map(|node| node.id())
                .collect::<Vec<_>>()
        };
        py_nodes_list(py, &slf.clone().unbind(), ids)
    }

    fn find_environments(slf: &Bound<'_, Self>, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        let ids = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            document
                .inner
                .find_environments(name)
                .map(|node| node.id())
                .collect::<Vec<_>>()
        };
        py_nodes_list(py, &slf.clone().unbind(), ids)
    }

    fn to_syntax(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let syntax = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            document.inner.to_syntax()
        };
        Ok(pythonize(py, &syntax)?.unbind())
    }

    fn node_spans(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let entries = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            texform::bindings::node_spans_to_dto(&document.inner)
        };
        Ok(pythonize(py, &entries)?.unbind())
    }

    #[pyo3(signature = (options = None))]
    fn to_latex(slf: &Bound<'_, Self>, options: Option<&Bound<'_, PyAny>>) -> PyResult<String> {
        let options = serialize_options_from_python(options)?;
        let document = slf.try_borrow().map_err(borrow_error)?;
        document
            .inner
            .to_latex_with(&options)
            .map_err(|error| TexformError::new_err(error.to_string()))
    }

    #[pyo3(signature = (options = None))]
    fn to_tokenized_latex(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let options = serialize_options_from_python(options)?;
        let result = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            document
                .inner
                .to_tokenized_latex_with(&options)
                .map_err(|error| TexformError::new_err(error.to_string()))?
        };
        let dto = texform::bindings::tokenized_latex_to_dto(result);
        Ok(pythonize(py, &dto)?.unbind())
    }

    fn create_char(slf: &Bound<'_, Self>, py: Python<'_>, value: &str) -> PyResult<Py<PyNode>> {
        let ch = parse_char(value)?;
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document.inner.create_char(ch).map_err(edit_error)?
        };
        py_node(py, slf.clone().unbind(), id)
    }

    fn create_text(slf: &Bound<'_, Self>, py: Python<'_>, value: &str) -> PyResult<Py<PyNode>> {
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document.inner.create_text(value).map_err(edit_error)?
        };
        py_node(py, slf.clone().unbind(), id)
    }

    fn create_active_space(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyNode>> {
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document.inner.create_active_space().map_err(edit_error)?
        };
        py_node(py, slf.clone().unbind(), id)
    }

    fn create_group(slf: &Bound<'_, Self>, py: Python<'_>, mode: &str) -> PyResult<Py<PyNode>> {
        let mode = py_content_mode(mode)?;
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document.inner.create_group(mode).map_err(edit_error)?
        };
        py_node(py, slf.clone().unbind(), id)
    }

    #[pyo3(signature = (name, args = None))]
    fn create_command(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        name: &str,
        args: Option<Vec<Py<PyAny>>>,
    ) -> PyResult<Py<PyNode>> {
        let owner = slf.clone().unbind();
        let args = py_arg_values(py, &owner, args)?;
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document
                .inner
                .create_command(name, args)
                .map_err(edit_error)?
        };
        py_node(py, owner, id)
    }

    #[pyo3(signature = (name, args = None))]
    fn create_declarative(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        name: &str,
        args: Option<Vec<Py<PyAny>>>,
    ) -> PyResult<Py<PyNode>> {
        let owner = slf.clone().unbind();
        let args = py_arg_values(py, &owner, args)?;
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document
                .inner
                .create_declarative(name, args)
                .map_err(edit_error)?
        };
        py_node(py, owner, id)
    }

    #[pyo3(signature = (name, args, body))]
    fn create_environment(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        name: &str,
        args: Option<Vec<Py<PyAny>>>,
        body: PyRef<'_, PyNode>,
    ) -> PyResult<Py<PyNode>> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &body)?;
        let args = py_arg_values(py, &owner, args)?;
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document
                .inner
                .create_environment(name, args, body.id)
                .map_err(edit_error)?
        };
        py_node(py, owner, id)
    }

    fn append_child(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        parent: PyRef<'_, PyNode>,
        child: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &parent)?;
        ensure_node_owner(py, &owner, &child)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document
            .inner
            .append_child(parent.id, child.id)
            .map_err(edit_error)
    }

    fn insert_before(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        anchor: PyRef<'_, PyNode>,
        new: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &anchor)?;
        ensure_node_owner(py, &owner, &new)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document
            .inner
            .insert_before(anchor.id, new.id)
            .map_err(edit_error)
    }

    fn insert_after(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        anchor: PyRef<'_, PyNode>,
        new: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &anchor)?;
        ensure_node_owner(py, &owner, &new)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document
            .inner
            .insert_after(anchor.id, new.id)
            .map_err(edit_error)
    }

    fn insert_child(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        parent: PyRef<'_, PyNode>,
        index: usize,
        child: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &parent)?;
        ensure_node_owner(py, &owner, &child)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document
            .inner
            .insert_child(parent.id, index, child.id)
            .map_err(edit_error)
    }

    fn replace_with(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        target: PyRef<'_, PyNode>,
        replacement: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &target)?;
        ensure_node_owner(py, &owner, &replacement)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document
            .inner
            .replace_with(target.id, replacement.id)
            .map_err(edit_error)
    }

    fn wrap(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        target: PyRef<'_, PyNode>,
        wrapper: PyRef<'_, PyNode>,
    ) -> PyResult<Py<PyNode>> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &target)?;
        ensure_node_owner(py, &owner, &wrapper)?;
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document
                .inner
                .wrap(target.id, wrapper.id)
                .map_err(edit_error)?
        };
        py_node(py, owner, id)
    }

    fn unwrap(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        group: PyRef<'_, PyNode>,
    ) -> PyResult<Py<PyAny>> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &group)?;
        let ids = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document.inner.unwrap(group.id).map_err(edit_error)?
        };
        py_nodes_list(py, &owner, ids)
    }

    fn extract(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        node: PyRef<'_, PyNode>,
    ) -> PyResult<Py<PyNode>> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &node)?;
        let id = {
            let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
            document.inner.extract(node.id).map_err(edit_error)?
        };
        py_node(py, owner, id)
    }

    fn remove(slf: &Bound<'_, Self>, py: Python<'_>, node: PyRef<'_, PyNode>) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &node)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document.inner.remove(node.id).map_err(edit_error)
    }

    fn clear(slf: &Bound<'_, Self>, py: Python<'_>, container: PyRef<'_, PyNode>) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &container)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document.inner.clear(container.id).map_err(edit_error)
    }

    fn set_command_name(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        node: PyRef<'_, PyNode>,
        name: &str,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &node)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document
            .inner
            .set_command_name(node.id, name)
            .map_err(edit_error)
    }

    fn set_text(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        node: PyRef<'_, PyNode>,
        value: &str,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &node)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document.inner.set_text(node.id, value).map_err(edit_error)
    }

    fn set_char(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        node: PyRef<'_, PyNode>,
        value: &str,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &node)?;
        let ch = parse_char(value)?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document.inner.set_char(node.id, ch).map_err(edit_error)
    }

    fn set_arg(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        node: PyRef<'_, PyNode>,
        index: usize,
        value: Py<PyAny>,
    ) -> PyResult<()> {
        let owner = slf.clone().unbind();
        ensure_node_owner(py, &owner, &node)?;
        let value = py_arg_value(py, &owner, value.bind(py))?;
        let mut document = slf.try_borrow_mut().map_err(borrow_error)?;
        document
            .inner
            .set_arg(node.id, index, value)
            .map_err(edit_error)
    }
}

#[pyclass(name = "Node")]
struct PyNode {
    doc: Py<PyDocument>,
    id: texform::NodeId,
}

#[pymethods]
impl PyNode {
    #[pyo3(signature = (name = None))]
    fn is_command(&self, py: Python<'_>, name: Option<&str>) -> PyResult<bool> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(match name {
            Some(name) => node.is_command(name),
            None => node.kind() == texform::NodeKind::Command,
        })
    }

    #[pyo3(signature = (value = None))]
    fn is_char(&self, py: Python<'_>, value: Option<&str>) -> PyResult<bool> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(match value {
            Some(value) => node.is_char(parse_char(value)?),
            None => node.kind() == texform::NodeKind::Char,
        })
    }

    fn is_error(&self, py: Python<'_>) -> PyResult<bool> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node.is_error())
    }

    fn kind(&self, py: Python<'_>) -> PyResult<String> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(format!("{:?}", node.kind()))
    }

    fn parent(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.parent().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn children(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ids = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.children().map(|node| node.id()).collect::<Vec<_>>()
        };
        py_nodes_list(py, &self.doc, ids)
    }

    fn next_sibling(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.next_sibling().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn prev_sibling(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.prev_sibling().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn ancestors(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ids = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.ancestors().map(|node| node.id()).collect::<Vec<_>>()
        };
        py_nodes_list(py, &self.doc, ids)
    }

    fn descendants(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ids = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.descendants().map(|node| node.id()).collect::<Vec<_>>()
        };
        py_nodes_list(py, &self.doc, ids)
    }

    fn command_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node.command_name().map(ToOwned::to_owned))
    }

    fn env_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node.env_name().map(ToOwned::to_owned))
    }

    fn text(&self, py: Python<'_>) -> PyResult<Option<String>> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node.text().map(ToOwned::to_owned))
    }

    fn char(&self, py: Python<'_>) -> PyResult<Option<String>> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node.char().map(|ch| ch.to_string()))
    }

    fn prime_count(&self, py: Python<'_>) -> PyResult<Option<usize>> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node.prime_count())
    }

    fn error_parts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let parts = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.error_parts()
                .map(|(message, snippet)| (message.to_string(), snippet.to_string()))
        };
        match parts {
            Some((message, snippet)) => {
                let out = PyDict::new(py);
                out.set_item("message", message)?;
                out.set_item("snippet", snippet)?;
                Ok(out.unbind().into_any())
            }
            None => Ok(py.None()),
        }
    }

    fn content_mode(&self, py: Python<'_>) -> PyResult<Option<String>> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node
            .content_mode()
            .map(content_mode_to_str)
            .map(str::to_owned))
    }

    fn group_kind(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.group_kind().map(py_group_kind)
        };
        match value {
            Some(value) => Ok(pythonize(py, &value)?.unbind()),
            None => Ok(py.None()),
        }
    }

    fn arg_count(&self, py: Python<'_>) -> PyResult<usize> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        let node = document.inner.node(self.id).map_err(edit_error)?;
        Ok(node.arg_count())
    }

    fn arg(&self, py: Python<'_>, index: usize) -> PyResult<Py<PyAny>> {
        let arg = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.arg(index)
                .map(|arg| py_arg_ref(py, &self.doc, arg))
                .transpose()?
        };
        Ok(arg.unwrap_or_else(|| py.None()))
    }

    fn arg_slots(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let args = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.arg_slots()
                .map(|arg| {
                    arg.map(|arg| py_arg_ref(py, &self.doc, arg))
                        .transpose()
                        .map(|value| value.unwrap_or_else(|| py.None()))
                })
                .collect::<PyResult<Vec<_>>>()?
        };
        let out = PyList::empty(py);
        for arg in args {
            out.append(arg)?;
        }
        Ok(out.unbind().into_any())
    }

    fn script_base(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.script_base().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn subscript(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.subscript().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn superscript(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.superscript().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn infix_left(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.infix_left().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn infix_right(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.infix_right().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn env_body(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let id = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.env_body().map(|node| node.id())
        };
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    fn span(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let span = {
            let document = self.doc.try_borrow(py).map_err(borrow_error)?;
            let node = document.inner.node(self.id).map_err(edit_error)?;
            node.span()
        };
        match span {
            Some(span) => Ok(pythonize(py, &span)?.unbind()),
            None => Ok(py.None()),
        }
    }
}

fn apply_parse_config_dict(config: &mut CoreParseConfig, dict: &Bound<'_, PyDict>) -> PyResult<()> {
    if let Some(reject_unknown) = py_optional_bool(dict, "reject_unknown")? {
        config.reject_unknown = reject_unknown;
    }
    if let Some(abort_on_error) = py_optional_bool(dict, "abort_on_error")? {
        config.abort_on_error = abort_on_error;
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

fn apply_finalize_ast_dict(
    config: &mut CoreFinalizeAstConfig,
    dict: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if let Some(value) = py_optional_bool(dict, "enabled")? {
        config.enabled = value;
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
    if let Some(finalize_ast) = dict.get_item("finalize_ast")?
        && !finalize_ast.is_none()
    {
        if let Ok(finalize_ast) = finalize_ast.extract::<PyRef<'_, PyFinalizeAstConfig>>() {
            config.transform.finalize_ast = finalize_ast.to_core();
        } else {
            let dict = finalize_ast.cast::<PyDict>().map_err(|_| {
                ParseError::new_err("finalize_ast must be a FinalizeAstConfig or dict")
            })?;
            apply_finalize_ast_dict(&mut config.transform.finalize_ast, dict)?;
        }
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

fn apply_transform_config_dict(
    config: &mut CoreTransformConfig,
    dict: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if let Some(value) = py_optional_bool(dict, "rewrite_enabled")? {
        config.rewrite_enabled = value;
    }
    if let Some(value) = py_optional_bool(dict, "lower_attributes_enabled")? {
        config.lower_attributes_enabled = value;
    }
    if let Some(value) = py_optional_usize(dict, "max_iterations")? {
        config.max_iterations = value;
    }
    if let Some(finalize_ast) = dict.get_item("finalize_ast")?
        && !finalize_ast.is_none()
    {
        if let Ok(finalize_ast) = finalize_ast.extract::<PyRef<'_, PyFinalizeAstConfig>>() {
            config.finalize_ast = finalize_ast.to_core();
        } else {
            let dict = finalize_ast.cast::<PyDict>().map_err(|_| {
                ConfigError::new_err("finalize_ast must be a FinalizeAstConfig or dict")
            })?;
            apply_finalize_ast_dict(&mut config.finalize_ast, dict)?;
        }
    }
    if let Some(flatten_groups) = dict.get_item("flatten_groups")?
        && !flatten_groups.is_none()
    {
        if let Ok(flatten_groups) = flatten_groups.extract::<PyRef<'_, PyFlattenGroupsConfig>>() {
            config.flatten_groups = flatten_groups.to_core();
        } else {
            let dict = flatten_groups.cast::<PyDict>().map_err(|_| {
                ConfigError::new_err("flatten_groups must be a FlattenGroupsConfig or dict")
            })?;
            apply_flatten_groups_dict(&mut config.flatten_groups, dict)?;
        }
    }
    Ok(())
}

fn transform_config_from_python(
    config: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
    default: CoreTransformConfig,
) -> PyResult<CoreTransformConfig> {
    let mut parsed = default;
    if let Some(value) = config {
        if value.is_none() {
            // Keep the default.
        } else if let Ok(transform) = value.extract::<PyRef<'_, PyTransformConfig>>() {
            parsed = transform.to_core();
        } else {
            let dict = value
                .cast::<PyDict>()
                .map_err(|_| ConfigError::new_err("config must be a TransformConfig or dict"))?;
            apply_transform_config_dict(&mut parsed, dict)?;
        }
    }
    if let Some(kwargs) = kwargs {
        apply_transform_config_dict(&mut parsed, kwargs)?;
    }
    Ok(parsed)
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
                .map_err(|error| ConfigError::new_err(error.to_string()))?,
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
        let default = self.inner.default_parse_config().clone();
        let output = match parse_config_from_python(config, kwargs, default)? {
            Some(config) => self.inner.parse_with(src, &config),
            None => self.inner.parse(src),
        };
        parse_result_to_python(py, output)
    }

    fn lookup_command(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self.inner.lookup_command(name, py_content_mode(mode)?) {
                Some(record) => {
                    pythonize(py, &texform::bindings::command_info_to_dto(record))?.unbind()
                }
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
                Some(record) => {
                    pythonize(py, &texform::bindings::command_info_to_dto(record))?.unbind()
                }
                None => py.None(),
            },
        )
    }

    fn lookup_character(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self.inner.lookup_character(name, py_content_mode(mode)?) {
                Some(record) => {
                    pythonize(py, &texform::bindings::character_info_to_dto(record))?.unbind()
                }
                None => py.None(),
            },
        )
    }

    fn lookup_env(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(match self.inner.lookup_env(name, py_content_mode(mode)?) {
            Some(record) => pythonize(py, &texform::bindings::env_info_to_dto(record))?.unbind(),
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

#[pyclass(name = "TransformEngine")]
struct PyTransformEngine {
    inner: texform::TransformEngine,
}

#[pymethods]
impl PyTransformEngine {
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
            texform::TransformEngine::builder().profile(profile_from_name(profile)?),
            packages,
            items,
            remove_commands,
            remove_environments,
            remove_delimiter_controls,
        )?;
        for rule in disable_rules.unwrap_or_default() {
            builder = builder
                .disable_rule_by_name(&rule)
                .map_err(|error| ConfigError::new_err(error.to_string()))?;
        }
        Ok(Self {
            inner: builder
                .build()
                .map_err(|error| ConfigError::new_err(error.to_string()))?,
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
            parse: self.inner.parser().default_parse_config().clone(),
            transform: *self.inner.default_transform_config(),
        };
        let result = match normalize_config_from_python(config, kwargs, default)? {
            Some(config) => self.inner.normalize_with(src, &config),
            None => self.inner.normalize(src),
        }
        .map_err(|error| {
            binding_error_parts_to_py(py, texform::bindings::normalize_error_to_parts(error))
                .unwrap_or_else(|error| error)
        })?;
        transform_result_to_python(py, result.normalized, &result.report)
    }

    #[pyo3(signature = (document, config = None, **kwargs))]
    fn transform(
        &self,
        py: Python<'_>,
        document: &Bound<'_, PyDocument>,
        config: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let config =
            transform_config_from_python(config, kwargs, *self.inner.default_transform_config())?;
        let report = {
            let mut document = document.try_borrow_mut().map_err(borrow_error)?;
            self.inner.transform_with(&mut document.inner, &config)
        }
        .map_err(|error| {
            binding_error_parts_to_py(py, texform::bindings::normalize_error_to_parts(error))
                .unwrap_or_else(|error| error)
        })?;
        transform_report_to_python(py, &report)
    }

    #[pyo3(signature = (src, config = None, **kwargs))]
    fn parse(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let default = self.inner.parser().default_parse_config().clone();
        let output = match parse_config_from_python(config, kwargs, default)? {
            Some(config) => self.inner.parser().parse_with(src, &config),
            None => self.inner.parser().parse(src),
        };
        parse_result_to_python(py, output)
    }

    fn lookup_command(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self
                .inner
                .parser()
                .lookup_command(name, py_content_mode(mode)?)
            {
                Some(record) => {
                    pythonize(py, &texform::bindings::command_info_to_dto(record))?.unbind()
                }
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
                .parser()
                .lookup_explicit_command(name, py_content_mode(mode)?)
            {
                Some(record) => {
                    pythonize(py, &texform::bindings::command_info_to_dto(record))?.unbind()
                }
                None => py.None(),
            },
        )
    }

    fn lookup_character(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self
                .inner
                .parser()
                .lookup_character(name, py_content_mode(mode)?)
            {
                Some(record) => {
                    pythonize(py, &texform::bindings::character_info_to_dto(record))?.unbind()
                }
                None => py.None(),
            },
        )
    }

    fn lookup_env(&self, py: Python<'_>, name: &str, mode: &str) -> PyResult<Py<PyAny>> {
        Ok(
            match self.inner.parser().lookup_env(name, py_content_mode(mode)?) {
                Some(record) => {
                    pythonize(py, &texform::bindings::env_info_to_dto(record))?.unbind()
                }
                None => py.None(),
            },
        )
    }

    fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.parser().is_delimiter_control(name)
    }

    fn knows_command_name(&self, name: &str) -> bool {
        self.inner.parser().knows_command_name(name)
    }

    fn knows_env_name(&self, name: &str) -> bool {
        self.inner.parser().knows_env_name(name)
    }

    fn knows_character_name(&self, name: &str) -> bool {
        self.inner.parser().knows_character_name(name)
    }
}

fn transform_result_to_python(
    py: Python<'_>,
    normalized: String,
    report: &texform::TransformReport,
) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    out.set_item("normalized", normalized)?;
    out.set_item("report", transform_report_to_python(py, report)?)?;
    Ok(out.unbind().into_any())
}

fn transform_report_to_python(
    py: Python<'_>,
    report: &texform::TransformReport,
) -> PyResult<Py<PyAny>> {
    Ok(pythonize(py, &texform::bindings::transform_report_to_dto(report))?.unbind())
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
    let node = depythonize::<texform::SyntaxNode>(node)
        .map_err(|error| ParseError::new_err(format!("invalid syntax node: {error}")))?;
    let options = serialize_options_from_python(options)?;
    let document = texform::Document::from_syntax(&node).map_err(|error| match error {
        texform::FromSyntaxError::NotARoot => {
            ParseError::new_err("serialize expects a syntax root")
        }
        _ => ParseError::new_err(error.to_string()),
    })?;
    document
        .to_latex_with(&options)
        .map_err(|error| ParseError::new_err(error.to_string()))
}

#[pyfunction]
fn validate_argspec(py: Python<'_>, spec: &str) -> PyResult<Py<PyAny>> {
    Ok(pythonize(py, &texform::validate_argspec(spec))?.unbind())
}

#[pyfunction]
fn list_packages(py: Python<'_>) -> PyResult<Py<PyAny>> {
    Ok(pythonize(py, &texform::bindings::list_packages_to_dto())?.unbind())
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
    m.add_function(wrap_pyfunction!(list_packages, m)?)?;
    m.add_class::<PyParseConfig>()?;
    m.add_class::<PyLowerAttributesConfig>()?;
    m.add_class::<PyRewriteConfig>()?;
    m.add_class::<PyFinalizeAstConfig>()?;
    m.add_class::<PyFlattenGroupsConfig>()?;
    m.add_class::<PyTransformConfig>()?;
    m.add_class::<PyDocument>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyParser>()?;
    m.add_class::<PyTransformEngine>()?;
    m.add("TexformError", m.py().get_type::<TexformError>())?;
    m.add("ParseError", m.py().get_type::<ParseError>())?;
    m.add("EditError", m.py().get_type::<EditError>())?;
    m.add("ConfigError", m.py().get_type::<ConfigError>())?;
    m.add("TransformError", m.py().get_type::<TransformError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_parse_returns_document_and_diagnostics() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let result = parser.call_method1("parse", (r"\frac{a}{b}",)).unwrap();
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
            let document = dict.get_item("document").unwrap().unwrap();
            let diagnostics = dict.get_item("diagnostics").unwrap().unwrap();

            assert!(document.is_instance_of::<PyDocument>());
            assert_eq!(diagnostics.len().unwrap(), 0);
            assert_eq!(
                document
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"\frac { a } { b }"
            );
            let tokenized = document.call_method0("to_tokenized_latex").unwrap();
            let tokenized = tokenized.cast::<pyo3::types::PyDict>().unwrap();
            assert_eq!(
                tokenized
                    .get_item("latex")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"\frac { a } { b }"
            );
            let tokens = tokenized
                .get_item("tokens")
                .unwrap()
                .unwrap()
                .cast_into::<pyo3::types::PyList>()
                .unwrap();
            let first = tokens
                .get_item(0)
                .unwrap()
                .cast_into::<pyo3::types::PyDict>()
                .unwrap();
            assert!(first.get_item("start_byte").unwrap().is_some());
            assert_eq!(
                document
                    .call_method0("root")
                    .unwrap()
                    .call_method0("kind")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "Root"
            );

            let config = pyo3::types::PyDict::new(py);
            config.set_item("reject_unknown", true).unwrap();
            let result = parser
                .call_method1("parse", (r"\unknowncmd", config))
                .expect("diagnostics should be returned instead of raised");
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
            let document = dict.get_item("document").unwrap().unwrap();
            let diagnostics = dict.get_item("diagnostics").unwrap().unwrap();

            assert!(document.is_instance_of::<PyDocument>());
            assert!(
                document
                    .call_method0("has_errors")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert_eq!(diagnostics.len().unwrap(), 1);
        });
    }

    #[test]
    fn python_validate_argspec_returns_snake_case_contract() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let result = module
                .getattr("validate_argspec")
                .unwrap()
                .call1(("m",))
                .unwrap()
                .cast_into::<PyDict>()
                .unwrap();

            assert!(
                result
                    .get_item("valid")
                    .unwrap()
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert_eq!(
                result
                    .get_item("arg_count")
                    .unwrap()
                    .unwrap()
                    .extract::<usize>()
                    .unwrap(),
                1
            );
            let parsed = result.get_item("parsed").unwrap().unwrap();
            assert!(parsed.cast::<PyList>().is_ok());
        });
    }

    #[test]
    fn python_normalize_parse_error_has_error_base_and_payload() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = PyDict::new(py);
            kwargs.set_item("profile", "authoring").unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();
            let error = engine
                .call_method1("normalize", ("{",))
                .expect_err("normalize should raise a parse error");

            assert!(error.is_instance_of::<ParseError>(py));
            assert!(error.is_instance_of::<TexformError>(py));
            let value = error.value(py);
            assert!(
                value
                    .getattr("diagnostics")
                    .unwrap()
                    .cast::<PyList>()
                    .is_ok()
            );
            assert!(!value.getattr("document").unwrap().is_none());
        });
    }

    #[test]
    fn python_rejects_cross_document_nodes() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let document_cls = module.getattr("Document").unwrap();
            let first = document_cls.call0().unwrap();
            let second = document_cls.call0().unwrap();
            let root = first.call_method0("root").unwrap();
            let foreign = second.call_method1("create_char", ("x",)).unwrap();

            let error = first
                .call_method1("append_child", (root, foreign))
                .expect_err("foreign child should be rejected");
            assert!(error.to_string().contains("different document"));
        });
    }

    #[test]
    fn python_create_command_with_arg_roundtrips_latex() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let document = module.getattr("Document").unwrap().call0().unwrap();
            let arg = document.call_method1("create_char", ("x",)).unwrap();
            let arg_value = PyDict::new(py);
            arg_value.set_item("kind", "Math").unwrap();
            arg_value.set_item("node", arg).unwrap();

            let command = document
                .call_method1("create_command", ("sqrt", vec![arg_value]))
                .unwrap();
            let root = document.call_method0("root").unwrap();
            document
                .call_method1("append_child", (root, &command))
                .unwrap();

            let read_arg_value = command.call_method1("arg", (0usize,)).unwrap();
            let read_arg = read_arg_value.cast::<PyDict>().unwrap();
            assert_eq!(
                read_arg
                    .get_item("kind")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "Math"
            );
            assert_eq!(
                document
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"\sqrt { x }"
            );
        });
    }

    #[test]
    fn python_rejects_read_only_error_document_editing() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let config = PyDict::new(py);
            config.set_item("reject_unknown", true).unwrap();
            let result = parser
                .call_method1("parse", (r"\unknowncmd", config))
                .unwrap();
            let dict = result.cast::<PyDict>().unwrap();
            let document = dict.get_item("document").unwrap().unwrap();

            assert!(
                document
                    .call_method0("is_read_only")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            let error = document
                .call_method1("create_char", ("x",))
                .expect_err("read-only document edits should fail");
            assert!(error.to_string().contains("read-only"));
        });
    }

    #[test]
    fn python_engine_normalizes_with_profile_and_packages() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let engine_cls = module.getattr("TransformEngine").unwrap();
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
            config_kwargs.set_item("reject_unknown", true).unwrap();
            config_kwargs.set_item("abort_on_error", true).unwrap();
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
            config.set_item("reject_unknown", true).unwrap();
            let result = parser
                .call_method1("parse", (r"\unknowncmd", config))
                .expect("reject_unknown dict config should return diagnostics");
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
            assert!(
                dict.get_item("document")
                    .unwrap()
                    .unwrap()
                    .is_instance_of::<PyDocument>()
            );
            assert_eq!(
                dict.get_item("diagnostics")
                    .unwrap()
                    .unwrap()
                    .len()
                    .unwrap(),
                1
            );

            let config = pyo3::types::PyDict::new(py);
            config.set_item("reject_unknown", true).unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("config", config).unwrap();
            kwargs.set_item("reject_unknown", false).unwrap();
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
                .getattr("TransformEngine")
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
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let result = engine
                .call_method1("parse", (r"\unknowncmd",))
                .expect("engine parse should preserve unknown commands");
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
            assert_eq!(
                dict.get_item("diagnostics")
                    .unwrap()
                    .unwrap()
                    .len()
                    .unwrap(),
                0
            );
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
                .getattr("TransformEngine")
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
            let finalize_ast = pyo3::types::PyDict::new(py);
            finalize_ast.set_item("enabled", false).unwrap();
            call_kwargs.set_item("finalize_ast", finalize_ast).unwrap();
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
    fn python_engine_transform_updates_own_document_in_place() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs.set_item("packages", vec!["base"]).unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let parsed = engine.call_method1("parse", ("{{x}}",)).unwrap();
            let document = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            let config = pyo3::types::PyDict::new(py);
            config.set_item("rewrite_enabled", false).unwrap();
            config.set_item("lower_attributes_enabled", false).unwrap();
            let flatten_groups = pyo3::types::PyDict::new(py);
            flatten_groups.set_item("enabled", true).unwrap();
            config.set_item("flatten_groups", flatten_groups).unwrap();

            let report = engine
                .call_method1("transform", (&document, config))
                .expect("transform should succeed");

            assert_eq!(
                document
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "x"
            );
            assert!(
                report
                    .cast::<pyo3::types::PyDict>()
                    .unwrap()
                    .get_item("flatten_groups")
                    .unwrap()
                    .is_some()
            );
        });
    }

    #[test]
    fn python_engine_transform_rejects_document_without_parse_context() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs.set_item("packages", vec!["base"]).unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let parsed = engine.call_method1("parse", ("x",)).unwrap();
            let parsed_document = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            let syntax = parsed_document.call_method0("to_syntax").unwrap();
            let document_cls = module.getattr("Document").unwrap();
            let document = document_cls
                .call_method1("from_syntax", (syntax,))
                .expect("syntax should rebuild document");

            let error = engine
                .call_method1("transform", (document,))
                .expect_err("syntax-created documents must not be transformed");

            assert!(error.is_instance_of::<TransformError>(py));
        });
    }

    #[test]
    fn python_engine_transform_rejects_document_from_another_engine() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs.set_item("packages", vec!["base"]).unwrap();
            let first_engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();
            let second_engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let parsed = first_engine.call_method1("parse", ("x",)).unwrap();
            let document = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();

            let error = second_engine
                .call_method1("transform", (document,))
                .expect_err("documents from another engine must not be transformed");

            assert!(error.is_instance_of::<TransformError>(py));
        });
    }

    #[test]
    fn python_engine_transform_invalid_config_raises_config_error() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs.set_item("packages", vec!["base"]).unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();
            let parsed = engine.call_method1("parse", ("x",)).unwrap();
            let document = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();

            let error = engine
                .call_method1("transform", (document.clone(), py.None()))
                .expect("None config should be accepted");
            assert!(error.cast::<pyo3::types::PyDict>().is_ok());

            let error = engine
                .call_method1("transform", (document, "not a config"))
                .expect_err("invalid config object should fail");

            assert!(error.is_instance_of::<ConfigError>(py));
        });
    }

    #[test]
    fn python_engine_reports_finalize_ast_and_can_disable_it() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "corpus").unwrap();
            kwargs.set_item("packages", vec!["base"]).unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let result = engine
                .call_method1("normalize", (r"f^{\prime\prime}",))
                .expect("normalize should use default FinalizeAst");
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
            assert_eq!(
                dict.get_item("normalized")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "f''"
            );
            let report_value = dict.get_item("report").unwrap().unwrap();
            let report = report_value.cast::<pyo3::types::PyDict>().unwrap();
            assert!(report.get_item("finalize_ast").unwrap().is_some());
            assert!(report.get_item("finalizeAst").unwrap().is_none());

            let finalize_ast = pyo3::types::PyDict::new(py);
            finalize_ast.set_item("enabled", false).unwrap();
            let call_kwargs = pyo3::types::PyDict::new(py);
            call_kwargs.set_item("finalize_ast", finalize_ast).unwrap();
            let disabled = engine
                .call_method("normalize", (r"f^{\prime\prime}",), Some(&call_kwargs))
                .expect("normalize should accept finalize_ast kwargs");
            let disabled = disabled.cast::<pyo3::types::PyDict>().unwrap();
            assert_eq!(
                disabled
                    .get_item("normalized")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"f ^ { '' }"
            );
        });
    }

    #[test]
    fn python_node_exposes_prime_count() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let parsed = parser.call_method1("parse", ("f''",)).unwrap();
            let document = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            let prime = document
                .call_method0("root")
                .unwrap()
                .call_method0("children")
                .unwrap()
                .get_item(0)
                .unwrap()
                .call_method0("superscript")
                .unwrap();

            assert_eq!(
                prime
                    .call_method0("kind")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "Prime"
            );
            assert_eq!(
                prime
                    .call_method0("prime_count")
                    .unwrap()
                    .extract::<usize>()
                    .unwrap(),
                2
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
            let document = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            let node = document.call_method0("to_syntax").unwrap();
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
    fn python_module_profile_names_use_faithful_and_reject_corpus_drop() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let config_cls = module.getattr("TransformConfig").unwrap();
            assert!(config_cls.call_method0("corpus_drop").is_err());

            let faithful = config_cls.call_method0("faithful").unwrap();
            let faithful_flatten_groups = faithful.getattr("flatten_groups").unwrap();
            assert!(
                faithful_flatten_groups
                    .getattr("preserve_group_adjacent_to_command_like")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );

            let config = config_cls.call_method0("corpus").unwrap();
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
            assert!(repr.contains("finalize_ast"));
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
