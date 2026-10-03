use std::hash::{Hash, Hasher};

use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pythonize::{depythonize, pythonize};
use texform::bindings::BindingErrorDto;
use texform::{Arg, ContentMode};

mod config;

use config::{
    PyFinalizeAstConfig, PyFlattenGroupsConfig, PyLowerAttributesConfig, PyParseConfig,
    PyRewriteConfig, PyTransformConfig, from_python, normalize_config_from_python,
    parse_config_from_python, serialize_options_from_python, transform_config_from_python,
};

pyo3::create_exception!(texform, TexformError, PyException);
pyo3::create_exception!(texform, ParseError, TexformError);
pyo3::create_exception!(texform, EditError, TexformError);
pyo3::create_exception!(texform, ConformanceError, EditError);
pyo3::create_exception!(texform, ConfigError, TexformError);
pyo3::create_exception!(texform, TransformError, TexformError);

/// Call a constructor in an explicit context mode, or in its default context.
macro_rules! construct {
    ($doc:expr, $mode:expr, $method:ident($($arg:expr),*)) => {
        match $mode {
            Some(mode) => $doc.in_mode(mode).$method($($arg),*),
            None => $doc.$method($($arg),*),
        }
    };
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

fn py_content_mode(value: &str) -> PyResult<ContentMode> {
    match value {
        "math" => Ok(ContentMode::Math),
        "text" => Ok(ContentMode::Text),
        other => Err(ConfigError::new_err(format!(
            "unsupported content mode: {other}"
        ))),
    }
}

fn py_optional_mode(mode: Option<&str>) -> PyResult<Option<ContentMode>> {
    mode.map(py_content_mode).transpose()
}

/// Negative counts reach the core as zero, so it reports `invalid_prime_count`.
fn prime_count(count: i128) -> usize {
    usize::try_from(count.max(0)).unwrap_or(usize::MAX)
}

fn borrow_error(error: impl std::fmt::Display) -> PyErr {
    EditError::new_err(format!("document borrow conflict: {error}"))
}

fn edit_error(error: texform::EditError) -> PyErr {
    binding_error_to_py(texform::bindings::edit_error_to_dto(error), None)
}

fn normalize_error(error: texform::NormalizeError) -> PyErr {
    let parts = texform::bindings::normalize_error_to_parts(error);
    binding_error_to_py(parts.error, parts.document)
}

/// Raise the exception class for `error.kind` with its structured attributes.
fn binding_error_to_py(error: BindingErrorDto, document: Option<texform::Document>) -> PyErr {
    Python::attach(|py| {
        let exception = match error.kind {
            "parse" => ParseError::new_err(error.message),
            "conformance" => ConformanceError::new_err(error.message),
            "edit" => EditError::new_err(error.message),
            "config" => ConfigError::new_err(error.message),
            "transform" => TransformError::new_err(error.message),
            _ => TexformError::new_err(error.message),
        };
        let value = exception.value(py);
        let attached = (|| {
            if error.kind == "parse" {
                value.setattr("diagnostics", pythonize(py, &error.diagnostics)?)?;
                let document = document.map(|inner| Py::new(py, PyDocument { inner }));
                value.setattr("document", document.transpose()?)?;
            }
            if let Some(conformance) = error.conformance {
                value.setattr("path", conformance.path)?;
                value.setattr("rule", conformance.rule)?;
            }
            PyResult::Ok(())
        })();
        attached.err().unwrap_or(exception)
    })
}

fn syntax_node(node: &Bound<'_, PyAny>) -> PyResult<texform::SyntaxNode> {
    depythonize(node).map_err(|error| {
        binding_error_to_py(
            BindingErrorDto::new("parse", format!("invalid syntax node: {error}")),
            None,
        )
    })
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

fn ensure_node_owner(owner: &Bound<'_, PyDocument>, node: &PyNode) -> PyResult<()> {
    if node.doc.bind(owner.py()).is(owner) {
        Ok(())
    } else {
        Err(EditError::new_err("node belongs to a different document"))
    }
}

#[pyclass(name = "Paired", frozen, get_all, module = "texform._native")]
struct PyPaired {
    value: Py<PyAny>,
    open: String,
    close: String,
}

#[pymethods]
impl PyPaired {
    #[new]
    fn new(value: Py<PyAny>, open: String, close: String) -> Self {
        Self { value, open, close }
    }

    fn __eq__(&self, py: Python<'_>, other: PyRef<'_, Self>) -> PyResult<bool> {
        Ok(self.open == other.open
            && self.close == other.close
            && self.value.bind(py).eq(&other.value)?)
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        (&self.value, &self.open, &self.close)
            .into_pyobject(py)?
            .hash()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let fields = (&self.value, &self.open, &self.close).into_pyobject(py)?;
        Ok(format!("Paired{}", fields.repr()?))
    }
}

fn py_args(
    owner: &Bound<'_, PyDocument>,
    args: Option<Vec<Bound<'_, PyAny>>>,
) -> PyResult<Vec<Arg>> {
    args.unwrap_or_default()
        .iter()
        .map(|arg| py_arg(owner, arg))
        .collect()
}

fn py_arg(owner: &Bound<'_, PyDocument>, value: &Bound<'_, PyAny>) -> PyResult<Arg> {
    if value.is_none() {
        return Ok(Arg::Absent);
    }
    if let Ok(node) = value.extract::<PyRef<'_, PyNode>>() {
        ensure_node_owner(owner, &node)?;
        return Ok(Arg::Node(node.id));
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(Arg::Source(value));
    }
    if let Ok(value) = value.extract::<bool>() {
        return Ok(Arg::Star(value));
    }
    if let Ok(pair) = value.extract::<PyRef<'_, PyPaired>>() {
        let value = py_arg(owner, pair.value.bind(value.py()))?;
        return Arg::paired(value, &pair.open, &pair.close).map_err(edit_error);
    }
    Err(PyTypeError::new_err(
        "argument must be a Node, source string, bool, None, or Paired",
    ))
}

fn knowledge_base_value(value: Option<PyRef<'_, PyKnowledgeBase>>) -> texform::KnowledgeBase {
    value.map(|kb| kb.inner.clone()).unwrap_or_default()
}

#[pyclass(name = "KnowledgeBase", frozen)]
struct PyKnowledgeBase {
    inner: texform::KnowledgeBase,
}

#[pymethods]
impl PyKnowledgeBase {
    #[new]
    #[pyo3(signature = (packages = None, *, items = None, remove_commands = None, remove_environments = None, remove_delimiter_controls = None))]
    fn new(
        packages: Option<Vec<String>>,
        items: Option<&Bound<'_, PyAny>>,
        remove_commands: Option<Vec<String>>,
        remove_environments: Option<Vec<String>>,
        remove_delimiter_controls: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let input = texform::bindings::KnowledgeBaseInput {
            packages,
            items: items.map(|items| from_python(items, "items")).transpose()?,
            remove_commands,
            remove_environments,
            remove_delimiter_controls,
        };
        Ok(Self {
            inner: input.build().map_err(ConfigError::new_err)?,
        })
    }

    fn __eq__(&self, other: PyRef<'_, Self>) -> bool {
        self.inner.ptr_eq(&other.inner)
    }

    fn __hash__(&self) -> u64 {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        self.inner.identity().hash(&mut state);
        state.finish()
    }

    fn packages(&self) -> Vec<&'static str> {
        self.inner.packages()
    }

    fn commands(&self, py: Python<'_>, mode: &str) -> PyResult<Py<PyAny>> {
        let records = self
            .inner
            .commands(py_content_mode(mode)?)
            .into_iter()
            .map(texform::bindings::command_info_to_dto)
            .collect::<Vec<_>>();
        Ok(pythonize(py, &records)?.unbind())
    }
    fn environments(&self, py: Python<'_>, mode: &str) -> PyResult<Py<PyAny>> {
        let records = self
            .inner
            .environments(py_content_mode(mode)?)
            .into_iter()
            .map(texform::bindings::env_info_to_dto)
            .collect::<Vec<_>>();
        Ok(pythonize(py, &records)?.unbind())
    }
    fn characters(&self, py: Python<'_>, mode: &str) -> PyResult<Py<PyAny>> {
        let records = self
            .inner
            .characters(py_content_mode(mode)?)
            .into_iter()
            .map(texform::bindings::character_info_to_dto)
            .collect::<Vec<_>>();
        Ok(pythonize(py, &records)?.unbind())
    }
    fn delimiters(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let records = self
            .inner
            .delimiters()
            .into_iter()
            .map(texform::bindings::delimiter_info_to_dto)
            .collect::<Vec<_>>();
        Ok(pythonize(py, &records)?.unbind())
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

#[pyclass(name = "Document")]
struct PyDocument {
    inner: texform::Document,
}

impl PyDocument {
    /// Run a fallible operation on the document after checking node ownership.
    fn edit<T>(
        slf: &Bound<'_, Self>,
        nodes: &[&PyNode],
        op: impl FnOnce(&mut texform::Document) -> Result<T, texform::EditError>,
    ) -> PyResult<T> {
        for node in nodes {
            ensure_node_owner(slf, node)?;
        }
        op(&mut slf.try_borrow_mut().map_err(borrow_error)?.inner).map_err(edit_error)
    }

    /// Like [`Self::edit`], returning a handle to the resulting node.
    fn edit_node(
        slf: &Bound<'_, Self>,
        nodes: &[&PyNode],
        op: impl FnOnce(&mut texform::Document) -> Result<texform::NodeId, texform::EditError>,
    ) -> PyResult<Py<PyNode>> {
        let id = Self::edit(slf, nodes, op)?;
        py_node(slf.py(), slf.clone().unbind(), id)
    }

    /// Collect node ids from a read of the document into a list of handles.
    fn read_nodes(
        slf: &Bound<'_, Self>,
        read: impl FnOnce(&texform::Document) -> Vec<texform::NodeId>,
    ) -> PyResult<Py<PyAny>> {
        let ids = read(&slf.try_borrow().map_err(borrow_error)?.inner);
        py_nodes_list(slf.py(), &slf.clone().unbind(), ids)
    }
}

#[pymethods]
impl PyDocument {
    #[new]
    #[pyo3(signature = (knowledge_base = None, *, mode = "math"))]
    fn new(knowledge_base: Option<PyRef<'_, PyKnowledgeBase>>, mode: &str) -> PyResult<Self> {
        Ok(Self {
            inner: texform::Document::with_knowledge_base(
                &knowledge_base_value(knowledge_base),
                py_content_mode(mode)?,
            ),
        })
    }

    fn knowledge_base(&self) -> PyKnowledgeBase {
        PyKnowledgeBase {
            inner: self.inner.knowledge_base().clone(),
        }
    }

    fn copy(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
    fn __copy__(&self) -> Self {
        self.copy()
    }
    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.copy()
    }

    #[staticmethod]
    #[pyo3(signature = (node, knowledge_base = None))]
    fn from_syntax(
        node: &Bound<'_, PyAny>,
        knowledge_base: Option<PyRef<'_, PyKnowledgeBase>>,
    ) -> PyResult<Self> {
        let node = syntax_node(node)?;
        let inner =
            texform::Document::from_syntax_with(&knowledge_base_value(knowledge_base), &node)
                .map_err(|error| {
                    binding_error_to_py(texform::bindings::from_syntax_error_to_dto(error), None)
                })?;
        Ok(Self { inner })
    }

    fn root(slf: &Bound<'_, Self>) -> PyResult<Py<PyNode>> {
        let id = slf.try_borrow().map_err(borrow_error)?.inner.root().id();
        py_node(slf.py(), slf.clone().unbind(), id)
    }

    fn has_errors(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let document = slf.try_borrow().map_err(borrow_error)?;
        Ok(document.inner.has_errors())
    }

    fn is_read_only(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let document = slf.try_borrow().map_err(borrow_error)?;
        Ok(document.inner.is_read_only())
    }

    fn errors(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::read_nodes(slf, |document| {
            document.errors().map(|node| node.id()).collect()
        })
    }

    fn find_commands(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        Self::read_nodes(slf, |document| {
            document.find_commands(name).map(|node| node.id()).collect()
        })
    }

    fn find_environments(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        Self::read_nodes(slf, |document| {
            document
                .find_environments(name)
                .map(|node| node.id())
                .collect()
        })
    }

    fn to_syntax(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let syntax = slf.try_borrow().map_err(borrow_error)?.inner.to_syntax();
        Ok(pythonize(py, &syntax)?.unbind())
    }

    /// Flatten the tree into columns for bulk structural analysis with Arrow or DataFrame tools.
    fn to_columnar(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let tables = slf.try_borrow().map_err(borrow_error)?.inner.to_columnar();
        Ok(pythonize(py, &tables)?.unbind())
    }

    fn node_spans(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let entries = {
            let document = slf.try_borrow().map_err(borrow_error)?;
            texform::bindings::node_spans_to_dto(&document.inner)
        };
        Ok(pythonize(py, &entries)?.unbind())
    }

    #[pyo3(signature = (**options))]
    fn to_latex(slf: &Bound<'_, Self>, options: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
        let options = serialize_options_from_python(options)?;
        let document = slf.try_borrow().map_err(borrow_error)?;
        document
            .inner
            .to_latex_with(&options)
            .map_err(|error| TexformError::new_err(error.to_string()))
    }

    #[pyo3(signature = (**options))]
    fn to_tokenized_latex(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        options: Option<&Bound<'_, PyDict>>,
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

    #[pyo3(signature = (value, *, mode = None))]
    fn create_char(slf: &Bound<'_, Self>, value: &str, mode: Option<&str>) -> PyResult<Py<PyNode>> {
        let value = texform::bindings::parse_char(value).map_err(edit_error)?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| construct!(doc, mode, create_char(value)))
    }

    #[pyo3(signature = (value, *, mode = None))]
    fn create_text(slf: &Bound<'_, Self>, value: &str, mode: Option<&str>) -> PyResult<Py<PyNode>> {
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| construct!(doc, mode, create_text(value)))
    }

    #[pyo3(signature = (*, mode = None))]
    fn create_active_space(slf: &Bound<'_, Self>, mode: Option<&str>) -> PyResult<Py<PyNode>> {
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| construct!(doc, mode, create_active_space()))
    }

    #[pyo3(signature = (*, mode = None))]
    fn create_alignment_tab(slf: &Bound<'_, Self>, mode: Option<&str>) -> PyResult<Py<PyNode>> {
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            construct!(doc, mode, create_alignment_tab())
        })
    }

    #[pyo3(signature = (count, *, mode = None))]
    fn create_prime(
        slf: &Bound<'_, Self>,
        count: i128,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let count = prime_count(count);
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| construct!(doc, mode, create_prime(count)))
    }

    /// The group's own `mode`, also its context, defaults to the root mode.
    #[pyo3(signature = (mode = None, children = None))]
    fn create_group(
        slf: &Bound<'_, Self>,
        mode: Option<&str>,
        children: Option<Vec<Bound<'_, PyAny>>>,
    ) -> PyResult<Py<PyNode>> {
        let children = py_args(slf, children)?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            let mode =
                mode.unwrap_or_else(|| doc.root().content_mode().unwrap_or(ContentMode::Math));
            doc.create_group(mode, children)
        })
    }

    #[pyo3(signature = (left, right, children = None, *, mode = None))]
    fn create_delimited_group(
        slf: &Bound<'_, Self>,
        left: &str,
        right: &str,
        children: Option<Vec<Bound<'_, PyAny>>>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let left = left.parse().map_err(edit_error)?;
        let right = right.parse().map_err(edit_error)?;
        let children = py_args(slf, children)?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            construct!(doc, mode, create_delimited_group(left, right, children))
        })
    }

    #[pyo3(signature = (children = None, *, mode = None))]
    fn create_inline_math(
        slf: &Bound<'_, Self>,
        children: Option<Vec<Bound<'_, PyAny>>>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let children = py_args(slf, children)?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            construct!(doc, mode, create_inline_math(children))
        })
    }

    #[pyo3(signature = (base, sub = None, sup = None, *, mode = None))]
    fn create_scripted(
        slf: &Bound<'_, Self>,
        base: &Bound<'_, PyAny>,
        sub: Option<Bound<'_, PyAny>>,
        sup: Option<Bound<'_, PyAny>>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let base = py_arg(slf, base)?;
        let sub = sub.map(|sub| py_arg(slf, &sub)).transpose()?;
        let sup = sup.map(|sup| py_arg(slf, &sup)).transpose()?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            construct!(doc, mode, create_scripted(base, sub, sup))
        })
    }

    #[pyo3(signature = (name, args = None, *, mode = None))]
    fn create_command(
        slf: &Bound<'_, Self>,
        name: &str,
        args: Option<Vec<Bound<'_, PyAny>>>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let args = py_args(slf, args)?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            construct!(doc, mode, create_command(name, args))
        })
    }

    #[pyo3(signature = (name, args = None, *, mode = None))]
    fn create_declarative(
        slf: &Bound<'_, Self>,
        name: &str,
        args: Option<Vec<Bound<'_, PyAny>>>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let args = py_args(slf, args)?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            construct!(doc, mode, create_declarative(name, args))
        })
    }

    #[pyo3(signature = (name, left, right, args = None, *, mode = None))]
    fn create_infix(
        slf: &Bound<'_, Self>,
        name: &str,
        left: &Bound<'_, PyAny>,
        right: &Bound<'_, PyAny>,
        args: Option<Vec<Bound<'_, PyAny>>>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let left = py_arg(slf, left)?;
        let right = py_arg(slf, right)?;
        let args = py_args(slf, args)?;
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| {
            construct!(doc, mode, create_infix(name, left, right, args))
        })
    }

    /// A list body becomes the children of the implicit body group.
    #[pyo3(signature = (name, args = None, body = None, *, mode = None))]
    fn create_environment(
        slf: &Bound<'_, Self>,
        name: &str,
        args: Option<Vec<Bound<'_, PyAny>>>,
        body: Option<Bound<'_, PyAny>>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let args = py_args(slf, args)?;
        let mode = py_optional_mode(mode)?;
        match body {
            Some(body) if body.is_instance_of::<PyList>() => {
                let children = py_args(slf, Some(body.extract()?))?;
                Self::edit_node(slf, &[], |doc| {
                    construct!(
                        doc,
                        mode,
                        create_environment_with_children(name, args, children)
                    )
                })
            }
            body => {
                let body = body.map_or(Ok(Arg::Absent), |body| py_arg(slf, &body))?;
                Self::edit_node(slf, &[], |doc| {
                    construct!(doc, mode, create_environment(name, args, body))
                })
            }
        }
    }

    #[pyo3(signature = (source, *, mode = None))]
    fn parse_fragment(
        slf: &Bound<'_, Self>,
        source: &str,
        mode: Option<&str>,
    ) -> PyResult<Py<PyNode>> {
        let mode = py_optional_mode(mode)?;
        Self::edit_node(slf, &[], |doc| doc.parse_fragment(source, mode))
    }

    fn append_child(
        slf: &Bound<'_, Self>,
        parent: PyRef<'_, PyNode>,
        child: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        Self::edit(slf, &[&parent, &child], |doc| {
            doc.append_child(parent.id, child.id)
        })
    }

    fn insert_before(
        slf: &Bound<'_, Self>,
        anchor: PyRef<'_, PyNode>,
        new: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        Self::edit(slf, &[&anchor, &new], |doc| {
            doc.insert_before(anchor.id, new.id)
        })
    }

    fn insert_after(
        slf: &Bound<'_, Self>,
        anchor: PyRef<'_, PyNode>,
        new: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        Self::edit(slf, &[&anchor, &new], |doc| {
            doc.insert_after(anchor.id, new.id)
        })
    }

    fn insert_child(
        slf: &Bound<'_, Self>,
        parent: PyRef<'_, PyNode>,
        index: usize,
        child: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        Self::edit(slf, &[&parent, &child], |doc| {
            doc.insert_child(parent.id, index, child.id)
        })
    }

    fn replace_with(
        slf: &Bound<'_, Self>,
        target: PyRef<'_, PyNode>,
        replacement: PyRef<'_, PyNode>,
    ) -> PyResult<()> {
        Self::edit(slf, &[&target, &replacement], |doc| {
            doc.replace_with(target.id, replacement.id)
        })
    }

    fn wrap(
        slf: &Bound<'_, Self>,
        target: PyRef<'_, PyNode>,
        wrapper: PyRef<'_, PyNode>,
    ) -> PyResult<Py<PyNode>> {
        Self::edit_node(slf, &[&target, &wrapper], |doc| {
            doc.wrap(target.id, wrapper.id)
        })
    }

    fn unwrap(slf: &Bound<'_, Self>, group: PyRef<'_, PyNode>) -> PyResult<Py<PyAny>> {
        let ids = Self::edit(slf, &[&group], |doc| doc.unwrap(group.id))?;
        py_nodes_list(slf.py(), &slf.clone().unbind(), ids)
    }

    fn extract(slf: &Bound<'_, Self>, node: PyRef<'_, PyNode>) -> PyResult<Py<PyNode>> {
        Self::edit_node(slf, &[&node], |doc| doc.extract(node.id))
    }

    fn remove(slf: &Bound<'_, Self>, node: PyRef<'_, PyNode>) -> PyResult<()> {
        Self::edit(slf, &[&node], |doc| doc.remove(node.id))
    }

    fn clear(slf: &Bound<'_, Self>, container: PyRef<'_, PyNode>) -> PyResult<()> {
        Self::edit(slf, &[&container], |doc| doc.clear(container.id))
    }

    fn set_command_name(
        slf: &Bound<'_, Self>,
        node: PyRef<'_, PyNode>,
        name: &str,
    ) -> PyResult<()> {
        Self::edit(slf, &[&node], |doc| doc.set_command_name(node.id, name))
    }

    fn set_env_name(slf: &Bound<'_, Self>, node: PyRef<'_, PyNode>, name: &str) -> PyResult<()> {
        Self::edit(slf, &[&node], |doc| doc.set_env_name(node.id, name))
    }

    fn set_text(slf: &Bound<'_, Self>, node: PyRef<'_, PyNode>, value: &str) -> PyResult<()> {
        Self::edit(slf, &[&node], |doc| doc.set_text(node.id, value))
    }

    fn set_char(slf: &Bound<'_, Self>, node: PyRef<'_, PyNode>, value: &str) -> PyResult<()> {
        let value = texform::bindings::parse_char(value).map_err(edit_error)?;
        Self::edit(slf, &[&node], |doc| doc.set_char(node.id, value))
    }

    fn set_subscript(
        slf: &Bound<'_, Self>,
        target: PyRef<'_, PyNode>,
        sub: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyNode>> {
        let sub = sub.map(|sub| py_arg(slf, &sub)).transpose()?;
        Self::edit_node(slf, &[&target], |doc| doc.set_subscript(target.id, sub))
    }

    fn set_superscript(
        slf: &Bound<'_, Self>,
        target: PyRef<'_, PyNode>,
        sup: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyNode>> {
        let sup = sup.map(|sup| py_arg(slf, &sup)).transpose()?;
        Self::edit_node(slf, &[&target], |doc| doc.set_superscript(target.id, sup))
    }

    fn set_arg_delimiters(
        slf: &Bound<'_, Self>,
        node: PyRef<'_, PyNode>,
        index: usize,
        open: &str,
        close: &str,
    ) -> PyResult<()> {
        Self::edit(slf, &[&node], |doc| {
            doc.set_arg_delimiters(node.id, index, open, close)
        })
    }

    fn set_delimiters(
        slf: &Bound<'_, Self>,
        node: PyRef<'_, PyNode>,
        left: &str,
        right: &str,
    ) -> PyResult<()> {
        Self::edit(slf, &[&node], |doc| {
            doc.set_delimiters(node.id, left, right)
        })
    }

    fn set_prime_count(
        slf: &Bound<'_, Self>,
        node: PyRef<'_, PyNode>,
        count: i128,
    ) -> PyResult<()> {
        Self::edit(slf, &[&node], |doc| {
            doc.set_prime_count(node.id, prime_count(count))
        })
    }

    fn clone_node(slf: &Bound<'_, Self>, node: PyRef<'_, PyNode>) -> PyResult<Py<PyNode>> {
        Self::edit_node(slf, &[&node], |doc| doc.clone_node(node.id))
    }

    fn import_node(slf: &Bound<'_, Self>, node: PyRef<'_, PyNode>) -> PyResult<Py<PyNode>> {
        if node.doc.bind(slf.py()).is(slf) {
            return Self::clone_node(slf, node);
        }
        let source = node.doc.try_borrow(slf.py()).map_err(borrow_error)?;
        Self::edit_node(slf, &[], |doc| doc.import_node(&source.inner, node.id))
    }

    fn node_at(slf: &Bound<'_, Self>, path: &str) -> PyResult<Py<PyNode>> {
        let id = slf
            .try_borrow()
            .map_err(borrow_error)?
            .inner
            .node_at(path)
            .map_err(edit_error)?
            .id();
        py_node(slf.py(), slf.clone().unbind(), id)
    }

    fn set_arg(
        slf: &Bound<'_, Self>,
        node: PyRef<'_, PyNode>,
        index: usize,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let value = py_arg(slf, value)?;
        Self::edit(slf, &[&node], |doc| doc.set_arg(node.id, index, value))
    }
}

#[pyclass(name = "Node")]
struct PyNode {
    doc: Py<PyDocument>,
    id: texform::NodeId,
}

impl PyNode {
    fn with_ref<T>(
        &self,
        py: Python<'_>,
        read: impl FnOnce(texform::NodeRef<'_>) -> T,
    ) -> PyResult<T> {
        let document = self.doc.try_borrow(py).map_err(borrow_error)?;
        Ok(read(document.inner.node(self.id).map_err(edit_error)?))
    }

    /// A handle to the node selected by `read`, or `None`.
    fn related(
        &self,
        py: Python<'_>,
        read: impl FnOnce(texform::NodeRef<'_>) -> Option<texform::NodeRef<'_>>,
    ) -> PyResult<Py<PyAny>> {
        let id = self.with_ref(py, |node| read(node).map(|node| node.id()))?;
        py_optional_node(py, self.doc.clone_ref(py), id)
    }

    /// Handles to the nodes selected by `read`.
    fn related_list(
        &self,
        py: Python<'_>,
        read: impl FnOnce(texform::NodeRef<'_>) -> Vec<texform::NodeId>,
    ) -> PyResult<Py<PyAny>> {
        let ids = self.with_ref(py, read)?;
        py_nodes_list(py, &self.doc, ids)
    }

    /// Expose an argument, attaching a live handle for content arguments.
    fn arg_to_py(
        &self,
        py: Python<'_>,
        arg: Option<texform::bindings::ArgRefDto>,
    ) -> PyResult<Py<PyAny>> {
        let Some(arg) = arg else {
            return Ok(py.None());
        };
        let value = pythonize(py, &arg)?;
        if let Some(id) = arg.node {
            value.set_item("node", py_node(py, self.doc.clone_ref(py), id)?)?;
        }
        Ok(value.unbind())
    }
}

#[pymethods]
impl PyNode {
    fn __eq__(&self, py: Python<'_>, other: PyRef<'_, Self>) -> bool {
        self.id == other.id && self.doc.bind(py).is(other.doc.bind(py))
    }
    fn __hash__(&self) -> u64 {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut state);
        (self.doc.as_ptr() as usize).hash(&mut state);
        state.finish()
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        match self.doc.try_borrow(py).ok().and_then(|doc| {
            doc.inner.node(self.id).ok().map(|node| {
                let name = node
                    .command_name()
                    .or_else(|| node.env_name())
                    .unwrap_or("");
                format!(
                    "<Node {:?} {name} at {}>",
                    node.kind(),
                    node.path().as_deref().unwrap_or("detached")
                )
            })
        }) {
            Some(value) => value,
            None => format!("<Node unavailable {:?}>", self.id),
        }
    }
    fn path(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.with_ref(py, |node| node.path())
    }

    fn slot(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let slot = self.with_ref(py, |node| {
            node.slot().map(texform::bindings::NodeSlotDto::from)
        })?;
        Ok(pythonize(py, &slot)?.unbind())
    }

    fn is_known(&self, py: Python<'_>) -> PyResult<Option<bool>> {
        self.with_ref(py, |node| node.is_known())
    }

    fn document(&self, py: Python<'_>) -> Py<PyDocument> {
        self.doc.clone_ref(py)
    }

    #[pyo3(signature = (name = None))]
    fn is_command(&self, py: Python<'_>, name: Option<&str>) -> PyResult<bool> {
        self.with_ref(py, |node| match name {
            Some(name) => node.is_command(name),
            None => node.kind() == texform::NodeKind::Command,
        })
    }

    #[pyo3(signature = (value = None))]
    fn is_char(&self, py: Python<'_>, value: Option<&str>) -> PyResult<bool> {
        let value = value
            .map(texform::bindings::parse_char)
            .transpose()
            .map_err(edit_error)?;
        self.with_ref(py, |node| match value {
            Some(value) => node.is_char(value),
            None => node.kind() == texform::NodeKind::Char,
        })
    }

    fn is_error(&self, py: Python<'_>) -> PyResult<bool> {
        self.with_ref(py, |node| node.is_error())
    }

    fn kind(&self, py: Python<'_>) -> PyResult<String> {
        self.with_ref(py, |node| format!("{:?}", node.kind()))
    }

    fn parent(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.parent())
    }

    fn children(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related_list(py, |node| node.children().map(|node| node.id()).collect())
    }

    fn next_sibling(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.next_sibling())
    }

    fn prev_sibling(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.prev_sibling())
    }

    fn ancestors(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related_list(py, |node| node.ancestors().map(|node| node.id()).collect())
    }

    fn descendants(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related_list(py, |node| {
            node.descendants().map(|node| node.id()).collect()
        })
    }

    fn command_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.with_ref(py, |node| node.command_name().map(ToOwned::to_owned))
    }

    fn env_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.with_ref(py, |node| node.env_name().map(ToOwned::to_owned))
    }

    fn text(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.with_ref(py, |node| node.text().map(ToOwned::to_owned))
    }

    fn char(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.with_ref(py, |node| node.char().map(|ch| ch.to_string()))
    }

    fn prime_count(&self, py: Python<'_>) -> PyResult<Option<usize>> {
        self.with_ref(py, |node| node.prime_count())
    }

    fn error_parts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let parts = self.with_ref(py, |node| {
            node.error_parts()
                .map(|(message, snippet)| (message.to_string(), snippet.to_string()))
        })?;
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

    fn content_mode(&self, py: Python<'_>) -> PyResult<Option<&'static str>> {
        self.with_ref(py, |node| node.content_mode().map(ContentMode::as_str))
    }

    fn group_kind(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let kind = self.with_ref(py, |node| {
            node.group_kind().map(texform::bindings::GroupKindDto::from)
        })?;
        Ok(pythonize(py, &kind)?.unbind())
    }

    fn arg_count(&self, py: Python<'_>) -> PyResult<usize> {
        self.with_ref(py, |node| node.arg_count())
    }

    fn arg(&self, py: Python<'_>, index: usize) -> PyResult<Py<PyAny>> {
        let arg = self.with_ref(py, |node| texform::bindings::arg_ref_to_dto(node, index))?;
        self.arg_to_py(py, arg)
    }

    fn arg_slots(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let args = self.with_ref(py, |node| {
            (0..node.arg_count())
                .map(|index| texform::bindings::arg_ref_to_dto(node, index))
                .collect::<Vec<_>>()
        })?;
        let args = args
            .into_iter()
            .map(|arg| self.arg_to_py(py, arg))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyList::new(py, args)?.unbind().into_any())
    }

    fn script_base(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.script_base())
    }

    fn subscript(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.subscript())
    }

    fn superscript(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.superscript())
    }

    fn infix_left(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.infix_left())
    }

    fn infix_right(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.infix_right())
    }

    fn env_body(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.related(py, |node| node.env_body())
    }

    fn span(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let span = self.with_ref(py, |node| node.span())?;
        Ok(pythonize(py, &span)?.unbind())
    }
}

#[pyclass(name = "Parser")]
struct PyParser {
    inner: texform::Parser,
}

#[pymethods]
impl PyParser {
    #[new]
    #[pyo3(signature = (knowledge_base = None, *, default_parse_config = None))]
    fn new(
        knowledge_base: Option<PyRef<'_, PyKnowledgeBase>>,
        default_parse_config: Option<PyRef<'_, PyParseConfig>>,
    ) -> Self {
        let mut builder =
            texform::Parser::builder().knowledge_base(knowledge_base_value(knowledge_base));
        if let Some(config) = default_parse_config {
            builder = builder.default_parse_config(config.to_core());
        }
        Self {
            inner: builder.build(),
        }
    }

    #[pyo3(signature = (src, config = None, **overrides))]
    fn parse(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let config =
            parse_config_from_python(config, overrides, self.inner.default_parse_config().clone())?;
        parse_result_to_python(py, self.inner.parse_with(src, &config))
    }

    fn default_parse_config(&self) -> PyParseConfig {
        PyParseConfig::from_core(self.inner.default_parse_config().clone())
    }

    fn knowledge_base(&self) -> PyKnowledgeBase {
        PyKnowledgeBase {
            inner: self.inner.knowledge_base().clone(),
        }
    }
}

#[pyclass(name = "TransformEngine")]
struct PyTransformEngine {
    inner: texform::TransformEngine,
}

#[pymethods]
impl PyTransformEngine {
    #[new]
    #[pyo3(signature = (profile, knowledge_base = None, *, disable_rules = None, default_parse_config = None))]
    fn new(
        profile: &str,
        knowledge_base: Option<PyRef<'_, PyKnowledgeBase>>,
        disable_rules: Option<Vec<String>>,
        default_parse_config: Option<PyRef<'_, PyParseConfig>>,
    ) -> PyResult<Self> {
        let mut builder = texform::TransformEngine::builder()
            .profile(profile_from_name(profile)?)
            .knowledge_base(knowledge_base_value(knowledge_base));
        if let Some(config) = default_parse_config {
            builder = builder.default_parse_config(config.to_core());
        }
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

    #[pyo3(signature = (src, config = None, **overrides))]
    fn normalize(
        &self,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        // Plain path returns text only. It shares config parsing with
        // `normalize_with_report` and does not build a report DTO.
        let config =
            normalize_config_from_python(config, overrides, self.inner.default_normalize_config())?;
        self.inner
            .normalize_with(src, &config)
            .map_err(normalize_error)
    }

    #[pyo3(signature = (src, config = None, **overrides))]
    fn normalize_with_report(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let config =
            normalize_config_from_python(config, overrides, self.inner.default_normalize_config())?;
        let result = self
            .inner
            .normalize_with_report(src, &config)
            .map_err(normalize_error)?;
        normalize_report_result_to_python(py, result.normalized, &result.report)
    }

    // Unstable research entry: omitted from the public stub. Always collects a
    // report and always validates `guards`, even when FlattenGroups is disabled.
    #[pyo3(signature = (source, config = None, *, guards, **overrides))]
    fn _normalize_with_flatten_groups_guards(
        &self,
        py: Python<'_>,
        source: &str,
        config: Option<&Bound<'_, PyAny>>,
        guards: &Bound<'_, PyAny>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        if guards.cast::<PyDict>().is_err() {
            return Err(ConfigError::new_err(format!(
                "invalid guards: unsupported value of type `{}`; pass a dict",
                match guards.get_type().name() {
                    Ok(name) => name.to_string(),
                    Err(_) => "unknown".to_string(),
                }
            )));
        }
        let overlay = from_python::<texform::FlattenGroupsGuardsOverlay>(guards, "guards")?;
        let config =
            normalize_config_from_python(config, overrides, self.inner.default_normalize_config())?;
        let result = self
            .inner
            .normalize_with_flatten_groups_guards(source, &config, &overlay)
            .map_err(normalize_error)?;
        normalize_report_result_to_python(py, result.normalized, &result.report)
    }

    #[pyo3(signature = (document, config = None, **overrides))]
    fn transform(
        &self,
        document: &Bound<'_, PyDocument>,
        config: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        // Plain path returns None. It shares config parsing with
        // `transform_with_report` and does not build a report DTO.
        let config = transform_config_from_python(
            config,
            overrides,
            *self.inner.default_transform_config(),
        )?;
        {
            let mut document = document.try_borrow_mut().map_err(borrow_error)?;
            self.inner.transform_with(&mut document.inner, &config)
        }
        .map_err(normalize_error)?;
        Ok(())
    }

    #[pyo3(signature = (document, config = None, **overrides))]
    fn transform_with_report(
        &self,
        py: Python<'_>,
        document: &Bound<'_, PyDocument>,
        config: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let config = transform_config_from_python(
            config,
            overrides,
            *self.inner.default_transform_config(),
        )?;
        let report = {
            let mut document = document.try_borrow_mut().map_err(borrow_error)?;
            self.inner
                .transform_with_report(&mut document.inner, &config)
        }
        .map_err(normalize_error)?;
        transform_report_to_python(py, &report)
    }

    #[pyo3(signature = (src, config = None, **overrides))]
    fn parse(
        &self,
        py: Python<'_>,
        src: &str,
        config: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let config = parse_config_from_python(
            config,
            overrides,
            self.inner.parser().default_parse_config().clone(),
        )?;
        parse_result_to_python(py, self.inner.parser().parse_with(src, &config))
    }

    fn default_parse_config(&self) -> PyParseConfig {
        PyParseConfig::from_core(self.inner.parser().default_parse_config().clone())
    }

    fn default_transform_config(&self, py: Python<'_>) -> PyResult<PyTransformConfig> {
        PyTransformConfig::from_core(py, *self.inner.default_transform_config())
    }

    fn knowledge_base(&self) -> PyKnowledgeBase {
        PyKnowledgeBase {
            inner: self.inner.knowledge_base().clone(),
        }
    }
}

fn normalize_report_result_to_python(
    py: Python<'_>,
    normalized: String,
    report: &texform::diagnostics::TransformReport,
) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    out.set_item("normalized", normalized)?;
    out.set_item("report", transform_report_to_python(py, report)?)?;
    Ok(out.unbind().into_any())
}

fn transform_report_to_python(
    py: Python<'_>,
    report: &texform::diagnostics::TransformReport,
) -> PyResult<Py<PyAny>> {
    Ok(pythonize(py, &texform::bindings::transform_report_to_dto(report))?.unbind())
}

#[pyfunction]
#[pyo3(signature = (node, **options))]
fn serialize(node: &Bound<'_, PyAny>, options: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let node = syntax_node(node)?;
    let options = serialize_options_from_python(options)?;
    texform::bindings::serialize_syntax(&node, &options)
        .map_err(|error| binding_error_to_py(error, None))
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
#[pyo3(signature = (src, config = None, *, knowledge_base = None))]
fn count_targets(
    py: Python<'_>,
    src: &str,
    config: Option<PyRef<'_, PyParseConfig>>,
    knowledge_base: Option<PyRef<'_, PyKnowledgeBase>>,
) -> PyResult<Py<PyAny>> {
    let ctx = texform::Parser::builder()
        .knowledge_base(knowledge_base_value(knowledge_base))
        .build();
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
    m.add_class::<PyKnowledgeBase>()?;
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
    m.add("ConformanceError", m.py().get_type::<ConformanceError>())?;
    m.add_class::<PyPaired>()?;
    m.add("ConfigError", m.py().get_type::<ConfigError>())?;
    m.add("TransformError", m.py().get_type::<TransformError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_invalid_character_raises_ordinary_parse_error() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call1(("corpus",))
                .unwrap();
            let error = engine
                .call_method1("normalize", ("\u{1b}",))
                .expect_err("invalid input should raise a parse error");
            // A lexer panic would surface as `PanicException`, which `except Exception` misses.
            assert!(error.is_instance_of::<ParseError>(py));
            assert!(error.is_instance_of::<PyException>(py));
            assert!(error.value(py).getattr("document").unwrap().is_none());
        });
    }

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

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("reject_unknown", true).unwrap();
            let result = parser
                .call_method("parse", (r"\unknowncmd",), Some(&kwargs))
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
    fn python_normalize_incomplete_input_with_abort_off_is_parse_error() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = PyDict::new(py);
            kwargs.set_item("profile", "corpus").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();
            let overrides = PyDict::new(py);
            overrides.set_item("abort_on_error", false).unwrap();
            let error = engine
                .call_method("normalize", (r"\sqrt[",), Some(&overrides))
                .expect_err("normalize should raise a parse error");

            assert!(error.is_instance_of::<ParseError>(py));
            assert!(error.is_instance_of::<TexformError>(py));
            let value = error.value(py);
            let diagnostics = value.getattr("diagnostics").unwrap();
            assert!(diagnostics.cast::<PyList>().is_ok());
            assert!(diagnostics.len().unwrap() > 0);
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
            let local = first.call_method1("create_char", ("a",)).unwrap();
            first
                .call_method1("append_child", (&root, &local))
                .expect("same-document child should attach");
            let foreign = second.call_method1("create_char", ("x",)).unwrap();
            let first_latex = first
                .call_method0("to_latex")
                .unwrap()
                .extract::<String>()
                .unwrap();
            let second_latex = second
                .call_method0("to_latex")
                .unwrap()
                .extract::<String>()
                .unwrap();

            let error = first
                .call_method1("append_child", (&root, &foreign))
                .expect_err("foreign child should be rejected");
            assert!(error.is_instance_of::<EditError>(py));
            assert!(error.to_string().contains("different document"));
            assert_eq!(
                first
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                first_latex
            );
            assert_eq!(
                second
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                second_latex
            );

            let error = first
                .call_method1("replace_with", (&local, &foreign))
                .expect_err("foreign replacement should be rejected");
            assert!(error.is_instance_of::<EditError>(py));
            assert!(error.to_string().contains("different document"));
            assert_eq!(
                first
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                first_latex
            );
            assert_eq!(
                second
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                second_latex
            );
        });
    }

    #[test]
    fn python_create_command_with_arg_roundtrips_latex() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let document = module.getattr("Document").unwrap().call0().unwrap();
            let arg = document.call_method1("create_char", ("x",)).unwrap();
            let arg_value = arg;

            let command = document
                .call_method1("create_command", ("sqrt", vec![arg_value]))
                .unwrap();
            let root = document.call_method0("root").unwrap();
            document
                .call_method1("append_child", (root, &command))
                .unwrap();

            let read_arg_value = command.call_method1("arg", (1usize,)).unwrap();
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
    fn python_alignment_tab_is_distinct_from_literal_ampersand() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let document = module.getattr("Document").unwrap().call0().unwrap();
            let root = document.call_method0("root").unwrap();
            let tab = document.call_method0("create_alignment_tab").unwrap();
            assert_eq!(
                tab.call_method0("kind")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "AlignmentTab"
            );
            let literal = document.call_method1("create_char", ("&",)).unwrap();
            for node in [&tab, &literal] {
                document
                    .call_method1("append_child", (&root, node))
                    .unwrap();
            }
            assert_eq!(
                document
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"& \&"
            );
        });
    }

    #[test]
    fn python_rejects_read_only_error_document_editing() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let kwargs = PyDict::new(py);
            kwargs.set_item("reject_unknown", true).unwrap();
            let result = parser
                .call_method("parse", (r"\unknowncmd",), Some(&kwargs))
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
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base", "physics"],))
                        .unwrap(),
                )
                .unwrap();
            kwargs.set_item("profile", "authoring").unwrap();
            let engine = engine_cls.call((), Some(&kwargs)).unwrap();

            let result = engine
                .call_method1("normalize", (r"\quantity{x}",))
                .unwrap();

            assert_eq!(result.extract::<String>().unwrap(), r"\qty { x }");
        });
    }

    #[test]
    fn python_parser_empty_packages_means_empty_knowledge() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser_cls = module.getattr("Parser").unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((Vec::<String>::new(),))
                        .unwrap(),
                )
                .unwrap();
            let parser = parser_cls.call((), Some(&kwargs)).unwrap();

            let knows_frac = parser
                .call_method0("knowledge_base")
                .unwrap()
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
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((Vec::<String>::new(),))
                        .unwrap(),
                )
                .unwrap();
            let kb_kwargs = PyDict::new(py);
            kb_kwargs.set_item("items", vec![item]).unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call((Vec::<String>::new(),), Some(&kb_kwargs))
                        .unwrap(),
                )
                .unwrap();
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
    fn python_parser_supports_kwarg_overrides_and_rejects_dict_config() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser_cls = module.getattr("Parser").unwrap();
            let parser = parser_cls.call0().unwrap();

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("reject_unknown", true).unwrap();
            let result = parser
                .call_method("parse", (r"\unknowncmd",), Some(&kwargs))
                .expect("reject_unknown override should return diagnostics");
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

            let config_cls = module.getattr("ParseConfig").unwrap();
            let config_kwargs = pyo3::types::PyDict::new(py);
            config_kwargs.set_item("reject_unknown", true).unwrap();
            let config = config_cls.call((), Some(&config_kwargs)).unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("config", config).unwrap();
            kwargs.set_item("reject_unknown", false).unwrap();
            parser
                .call_method("parse", (r"\unknowncmd",), Some(&kwargs))
                .expect("kwargs should overlay a complete ParseConfig");

            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("reject_unknown", true).unwrap();
            let error = parser
                .call_method1("parse", (r"\unknowncmd", dict))
                .expect_err("dict as config should fail");
            assert!(error.is_instance_of::<ConfigError>(py));
            assert!(error.to_string().contains("**overrides"));
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
                    .call_method0("knowledge_base")
                    .unwrap()
                    .call_method1("lookup_command", ("frac", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method0("knowledge_base")
                    .unwrap()
                    .call_method1("lookup_explicit_command", ("frac", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method0("knowledge_base")
                    .unwrap()
                    .call_method1("lookup_character", ("le", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method0("knowledge_base")
                    .unwrap()
                    .call_method1("lookup_env", ("array", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                parser
                    .call_method0("knowledge_base")
                    .unwrap()
                    .call_method1("is_delimiter_control", ("lbrace",))
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert!(
                parser
                    .call_method0("knowledge_base")
                    .unwrap()
                    .call_method1("knows_env_name", ("array",))
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert!(
                parser
                    .call_method0("knowledge_base")
                    .unwrap()
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
                    .call_method0("knowledge_base")
                    .unwrap()
                    .call_method1("lookup_command", ("frac", "math"))
                    .unwrap()
                    .cast::<pyo3::types::PyDict>()
                    .is_ok()
            );
            assert!(
                engine
                    .call_method0("knowledge_base")
                    .unwrap()
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
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base", "physics"],))
                        .unwrap(),
                )
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
            assert_eq!(result.extract::<String>().unwrap(), r"\qty { x }");
        });
    }

    #[test]
    fn python_engine_normalize_kwargs_disable_pipeline() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "authoring").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base", "physics"],))
                        .unwrap(),
                )
                .unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let rewrite = pyo3::types::PyDict::new(py);
            rewrite.set_item("enabled", false).unwrap();
            let lower_attributes = pyo3::types::PyDict::new(py);
            lower_attributes.set_item("enabled", false).unwrap();
            let finalize_ast = pyo3::types::PyDict::new(py);
            finalize_ast.set_item("enabled", false).unwrap();
            let flatten_groups = pyo3::types::PyDict::new(py);
            flatten_groups.set_item("enabled", false).unwrap();
            let call_kwargs = pyo3::types::PyDict::new(py);
            call_kwargs.set_item("rewrite", rewrite).unwrap();
            call_kwargs
                .set_item("lower_attributes", lower_attributes)
                .unwrap();
            call_kwargs.set_item("finalize_ast", finalize_ast).unwrap();
            call_kwargs
                .set_item("flatten_groups", flatten_groups)
                .unwrap();

            let result = engine
                .call_method("normalize", (r"\quantity{x}",), Some(&call_kwargs))
                .expect("normalize should accept kwargs");
            assert_eq!(result.extract::<String>().unwrap(), r"\quantity { x }");
        });
    }

    #[test]
    fn python_engine_transform_updates_own_document_in_place() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
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
            let rewrite = pyo3::types::PyDict::new(py);
            rewrite.set_item("enabled", false).unwrap();
            let lower_attributes = pyo3::types::PyDict::new(py);
            lower_attributes.set_item("enabled", false).unwrap();
            let flatten_groups = pyo3::types::PyDict::new(py);
            flatten_groups.set_item("enabled", true).unwrap();
            let overrides = pyo3::types::PyDict::new(py);
            overrides.set_item("rewrite", rewrite).unwrap();
            overrides
                .set_item("lower_attributes", lower_attributes)
                .unwrap();
            overrides
                .set_item("flatten_groups", flatten_groups)
                .unwrap();

            let transformed = engine
                .call_method("transform", (&document,), Some(&overrides))
                .expect("transform should succeed");
            assert!(transformed.is_none());

            assert_eq!(
                document
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "x"
            );

            let parsed_again = engine.call_method1("parse", ("{{x}}",)).unwrap();
            let reported = parsed_again
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            let report = engine
                .call_method("transform_with_report", (&reported,), Some(&overrides))
                .expect("transform with report should succeed");
            let report = report.cast::<pyo3::types::PyDict>().unwrap();
            assert!(report.get_item("flatten_groups").unwrap().is_some());
            assert!(report.get_item("iterations").unwrap().is_none());
            assert_eq!(
                reported
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "x"
            );
        });
    }

    #[test]
    fn python_engine_transform_rejects_document_with_default_knowledge_when_customized() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
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
                .expect_err("documents with different knowledge must not be transformed");

            assert!(error.is_instance_of::<TransformError>(py));
        });
    }

    #[test]
    fn python_engine_transform_accepts_document_from_another_engine_sharing_knowledge() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
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

            second_engine
                .call_method1("transform", (document,))
                .expect("shared knowledge permits transformation");
        });
    }

    #[test]
    fn python_engine_transform_rejects_document_with_parse_errors() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = PyDict::new(py);
            kwargs.set_item("profile", "corpus").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let parse_kwargs = PyDict::new(py);
            parse_kwargs.set_item("abort_on_error", false).unwrap();
            let parsed = engine
                .call_method("parse", (r"\frac{a}{b}\sqrt[",), Some(&parse_kwargs))
                .unwrap();
            let document = parsed
                .cast::<PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            assert!(
                document
                    .call_method0("has_errors")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            let latex_before = document
                .call_method0("to_latex")
                .unwrap()
                .extract::<String>()
                .unwrap();

            let error = engine
                .call_method1("transform", (document.clone(),))
                .expect_err("documents with parse errors must not be transformed");

            assert!(error.is_instance_of::<TransformError>(py));
            assert!(error.is_instance_of::<TexformError>(py));
            assert_eq!(
                document
                    .call_method0("to_latex")
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                latex_before
            );
        });
    }

    #[test]
    fn python_engine_transform_invalid_config_raises_config_error() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
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

            let transformed = engine
                .call_method1("transform", (document.clone(), py.None()))
                .expect("None config should be accepted");
            assert!(transformed.is_none());

            let parsed_again = engine.call_method1("parse", ("x",)).unwrap();
            let reported = parsed_again
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            let report = engine
                .call_method1("transform_with_report", (reported.clone(), py.None()))
                .expect("None config should be accepted by the report path");
            assert!(report.cast::<pyo3::types::PyDict>().is_ok());

            let error = engine
                .call_method1("transform", (document, "not a config"))
                .expect_err("invalid config object should fail");
            assert!(error.is_instance_of::<ConfigError>(py));

            let error = engine
                .call_method1("transform_with_report", (reported, "not a config"))
                .expect_err("invalid report config object should fail");
            assert!(error.is_instance_of::<ConfigError>(py));
        });
    }

    #[test]
    fn python_invalid_per_call_config_raises_config_error() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("profile", "equiv").unwrap();
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let error = engine
                .call_method1("parse", ("x", "not a config"))
                .expect_err("invalid parse config should fail");
            assert!(error.is_instance_of::<ConfigError>(py));

            let error = engine
                .call_method1("normalize", ("x", "not a config"))
                .expect_err("invalid normalize config should fail");
            assert!(error.is_instance_of::<ConfigError>(py));

            let error = engine
                .call_method1("normalize_with_report", ("x", "not a config"))
                .expect_err("invalid normalize report config should fail");
            assert!(error.is_instance_of::<ConfigError>(py));

            let finalize_kwargs = pyo3::types::PyDict::new(py);
            finalize_kwargs.set_item("finalize_ast", 42).unwrap();
            let error = engine
                .call_method("normalize", ("x",), Some(&finalize_kwargs))
                .expect_err("invalid finalize_ast should fail");
            assert!(error.is_instance_of::<ConfigError>(py));

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let error = parser
                .call_method1("parse", ("x", "not a config"))
                .expect_err("invalid parser config should fail");
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
            kwargs
                .set_item(
                    "knowledge_base",
                    module
                        .getattr("KnowledgeBase")
                        .unwrap()
                        .call1((vec!["base"],))
                        .unwrap(),
                )
                .unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();

            let plain = engine
                .call_method1("normalize", (r"f^{\prime\prime}",))
                .expect("normalize should use default FinalizeAst");
            assert_eq!(plain.extract::<String>().unwrap(), "f''");

            let result = engine
                .call_method1("normalize_with_report", (r"f^{\prime\prime}",))
                .expect("normalize with report should use default FinalizeAst");
            let dict = result.cast::<pyo3::types::PyDict>().unwrap();
            assert_eq!(
                dict.get_item("normalized")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                plain.extract::<String>().unwrap()
            );
            let report_value = dict.get_item("report").unwrap().unwrap();
            let report = report_value.cast::<pyo3::types::PyDict>().unwrap();
            let finalize_ast = report.get_item("finalize_ast").unwrap().unwrap();
            let finalize_ast = finalize_ast.cast::<pyo3::types::PyDict>().unwrap();
            assert!(finalize_ast.get_item("prime_run_merges").unwrap().is_some());
            assert!(
                finalize_ast
                    .get_item("text_normalizations")
                    .unwrap()
                    .is_some()
            );
            assert!(finalize_ast.get_item("steps").unwrap().is_none());
            assert!(report.get_item("finalizeAst").unwrap().is_none());
            assert!(report.get_item("iterations").unwrap().is_none());

            let finalize_ast = pyo3::types::PyDict::new(py);
            finalize_ast.set_item("enabled", false).unwrap();
            let call_kwargs = pyo3::types::PyDict::new(py);
            call_kwargs.set_item("finalize_ast", finalize_ast).unwrap();
            let disabled = engine
                .call_method("normalize", (r"f^{\prime\prime}",), Some(&call_kwargs))
                .expect("normalize should accept finalize_ast kwargs");
            assert_eq!(
                disabled.extract::<String>().unwrap(),
                r"f ^ { \prime \prime }"
            );
            let disabled_report = engine
                .call_method(
                    "normalize_with_report",
                    (r"f^{\prime\prime}",),
                    Some(&call_kwargs),
                )
                .expect("report path should accept the same finalize_ast kwargs");
            assert_eq!(
                disabled_report
                    .cast::<pyo3::types::PyDict>()
                    .unwrap()
                    .get_item("normalized")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                r"f ^ { \prime \prime }"
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
    fn python_serialize_options_accept_flat_kwargs_and_reject_nested_keys() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let parsed = parser.call_method1("parse", ("x_i^2",)).unwrap();
            let document = parsed
                .cast::<pyo3::types::PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();

            let default = document
                .call_method0("to_latex")
                .unwrap()
                .extract::<String>()
                .unwrap();
            assert_eq!(default, "x _ { i } ^ { 2 }");

            let kwargs = PyDict::new(py);
            kwargs.set_item("script_spacing", "compact").unwrap();
            kwargs.set_item("script_order", "sup_first").unwrap();
            let compact = document
                .call_method("to_latex", (), Some(&kwargs))
                .unwrap()
                .extract::<String>()
                .unwrap();
            assert_eq!(compact, "x^{ 2 }_{ i }");

            let none_kwargs = PyDict::new(py);
            none_kwargs.set_item("script_order", py.None()).unwrap();
            let omitted = document
                .call_method("to_latex", (), Some(&none_kwargs))
                .unwrap()
                .extract::<String>()
                .unwrap();
            assert_eq!(omitted, default);

            let nested = PyDict::new(py);
            let math = PyDict::new(py);
            math.set_item("scripts", PyDict::new(py)).unwrap();
            nested.set_item("math", math).unwrap();
            let error = document
                .call_method("to_latex", (), Some(&nested))
                .expect_err("legacy nested keys should fail");
            assert!(error.is_instance_of::<ConfigError>(py));
            let message = error.to_string();
            assert!(message.contains("unknown field `math`"), "{message}");
            assert!(message.contains("script_order"), "{message}");

            let misspelled = PyDict::new(py);
            misspelled.set_item("script_ordre", "sup_first").unwrap();
            let error = document
                .call_method("to_latex", (), Some(&misspelled))
                .expect_err("unknown field should fail");
            assert!(error.is_instance_of::<ConfigError>(py));
            assert!(error.to_string().contains("script_ordre"), "{}", error);

            let camel = PyDict::new(py);
            camel.set_item("scriptOrder", "sup_first").unwrap();
            let error = document
                .call_method("to_latex", (), Some(&camel))
                .expect_err("camelCase keys should fail in Python");
            assert!(error.is_instance_of::<ConfigError>(py));

            let syntax = document.call_method0("to_syntax").unwrap();
            let serialize_kwargs = PyDict::new(py);
            serialize_kwargs
                .set_item("group_inner_spacing", "compact")
                .unwrap();
            let serialized = module
                .getattr("serialize")
                .unwrap()
                .call((syntax,), Some(&serialize_kwargs))
                .unwrap()
                .extract::<String>()
                .unwrap();
            assert_eq!(serialized, "x _ {i} ^ {2}");
        });
    }

    #[test]
    fn python_module_profile_names_use_faithful_and_reject_corpus_drop() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let config_cls = module.getattr("TransformConfig").unwrap();
            assert!(config_cls.call_method0("corpus_drop").is_err());

            let standalone = module
                .getattr("FlattenGroupsConfig")
                .unwrap()
                .call0()
                .unwrap();
            assert!(
                standalone
                    .getattr("enabled")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
            assert!(
                standalone
                    .getattr("preserve_rendered_spacing")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );

            let faithful = config_cls.call_method0("faithful").unwrap();
            let faithful_flatten_groups = faithful.getattr("flatten_groups").unwrap();
            assert!(
                faithful_flatten_groups
                    .getattr("preserve_rendered_spacing")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );

            let config = config_cls.call_method0("corpus").unwrap();
            let flatten_groups = config.getattr("flatten_groups").unwrap();

            assert!(
                !flatten_groups
                    .getattr("preserve_rendered_spacing")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
        });
    }

    fn run_python_test(source: &std::ffi::CStr) {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");
            let globals = PyDict::new(py);
            globals.set_item("texform", module).unwrap();
            py.run(source, Some(&globals), None).unwrap();
        });
    }

    #[test]
    fn python_matches_shared_binding_cases() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");
            let globals = PyDict::new(py);
            globals.set_item("texform", module).unwrap();
            globals
                .set_item(
                    "CASES",
                    include_str!("../../texform/tests/binding_cases.json"),
                )
                .unwrap();
            py.run(
                cr#"
import json
errors = {
    "conformance": texform.ConformanceError,
    "edit": texform.EditError,
    "parse": texform.ParseError,
}
def value(arg):
    if isinstance(arg, list):
        return [value(item) for item in arg]
    if isinstance(arg, dict):
        return texform.Paired(value(arg["value"]), arg["open"], arg["close"])
    return arg
for case in json.loads(CASES):
    kb = texform.KnowledgeBase(case["packages"]) if "packages" in case else None
    doc = texform.Document(kb, mode=case.get("root", "math"))
    latex = None
    try:
        if case["call"] == "serialize":
            latex = texform.serialize(*case["args"])
        elif case["call"] == "from_syntax":
            latex = texform.Document.from_syntax(case["args"][0], kb).to_latex()
        else:
            options = {"mode": case["mode"]} if "mode" in case else {}
            node = getattr(doc, case["call"])(*map(value, case["args"]), **options)
            if "latex" in case:
                doc.append_child(doc.root(), node)
                latex = doc.to_latex()
    except texform.TexformError as error:
        assert type(error) is errors[case.get("error")], (case["name"], error)
        assert "rule" not in case or error.rule == case["rule"], case["name"]
        continue
    assert "error" not in case, case["name"]
    assert latex == case.get("latex"), (case["name"], latex)
"#,
                Some(&globals),
                None,
            )
            .unwrap();
        });
    }

    #[test]
    fn python_editing_paths_and_imports() {
        run_python_test(
            cr#"
doc = texform.Parser().parse("x_i")["document"]
scripted = doc.node_at("root.child.0")
base = scripted.script_base()
assert base.path() == "root.child.0.base"
assert base.slot() == {"kind": "script_base", "index": None}
assert scripted.slot() == {"kind": "child", "index": 0}
assert doc.root().slot() is None and doc.root().path() == "root"
assert base.is_known() is None
assert base.path() in repr(base)
assert doc.set_superscript(base, "2") == scripted
assert scripted.superscript() is not None
assert doc.set_subscript(base, None) == scripted
assert doc.set_superscript(base, None) == base
assert base.path() == "root.child.0"
variant = doc.copy()
variant.set_char(variant.node_at(base.path()), "y")
assert base.char() == "x"
for cloned in (doc.clone_node(base), doc.import_node(base)):
    assert cloned != base and cloned.char() == "x"
    assert cloned.path() is None and cloned.slot() is None
    doc.set_char(cloned, "z")
assert base.char() == "x"
imported = variant.import_node(base)
assert imported.document() is variant and imported.char() == "x"
try:
    doc.node_at("root.child.99")
except texform.EditError:
    pass
else:
    raise AssertionError("missing path must fail")
kb = texform.KnowledgeBase(["base", "ams", "physics"])
source = texform.Parser(kb).parse(r"\qty(x)")["document"]
qty = source.root().children()[0]
assert qty.is_known() is True
empty = texform.Document(texform.KnowledgeBase([]))
try:
    empty.import_node(qty)
except texform.ConformanceError as error:
    assert error.rule == "unknown_with_arguments"
else:
    raise AssertionError("cross-knowledge import must validate")
broken = texform.Parser().parse("{")["document"]
assert broken is not None and broken.has_errors()
try:
    empty.import_node(broken.root())
except texform.ConformanceError as error:
    assert error.rule == "error_node"
else:
    raise AssertionError("error subtree must fail")
empty.append_child(empty.root(), empty.create_char("x"))
assert empty.to_latex() == "x"
"#,
        );
    }

    #[test]
    fn python_slot_and_scalar_edits_preserve_forms() {
        run_python_test(
            cr#"
doc = texform.Document(texform.KnowledgeBase(["base", "ams", "physics"]))
qty = doc.create_command("qty", [texform.Paired("x", "(", ")")])
doc.set_arg(qty, 0, "y")
assert qty.arg(0)["form"] == {"kind": "paired", "open": "(", "close": ")"}
doc.set_arg_delimiters(qty, 0, "[", "]")
assert qty.arg(0)["form"] == {"kind": "paired", "open": "[", "close": "]"}
sqrt = doc.create_command("sqrt", ["x"])
doc.set_arg(sqrt, 0, "3")
assert sqrt.arg(0)["form"]["kind"] == "optional"
doc.set_arg(sqrt, 0, None)
assert sqrt.arg(0) is None
operator = doc.create_command("operatorname", ["sn"])
doc.set_arg(operator, 0, True)
assert operator.arg(0)["value"] is True
doc.set_arg(operator, 0, None)
assert operator.arg(0)["value"] is False
assert operator.arg(1)["kind"] == "OperatorName"
group = doc.create_delimited_group("(", ")", ["x"])
doc.set_delimiters(group, r"\langle", r"\rangle")
assert group.group_kind()["left"] == r"\langle"
prime = doc.create_prime(1)
doc.set_prime_count(prime, 3)
assert prime.prime_count() == 3
for count in (0, -1):
    try:
        doc.set_prime_count(prime, count)
    except texform.ConformanceError as error:
        assert error.rule == "invalid_prime_count"
    else:
        raise AssertionError("non-positive primes must fail")
assert prime.prime_count() == 3
"#,
        );
    }

    #[test]
    fn python_construction_sources_modes_pairs_and_errors() {
        run_python_test(
            cr#"
kb = texform.KnowledgeBase(["base", "ams", "physics"])
doc = texform.Document(kb)
frac = doc.create_command("frac", ["a+b", "c"])
doc.append_child(doc.root(), frac)
assert "\\frac" in doc.to_latex()
assert doc.create_command("sqrt", ["x"]).arg(0) is None
assert doc.create_command("operatorname", [True, "sn"]).arg(1)["node"].kind() == "Group"
pair = texform.Paired("x", "[", "]")
assert pair.value == "x" and pair.open == "[" and pair.close == "]"
assert pair == texform.Paired("x", "[", "]") and hash(pair) == hash(texform.Paired("x", "[", "]"))
assert pair != texform.Paired("x", "(", ")") and pair != ("x", "[", "]")
assert repr(pair) == "Paired('x', '[', ']')"
try:
    pair.open = "("
except AttributeError:
    pass
else:
    raise AssertionError("Paired must be immutable")
qty = doc.create_command("qty", [pair])
assert qty.arg(0)["node"] is not None
assert qty.arg(0)["form"] == {"kind": "paired", "open": "[", "close": "]"}
scripted = doc.create_scripted("x", sub="i", sup="2")
assert scripted.kind() == "Scripted"
assert doc.create_delimited_group("(", ")", [scripted]).kind() == "Group"
assert doc.create_group(children=["x", doc.create_prime(2)]).kind() == "Group"
assert doc.create_inline_math(["x"]).kind() == "Group"
assert doc.create_infix("over", "a", "b").kind() == "Infix"
assert doc.create_environment("matrix", body=["a", "b"]).kind() == "Environment"
assert doc.create_environment("matrix", body="a+b").kind() == "Environment"
assert doc.create_environment("matrix").kind() == "Environment"
assert doc.parse_fragment("a+b").kind() == "Group"
assert doc.create_text("hello", mode="text").kind() == "Text"
before = doc.to_syntax()
for operation in (
    lambda: doc.create_command("frac", []),
    lambda: doc.create_prime(0),
    lambda: doc.create_prime(-1),
    lambda: doc.create_char("ab"),
    lambda: doc.create_prime(1, mode="text"),
):
    try:
        operation()
    except texform.ConformanceError as error:
        assert isinstance(error, texform.EditError)
        assert isinstance(error.path, str) and error.rule
    else:
        raise AssertionError("invalid construction must fail")
    assert doc.to_syntax() == before
snapshot = doc.to_syntax()
snapshot["Root"]["children"][0]["Command"]["known"] = False
try:
    texform.Document.from_syntax(snapshot, kb)
except texform.ConformanceError as error:
    assert error.path.startswith("root")
    assert error.rule == "known_flag_mismatch"
else:
    raise AssertionError("strict import must reject inconsistent known flags")
try:
    doc.create_command("frac", ["{", "b"])
except texform.ParseError as error:
    assert error.diagnostics
else:
    raise AssertionError("invalid source must fail")
foreign = texform.Document().create_char("x")
try:
    doc.create_group(children=[foreign])
except texform.EditError:
    pass
else:
    raise AssertionError("foreign content must fail")
"#,
        );
    }

    #[test]
    fn python_knowledge_identity_copy_and_node_handles() {
        run_python_test(
            cr#"
import copy
kb = texform.KnowledgeBase(["base", "ams"])
parser = texform.Parser(kb)
engine = texform.TransformEngine("equiv", kb)
assert kb == parser.knowledge_base() == engine.knowledge_base()
assert hash(kb) == hash(parser.knowledge_base())
assert kb != texform.KnowledgeBase(["base", "ams"])
assert kb != object() and kb.__eq__(object()) is NotImplemented
try:
    kb.packages = []
except AttributeError:
    pass
else:
    raise AssertionError("knowledge must be immutable")
for method in (kb.commands, kb.environments, kb.characters):
    records = method("math")
    assert records
    assert [r["name"] for r in records] == sorted(r["name"] for r in records)
    records.clear()
    assert method("math")
assert kb.delimiters()
packages = kb.packages()
packages.clear()
assert kb.packages() == ["base", "ams"]
default = texform.Parser().knowledge_base()
assert default == texform.TransformEngine("equiv").knowledge_base()
assert default == texform.Document().knowledge_base()
assert default != texform.KnowledgeBase()
document = parser.parse("x")["document"]
root = document.root()
assert root == document.root()
assert hash(root) == hash(document.root())
assert root.document() is document
assert root != object()
assert "Node" in repr(root)
for duplicate in (document.copy(), copy.copy(document), copy.deepcopy(document)):
    assert duplicate.knowledge_base() == kb
    assert duplicate.to_syntax() == document.to_syntax()
    assert duplicate.root() != root
    child = duplicate.root().children()[0]
    duplicate.set_char(child, "y")
    assert document.to_latex() == "x"
    engine.transform(duplicate)
rebuilt = texform.Document.from_syntax(document.to_syntax(), kb)
engine.transform(rebuilt)
engine.transform(texform.Document(kb))
try:
    engine.transform(texform.Document(texform.KnowledgeBase(["base", "ams"])))
except texform.TransformError:
    pass
else:
    raise AssertionError("separate knowledge instances must mismatch")
assert texform.Document(mode="text").root().content_mode() == "text"
assert texform.count_targets(r"\frac{x}{y}", knowledge_base=kb)["cmd:frac"] == 1
for make in (texform.Parser, lambda **kw: texform.TransformEngine("equiv", **kw)):
    try:
        make(packages=["base"])
    except TypeError:
        pass
    else:
        raise AssertionError("knowledge options belong to KnowledgeBase")
"#,
        );
    }

    #[test]
    fn python_research_overlays_preserve_defaults_and_reports() {
        run_python_test(
            cr#"
for profile in ("authoring", "faithful", "corpus", "equiv"):
    engine = texform.TransformEngine(profile, knowledge_base=texform.KnowledgeBase(["base"]))
    source = r"a{} + {+}"
    baseline = engine.normalize(source)
    reported = engine.normalize_with_report(source)
    research = engine._normalize_with_flatten_groups_guards
    assert isinstance(baseline, str)
    assert research(source, guards={}) == reported
    assert reported["normalized"] == baseline
    kept = research(source, guards={"empty_group": True})
    removed = research(source, guards={"empty_group": False})
    assert kept["normalized"] != removed["normalized"]
    assert "guard_hits" in kept["report"]["flatten_groups"]
    assert "guards" not in kept["report"]["flatten_groups"]
    assert engine.normalize(source) == baseline
"#,
        );
    }

    #[test]
    fn python_research_rejects_invalid_guards_even_when_disabled() {
        run_python_test(
            cr#"
engine = texform.TransformEngine("authoring", knowledge_base=texform.KnowledgeBase(["base"]))
for enabled in (True, False):
    for guards, field in (
        ({"preserve_empty_group": False}, "preserve_empty_group"),
        ({"empty_group": None}, "empty_group"),
        ({"empty_group": 1}, "empty_group"),
        ({"empty_group": "false"}, "empty_group"),
        (["not", "a", "dict"], "guards"),
        (None, "guards"),
    ):
        try:
            engine._normalize_with_flatten_groups_guards(
                "x", guards=guards, flatten_groups={"enabled": enabled}
            )
        except texform.ConfigError as error:
            assert "guards" in str(error) and field in str(error), str(error)
        else:
            raise AssertionError((enabled, guards))
"#,
        );
    }

    #[test]
    fn python_normalize_rejects_all_old_flatten_groups_keys() {
        run_python_test(
            cr#"
engine = texform.TransformEngine("authoring", knowledge_base=texform.KnowledgeBase(["base"]))
for key in (
    "preserve_group_containing_declarative_command",
    "preserve_group_in_script_base_slot",
    "preserve_group_inside_env_body",
    "preserve_group_containing_infix",
    "preserve_group_adjacent_to_command_like",
    "preserve_group_after_scripted_command_like",
    "preserve_group_as_argument_of_command",
    "preserve_empty_group",
    "preserve_group_with_lone_atom_spacing_char",
    "preserve_group_starting_with_atom_spacing_char",
    "preserve_group_containing_delimited_pair",
):
    try:
        engine.normalize("x", flatten_groups={key: False})
    except texform.ConfigError as error:
        assert key in str(error), str(error)
    else:
        raise AssertionError(key)
"#,
        );
    }

    #[test]
    fn python_flatten_groups_null_overlays_do_not_override() {
        run_python_test(
            cr#"
for profile in ("authoring", "corpus"):
    engine = texform.TransformEngine(profile, knowledge_base=texform.KnowledgeBase(["base"]))
    source = r"a{} + \cos{A}"
    baseline = engine.normalize(source)
    for overlay in (None, {"enabled": None, "preserve_rendered_spacing": None}):
        assert engine.normalize(source, flatten_groups=overlay) == baseline
"#,
        );
    }

    #[test]
    fn python_research_respects_complete_config_then_kwargs() {
        run_python_test(
            cr#"
engine = texform.TransformEngine("authoring", knowledge_base=texform.KnowledgeBase(["base"]))
source = r"\cos{A} + a{}"
research = engine._normalize_with_flatten_groups_guards
for profile, spacing, expected in (
    ("authoring", False, r"\cos A + a"),
    ("corpus", True, engine.normalize(source)),
):
    config = getattr(texform.TransformConfig, profile)()
    assert research(source, config, guards={})["normalized"] == engine.normalize(source, config)
    assert research(source, config, guards={}) == engine.normalize_with_report(source, config)
    result = research(
        source, config, guards={}, flatten_groups={"preserve_rendered_spacing": spacing}
    )
    assert result["normalized"] == expected
"#,
        );
    }

    #[test]
    fn python_report_paths_match_plain_results_without_residue() {
        run_python_test(
            cr#"
engine = texform.TransformEngine("corpus", knowledge_base=texform.KnowledgeBase(["base", "physics"]))
source = r"\quantity{{\bf x}} + a \over b"
plain = engine.normalize(source)
first = engine.normalize_with_report(source)
assert isinstance(plain, str) and first["normalized"] == plain
try:
    engine.normalize("{")
except texform.ParseError:
    pass
else:
    raise AssertionError("invalid source should fail")
assert engine.normalize(source) == plain
second = engine.normalize_with_report(source)
assert second == first

report = first["report"]
assert set(report) == {"lower_attributes", "rewrite", "finalize_ast", "flatten_groups"}
assert set(report["rewrite"]) == {"iterations", "rules"}
assert report["rewrite"]["iterations"] > 0
rules = report["rewrite"]["rules"]
assert rules == sorted(rules, key=lambda item: item["key"])
assert any(rule["applied_count"] > 0 for rule in rules)
for rule in rules:
    assert set(rule) == {"key", "applied_count", "skipped_count"}
assert set(report["finalize_ast"]) == {"prime_run_merges", "text_normalizations"}
assert "steps" not in report["finalize_ast"]
flatten = report["flatten_groups"]
assert set(flatten["actions"]) == {
    "removed_empty",
    "replaced_single_child",
    "inlined_multi_child",
    "unwrapped_slot",
}
assert set(flatten["guard_hits"]) == {
    "declarative_scope",
    "script_base",
    "env_body",
    "infix_scope",
    "command_contact",
    "command_contact_via_scripted_base",
    "empty_group",
    "lone_atom_spacing_char",
    "leading_atom_spacing_char",
    "delimited_pair",
}
attributes = report["lower_attributes"]["attributes"]
assert attributes == sorted(attributes, key=lambda item: (item["attr"], item["value"]))
assert attributes
for item in attributes:
    assert set(item) == {"attr", "value", "consumed", "redundant", "emitted"}
    for bucket in ("consumed", "redundant", "emitted"):
        assert set(item[bucket]) == {"declaratives", "prefixes"}

off = {"rewrite": {"enabled": False}}
disabled = engine.normalize(source, **off)
disabled_report = engine.normalize_with_report(source, **off)
assert disabled == disabled_report["normalized"]
assert disabled != plain
assert disabled_report["report"]["rewrite"]["iterations"] == 0
assert disabled_report["report"]["rewrite"]["rules"] == []
for method in (engine.normalize, engine.normalize_with_report):
    try:
        method(source, rewrite="no")
    except texform.ConfigError:
        pass
    else:
        raise AssertionError(method)

def fresh():
    return engine.parse(source)["document"]

document = fresh()
assert engine.transform(document) is None
assert document.to_latex() == plain
incomplete = engine.parse(r"\sqrt[", abort_on_error=False)["document"]
before = incomplete.to_latex()
try:
    engine.transform_with_report(incomplete)
except texform.TransformError:
    pass
else:
    raise AssertionError("incomplete document")
assert incomplete.to_latex() == before
foreign = texform.Document()
try:
    engine.transform_with_report(foreign)
except texform.TransformError:
    pass
else:
    raise AssertionError("foreign document")
again = fresh()
assert engine.transform_with_report(again) == report
assert again.to_latex() == plain
assert engine.transform_with_report(fresh()) == report
"#,
        );
    }

    #[test]
    fn python_module_transform_config_repr_is_python_literal() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let config_cls = module.getattr("TransformConfig").unwrap();
            let config = config_cls.call_method0("authoring").unwrap();
            let repr = config
                .call_method0("__repr__")
                .unwrap()
                .extract::<String>()
                .unwrap();

            assert!(repr.starts_with("TransformConfig("), "{repr}");
            assert!(
                repr.contains("LowerAttributesConfig(enabled=True)"),
                "{repr}"
            );
            assert!(
                repr.contains("RewriteConfig(enabled=True, max_iterations=100)"),
                "{repr}"
            );
            assert!(!repr.contains("Py"), "{repr}");
        });
    }

    #[test]
    fn python_nested_rewrite_assignment_persists() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let kwargs = PyDict::new(py);
            kwargs.set_item("profile", "corpus").unwrap();
            let engine = module
                .getattr("TransformEngine")
                .unwrap()
                .call((), Some(&kwargs))
                .unwrap();
            let cfg = engine.call_method0("default_transform_config").unwrap();
            cfg.getattr("rewrite")
                .unwrap()
                .setattr("enabled", false)
                .unwrap();
            assert!(
                !cfg.getattr("rewrite")
                    .unwrap()
                    .getattr("enabled")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );

            let parsed = engine.call_method1("parse", (r"a \over b",)).unwrap();
            let document = parsed
                .cast::<PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            engine
                .call_method1("transform", (&document, &cfg))
                .expect("complete config should replace the baseline");
            let latex = document
                .call_method0("to_latex")
                .unwrap()
                .extract::<String>()
                .unwrap();
            assert!(latex.contains(r"\over"), "{latex}");
        });
    }

    #[test]
    fn python_default_parse_config_is_lenient_and_replaceable() {
        Python::attach(|py| {
            let module = PyModule::new(py, "_native").expect("module");
            _native(&module).expect("init module");

            let parser = module.getattr("Parser").unwrap().call0().unwrap();
            let default = parser.call_method0("default_parse_config").unwrap();
            assert!(
                !default
                    .getattr("reject_unknown")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );

            let config_cls = module.getattr("ParseConfig").unwrap();
            let config_kwargs = PyDict::new(py);
            config_kwargs.set_item("reject_unknown", true).unwrap();
            let strict = config_cls.call((), Some(&config_kwargs)).unwrap();
            let ctor = PyDict::new(py);
            ctor.set_item("default_parse_config", strict).unwrap();
            let strict_parser = module
                .getattr("Parser")
                .unwrap()
                .call((), Some(&ctor))
                .unwrap();
            let result = strict_parser
                .call_method1("parse", (r"\notknown",))
                .unwrap();
            let document = result
                .cast::<PyDict>()
                .unwrap()
                .get_item("document")
                .unwrap()
                .unwrap();
            assert!(
                document
                    .call_method0("has_errors")
                    .unwrap()
                    .extract::<bool>()
                    .unwrap()
            );
        });
    }

    #[test]
    fn python_columnar_export_has_complete_columns_with_optional_and_error_values() {
        run_python_test(
            cr#"
doc = texform.Parser().parse(r"\sqrt{x}+\operatorname{sn}")["document"]
try:
    doc.create_command("bf")
except texform.ConformanceError as error:
    assert error.rule == "command_kind_mismatch"
    assert "declarative constructor" in str(error)
else:
    raise AssertionError("expected constructor guidance")
tables = doc.to_columnar()
assert set(tables) == {"nodes", "args"}
assert len(tables["nodes"]) == 12 and len(tables["args"]) == 9
for table in tables.values():
    assert all(isinstance(column, list) for column in table.values())
    assert len({len(column) for column in table.values()}) == 1
assert tables["nodes"]["parent"][0] == -1
assert tables["nodes"]["slot"][0] is None
assert tables["nodes"]["kind"][0] == "Root"
assert tables["args"]["form"][0] == "optional"
assert tables["args"]["present"][0] is False
assert tables["args"]["content"][0] == -1
assert "operator_name" in tables["args"]["value_kind"]
assert "false" in tables["args"]["value"]
error_doc = texform.Document.from_syntax({"Root": {"mode": "Math", "children": [
    {"Error": {"message": "invalid", "snippet": "?"}}
]}})
assert error_doc.to_columnar()["nodes"]["value"] == [None, "?"]
assert error_doc.to_columnar()["args"]["owner"] == []
"#,
        );
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
