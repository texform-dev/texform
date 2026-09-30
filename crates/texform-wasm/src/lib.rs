use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

use texform::bindings::{ParseConfigInput, TransformConfigInput, transform_report_to_dto};
use texform::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, Arg, ContentMode,
    SyntaxNode,
};
use wasm_bindgen::prelude::*;

mod config;
mod dto;

#[cfg(test)]
use config::{ParserOptions, parser_from_options};
use config::{
    engine_from_js, normalize_config_from_js, parse_config_from_js, parser_from_js,
    serialize_options_from_js, transform_config_from_js,
};
use dto::{
    binding_dto_to_js, binding_error_parts_to_js, binding_error_to_js, config_error_to_js,
    edit_message_to_js, internal_message_to_js, js_set, parse_message_to_js, to_js_value,
};

/// Call a constructor in an explicit context mode, or in its default context.
macro_rules! construct {
    ($doc:expr, $mode:expr, $method:ident($($arg:expr),*)) => {
        match $mode {
            Some(mode) => $doc.in_mode(mode).$method($($arg),*),
            None => $doc.$method($($arg),*),
        }
    };
}

type SharedDocument = Rc<RefCell<texform::Document>>;
type NodeHandleEntry = (SharedDocument, texform::NodeId);

thread_local! {
    static NEXT_NODE_HANDLE: Cell<u32> = const { Cell::new(1) };
    static NODE_HANDLES: RefCell<HashMap<u32, NodeHandleEntry>> = RefCell::new(HashMap::new());
}

#[wasm_bindgen]
pub struct Document {
    inner: Rc<RefCell<texform::Document>>,
}

#[wasm_bindgen]
impl Document {
    #[wasm_bindgen(constructor)]
    pub fn new(kb: Option<KnowledgeBase>, mode: Option<String>) -> Result<Document, JsValue> {
        let kb = kb.map(|kb| kb.inner.clone()).unwrap_or_default();
        Ok(Self::from_core(texform::Document::with_knowledge_base(
            &kb,
            parse_content_mode(mode.as_deref().unwrap_or("math"))?,
        )))
    }
    #[wasm_bindgen(js_name = knowledgeBase)]
    pub fn knowledge_base(&self) -> Result<KnowledgeBase, JsValue> {
        Ok(KnowledgeBase {
            inner: borrow_document(&self.inner)?.knowledge_base().clone(),
        })
    }
    #[wasm_bindgen(js_name = clone)]
    pub fn clone_document(&self) -> Result<Document, JsValue> {
        Ok(Self::from_core(borrow_document(&self.inner)?.clone()))
    }

    #[wasm_bindgen(js_name = fromSyntax)]
    pub fn from_syntax(node: JsValue, kb: Option<KnowledgeBase>) -> Result<Document, JsValue> {
        let node = serde_wasm_bindgen::from_value::<SyntaxNode>(node)
            .map_err(|error| parse_message_to_js(format!("invalid syntax node: {error}")))?;
        texform::Document::from_syntax_with(
            &kb.map(|kb| kb.inner.clone()).unwrap_or_default(),
            &node,
        )
        .map(Self::from_core)
        .map_err(|error| binding_error_to_js(texform::bindings::from_syntax_error_to_dto(error)))
    }

    pub fn root(&self) -> Result<Node, JsValue> {
        let id = {
            let document = borrow_document(&self.inner)?;
            document.root().id()
        };
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = hasErrors)]
    pub fn has_errors(&self) -> Result<bool, JsValue> {
        Ok(borrow_document(&self.inner)?.has_errors())
    }

    #[wasm_bindgen(js_name = isReadOnly)]
    pub fn is_read_only(&self) -> Result<bool, JsValue> {
        Ok(borrow_document(&self.inner)?.is_read_only())
    }

    pub fn errors(&self) -> Result<js_sys::Array, JsValue> {
        let ids = borrow_document(&self.inner)?
            .errors()
            .map(|node| node.id())
            .collect::<Vec<_>>();
        Ok(nodes_to_js_array(&self.inner, ids))
    }

    #[wasm_bindgen(js_name = findCommands)]
    pub fn find_commands(&self, name: &str) -> Result<js_sys::Array, JsValue> {
        let ids = borrow_document(&self.inner)?
            .find_commands(name)
            .map(|node| node.id())
            .collect::<Vec<_>>();
        Ok(nodes_to_js_array(&self.inner, ids))
    }

    #[wasm_bindgen(js_name = findEnvironments)]
    pub fn find_environments(&self, name: &str) -> Result<js_sys::Array, JsValue> {
        let ids = borrow_document(&self.inner)?
            .find_environments(name)
            .map(|node| node.id())
            .collect::<Vec<_>>();
        Ok(nodes_to_js_array(&self.inner, ids))
    }

    #[wasm_bindgen(js_name = toSyntax)]
    pub fn to_syntax(&self) -> Result<JsValue, JsValue> {
        let syntax = borrow_document(&self.inner)?.to_syntax();
        serde_wasm_bindgen::to_value(&syntax)
            .map_err(|error| internal_message_to_js(error.to_string()))
    }

    #[wasm_bindgen(js_name = nodeSpans)]
    pub fn node_spans(&self) -> Result<JsValue, JsValue> {
        let entries = texform::bindings::node_spans_to_dto(&*borrow_document(&self.inner)?);
        binding_dto_to_js(&entries)
    }

    #[wasm_bindgen(js_name = toLatex)]
    pub fn to_latex(&self, options: Option<JsValue>) -> Result<String, JsValue> {
        let options = serialize_options_from_js(options)?;
        borrow_document(&self.inner)?
            .to_latex_with(&options)
            .map_err(|error| internal_message_to_js(error.to_string()))
    }

    #[wasm_bindgen(js_name = toTokenizedLatex)]
    pub fn to_tokenized_latex(&self, options: Option<JsValue>) -> Result<JsValue, JsValue> {
        let options = serialize_options_from_js(options)?;
        let result = borrow_document(&self.inner)?
            .to_tokenized_latex_with(&options)
            .map_err(|error| internal_message_to_js(error.to_string()))?;
        binding_dto_to_js(&texform::bindings::tokenized_latex_to_dto(result))
    }

    #[wasm_bindgen(js_name = createChar)]
    pub fn create_char(&self, value: &str, mode: Option<String>) -> Result<Node, JsValue> {
        let ch = texform::bindings::parse_char(value).map_err(edit_error_to_js)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_char(ch)))
    }
    #[wasm_bindgen(js_name = createText)]
    pub fn create_text(&self, value: &str, mode: Option<String>) -> Result<Node, JsValue> {
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_text(value)))
    }
    #[wasm_bindgen(js_name = createActiveSpace)]
    pub fn create_active_space(&self, mode: Option<String>) -> Result<Node, JsValue> {
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_active_space()))
    }
    #[wasm_bindgen(js_name = createAlignmentTab)]
    pub fn create_alignment_tab(&self, mode: Option<String>) -> Result<Node, JsValue> {
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_alignment_tab()))
    }
    #[wasm_bindgen(js_name = createPrime)]
    pub fn create_prime(&self, count: usize, mode: Option<String>) -> Result<Node, JsValue> {
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_prime(count)))
    }
    #[wasm_bindgen(js_name = createGroup)]
    pub fn create_group(
        &self,
        group_mode: &str,
        children: Option<JsValue>,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let group_mode = parse_content_mode(group_mode)?;
        let children = js_args(&self.inner, children)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_group(group_mode, children)))
    }
    #[wasm_bindgen(js_name = createDelimitedGroup)]
    pub fn create_delimited_group(
        &self,
        left: &str,
        right: &str,
        children: Option<JsValue>,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let left = left.parse().map_err(edit_error_to_js)?;
        let right = right.parse().map_err(edit_error_to_js)?;
        let children = js_args(&self.inner, children)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_delimited_group(left, right, children)))
    }
    #[wasm_bindgen(js_name = createInlineMath)]
    pub fn create_inline_math(
        &self,
        children: Option<JsValue>,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let children = js_args(&self.inner, children)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_inline_math(children)))
    }
    #[wasm_bindgen(js_name = createCommand)]
    pub fn create_command(
        &self,
        name: &str,
        args: Option<JsValue>,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let args = js_args(&self.inner, args)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_command(name, args)))
    }
    #[wasm_bindgen(js_name = createDeclarative)]
    pub fn create_declarative(
        &self,
        name: &str,
        args: Option<JsValue>,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let args = js_args(&self.inner, args)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_declarative(name, args)))
    }
    #[wasm_bindgen(js_name = createScripted)]
    pub fn create_scripted(
        &self,
        base: JsValue,
        sub: JsValue,
        sup: JsValue,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let base = js_arg(&self.inner, base)?;
        let sub = js_optional_arg(&self.inner, sub)?;
        let sup = js_optional_arg(&self.inner, sup)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_scripted(base, sub, sup)))
    }
    #[wasm_bindgen(js_name = createInfix)]
    pub fn create_infix(
        &self,
        name: &str,
        left: JsValue,
        right: JsValue,
        args: Option<JsValue>,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let left = js_arg(&self.inner, left)?;
        let right = js_arg(&self.inner, right)?;
        let args = js_args(&self.inner, args)?;
        let mode = parse_optional_mode(mode)?;
        self.construct(|doc| construct!(doc, mode, create_infix(name, left, right, args)))
    }
    /// An array body becomes the children of the implicit body group.
    #[wasm_bindgen(js_name = createEnvironment)]
    pub fn create_environment(
        &self,
        name: &str,
        args: Option<JsValue>,
        body: JsValue,
        mode: Option<String>,
    ) -> Result<Node, JsValue> {
        let args = js_args(&self.inner, args)?;
        let mode = parse_optional_mode(mode)?;
        if js_sys::Array::is_array(&body) {
            let children = js_args(&self.inner, Some(body))?;
            self.construct(|doc| {
                construct!(
                    doc,
                    mode,
                    create_environment_with_children(name, args, children)
                )
            })
        } else {
            let body = js_arg(&self.inner, body)?;
            self.construct(|doc| construct!(doc, mode, create_environment(name, args, body)))
        }
    }

    #[wasm_bindgen(js_name = parseFragment)]
    pub fn parse_fragment(&self, source: &str, mode: Option<String>) -> Result<Node, JsValue> {
        let mode = mode.as_deref().map(parse_content_mode).transpose()?;
        let id = borrow_document_mut(&self.inner)?
            .parse_fragment(source, mode)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = setEnvName)]
    pub fn set_env_name(&self, node: &Node, name: &str) -> Result<(), JsValue> {
        self.ensure_same_document(node)?;
        borrow_document_mut(&self.inner)?
            .set_env_name(node.id, name)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = appendChild)]
    pub fn append_child(&self, parent: &Node, child: &Node) -> Result<(), JsValue> {
        self.ensure_same_document(parent)?;
        parent.ensure_same_document(child)?;
        borrow_document_mut(&self.inner)?
            .append_child(parent.id, child.id)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = insertBefore)]
    pub fn insert_before(&self, anchor: &Node, new: &Node) -> Result<(), JsValue> {
        self.ensure_same_document(anchor)?;
        anchor.ensure_same_document(new)?;
        borrow_document_mut(&self.inner)?
            .insert_before(anchor.id, new.id)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = insertAfter)]
    pub fn insert_after(&self, anchor: &Node, new: &Node) -> Result<(), JsValue> {
        self.ensure_same_document(anchor)?;
        anchor.ensure_same_document(new)?;
        borrow_document_mut(&self.inner)?
            .insert_after(anchor.id, new.id)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = insertChild)]
    pub fn insert_child(&self, parent: &Node, index: usize, child: &Node) -> Result<(), JsValue> {
        self.ensure_same_document(parent)?;
        parent.ensure_same_document(child)?;
        borrow_document_mut(&self.inner)?
            .insert_child(parent.id, index, child.id)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = replaceWith)]
    pub fn replace_with(&self, target: &Node, replacement: &Node) -> Result<(), JsValue> {
        self.ensure_same_document(target)?;
        target.ensure_same_document(replacement)?;
        borrow_document_mut(&self.inner)?
            .replace_with(target.id, replacement.id)
            .map_err(edit_error_to_js)
    }

    pub fn wrap(&self, target: &Node, wrapper: &Node) -> Result<Node, JsValue> {
        self.ensure_same_document(target)?;
        target.ensure_same_document(wrapper)?;
        let id = borrow_document_mut(&self.inner)?
            .wrap(target.id, wrapper.id)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    pub fn unwrap(&self, group: &Node) -> Result<js_sys::Array, JsValue> {
        self.ensure_same_document(group)?;
        let ids = borrow_document_mut(&self.inner)?
            .unwrap(group.id)
            .map_err(edit_error_to_js)?;
        Ok(nodes_to_js_array(&self.inner, ids))
    }

    pub fn extract(&self, node: &Node) -> Result<Node, JsValue> {
        self.ensure_same_document(node)?;
        let id = borrow_document_mut(&self.inner)?
            .extract(node.id)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    pub fn remove(&self, node: &Node) -> Result<(), JsValue> {
        self.ensure_same_document(node)?;
        borrow_document_mut(&self.inner)?
            .remove(node.id)
            .map_err(edit_error_to_js)
    }

    pub fn clear(&self, container: &Node) -> Result<(), JsValue> {
        self.ensure_same_document(container)?;
        borrow_document_mut(&self.inner)?
            .clear(container.id)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = setCommandName)]
    pub fn set_command_name(&self, node: &Node, name: &str) -> Result<(), JsValue> {
        self.ensure_same_document(node)?;
        borrow_document_mut(&self.inner)?
            .set_command_name(node.id, name)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = setText)]
    pub fn set_text(&self, node: &Node, value: &str) -> Result<(), JsValue> {
        self.ensure_same_document(node)?;
        borrow_document_mut(&self.inner)?
            .set_text(node.id, value)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = setChar)]
    pub fn set_char(&self, node: &Node, value: &str) -> Result<(), JsValue> {
        self.ensure_same_document(node)?;
        let ch = texform::bindings::parse_char(value).map_err(edit_error_to_js)?;
        borrow_document_mut(&self.inner)?
            .set_char(node.id, ch)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = setArg)]
    pub fn set_arg(&self, node: &Node, index: usize, value: JsValue) -> Result<(), JsValue> {
        self.ensure_same_document(node)?;
        let value = js_arg(&self.inner, value)?;
        borrow_document_mut(&self.inner)?
            .set_arg(node.id, index, value)
            .map_err(edit_error_to_js)
    }
}

impl Document {
    pub(crate) fn from_core(document: texform::Document) -> Self {
        Self {
            inner: Rc::new(RefCell::new(document)),
        }
    }

    fn construct(
        &self,
        build: impl FnOnce(&mut texform::Document) -> Result<texform::NodeId, texform::EditError>,
    ) -> Result<Node, JsValue> {
        let id = build(&mut *borrow_document_mut(&self.inner)?).map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    fn ensure_same_document(&self, node: &Node) -> Result<(), JsValue> {
        if Rc::ptr_eq(&self.inner, &node.document) {
            Ok(())
        } else {
            Err(edit_message_to_js("node belongs to a different document"))
        }
    }

    #[cfg(test)]
    fn create_command_with_args(&self, name: &str, args: Vec<Arg>) -> Result<Node, JsValue> {
        let id = borrow_document_mut(&self.inner)?
            .create_command(name, args)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::from_core(texform::Document::new())
    }
}

#[wasm_bindgen]
pub struct Node {
    document: Rc<RefCell<texform::Document>>,
    id: texform::NodeId,
    handle: u32,
}

#[wasm_bindgen]
impl Node {
    #[wasm_bindgen(js_name = isSameNode)]
    pub fn is_same_node(&self, other: &Node) -> bool {
        Rc::ptr_eq(&self.document, &other.document) && self.id == other.id
    }
    pub fn document(&self) -> Document {
        Document {
            inner: Rc::clone(&self.document),
        }
    }

    #[wasm_bindgen(getter, js_name = __texformBindingHandle)]
    pub fn binding_handle(&self) -> u32 {
        self.handle
    }

    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> Result<String, JsValue> {
        self.with_ref(|node| node_kind_to_string(node.kind()).to_string())
    }

    #[wasm_bindgen(js_name = isCommand)]
    pub fn is_command(&self, name: Option<String>) -> Result<bool, JsValue> {
        self.with_ref(|node| {
            if let Some(name) = name.as_deref() {
                node.is_command(name)
            } else {
                node.kind() == texform::NodeKind::Command
            }
        })
    }

    #[wasm_bindgen(js_name = isChar)]
    pub fn is_char(&self, value: Option<String>) -> Result<bool, JsValue> {
        let ch = value
            .as_deref()
            .map(texform::bindings::parse_char)
            .transpose()
            .map_err(edit_error_to_js)?;
        self.with_ref(|node| match ch {
            Some(ch) => node.is_char(ch),
            None => node.kind() == texform::NodeKind::Char,
        })
    }

    #[wasm_bindgen(js_name = isError)]
    pub fn is_error(&self) -> Result<bool, JsValue> {
        self.with_ref(|node| node.is_error())
    }

    pub fn parent(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.parent().map(|parent| parent.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    #[wasm_bindgen(getter, js_name = commandName)]
    pub fn command_name(&self) -> Result<JsValue, JsValue> {
        self.with_ref(|node| optional_string_to_js(node.command_name()))
    }

    #[wasm_bindgen(getter, js_name = envName)]
    pub fn env_name(&self) -> Result<JsValue, JsValue> {
        self.with_ref(|node| optional_string_to_js(node.env_name()))
    }

    #[wasm_bindgen(getter)]
    pub fn text(&self) -> Result<JsValue, JsValue> {
        self.with_ref(|node| optional_string_to_js(node.text()))
    }

    #[wasm_bindgen(getter, js_name = char)]
    pub fn char_value(&self) -> Result<JsValue, JsValue> {
        self.with_ref(|node| {
            node.char()
                .map(|ch| JsValue::from(ch.to_string()))
                .unwrap_or(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = primeCount)]
    pub fn prime_count(&self) -> Result<JsValue, JsValue> {
        self.with_ref(|node| {
            node.prime_count()
                .map(|count| JsValue::from_f64(count as f64))
                .unwrap_or(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = errorParts)]
    pub fn error_parts(&self) -> Result<JsValue, JsValue> {
        let parts = self.with_ref(|node| {
            node.error_parts()
                .map(|(message, snippet)| (message.to_string(), snippet.to_string()))
        })?;
        Ok(match parts {
            Some((message, snippet)) => {
                let value = js_sys::Object::new();
                js_set(value.as_ref(), "message", &message.into())?;
                js_set(value.as_ref(), "snippet", &snippet.into())?;
                value.into()
            }
            None => JsValue::NULL,
        })
    }

    #[wasm_bindgen(js_name = contentMode)]
    pub fn content_mode(&self) -> Result<JsValue, JsValue> {
        self.with_ref(|node| {
            node.content_mode()
                .map(ContentMode::as_str)
                .map(JsValue::from)
                .unwrap_or(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = groupKind)]
    pub fn group_kind(&self) -> Result<JsValue, JsValue> {
        let kind =
            self.with_ref(|node| node.group_kind().map(texform::bindings::GroupKindDto::from))?;
        binding_dto_to_js(&kind)
    }

    #[wasm_bindgen(js_name = argCount)]
    pub fn arg_count(&self) -> Result<usize, JsValue> {
        self.with_ref(|node| node.arg_count())
    }

    pub fn arg(&self, index: usize) -> Result<JsValue, JsValue> {
        let arg = self.with_ref(|node| texform::bindings::arg_ref_to_dto(node, index))?;
        self.arg_to_js(arg)
    }

    #[wasm_bindgen(js_name = argSlots)]
    pub fn arg_slots(&self) -> Result<js_sys::Array, JsValue> {
        let args = self.with_ref(|node| {
            (0..node.arg_count())
                .map(|index| texform::bindings::arg_ref_to_dto(node, index))
                .collect::<Vec<_>>()
        })?;
        args.into_iter().map(|arg| self.arg_to_js(arg)).collect()
    }

    #[wasm_bindgen(js_name = scriptBase)]
    pub fn script_base(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.script_base().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    pub fn subscript(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.subscript().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    pub fn superscript(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.superscript().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    #[wasm_bindgen(js_name = infixLeft)]
    pub fn infix_left(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.infix_left().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    #[wasm_bindgen(js_name = infixRight)]
    pub fn infix_right(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.infix_right().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    #[wasm_bindgen(js_name = envBody)]
    pub fn env_body(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.env_body().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    pub fn span(&self) -> Result<JsValue, JsValue> {
        let span = self.with_ref(|node| node.span())?;
        match span {
            Some(span) => serde_wasm_bindgen::to_value(&span)
                .map_err(|error| internal_message_to_js(error.to_string())),
            None => Ok(JsValue::NULL),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn children(&self) -> Result<js_sys::Array, JsValue> {
        let ids =
            self.with_ref(|node| node.children().map(|child| child.id()).collect::<Vec<_>>())?;
        Ok(nodes_to_js_array(&self.document, ids))
    }

    #[wasm_bindgen(js_name = nextSibling)]
    pub fn next_sibling(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.next_sibling().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    #[wasm_bindgen(js_name = prevSibling)]
    pub fn prev_sibling(&self) -> Result<JsValue, JsValue> {
        let id = self.with_ref(|node| node.prev_sibling().map(|child| child.id()))?;
        Ok(optional_node_to_js(&self.document, id))
    }

    pub fn ancestors(&self) -> Result<js_sys::Array, JsValue> {
        let ids =
            self.with_ref(|node| node.ancestors().map(|node| node.id()).collect::<Vec<_>>())?;
        Ok(nodes_to_js_array(&self.document, ids))
    }

    pub fn descendants(&self) -> Result<js_sys::Array, JsValue> {
        let ids =
            self.with_ref(|node| node.descendants().map(|node| node.id()).collect::<Vec<_>>())?;
        Ok(nodes_to_js_array(&self.document, ids))
    }
}

impl Node {
    fn from_parts(document: Rc<RefCell<texform::Document>>, id: texform::NodeId) -> Self {
        let handle = register_node_handle(&document, id);
        Self {
            document,
            id,
            handle,
        }
    }

    fn ensure_same_document(&self, other: &Node) -> Result<(), JsValue> {
        if Rc::ptr_eq(&self.document, &other.document) {
            Ok(())
        } else {
            Err(edit_message_to_js("node belongs to a different document"))
        }
    }

    fn with_ref<T>(&self, f: impl FnOnce(texform::NodeRef<'_>) -> T) -> Result<T, JsValue> {
        let document = borrow_document(&self.document)?;
        let node = document.node(self.id).map_err(edit_error_to_js)?;
        Ok(f(node))
    }

    /// Expose an argument, attaching a live handle for content arguments.
    fn arg_to_js(&self, arg: Option<texform::bindings::ArgRefDto>) -> Result<JsValue, JsValue> {
        let Some(arg) = arg else {
            return Ok(JsValue::NULL);
        };
        let value = binding_dto_to_js(&arg)?;
        if let Some(id) = arg.node {
            js_set(
                &value,
                "node",
                &Node::from_parts(Rc::clone(&self.document), id).into(),
            )?;
        }
        Ok(value)
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        NODE_HANDLES.with(|handles| {
            handles.borrow_mut().remove(&self.handle);
        });
    }
}

fn borrow_document(
    document: &Rc<RefCell<texform::Document>>,
) -> Result<Ref<'_, texform::Document>, JsValue> {
    document
        .try_borrow()
        .map_err(|_| edit_message_to_js("document is already mutably borrowed"))
}

fn borrow_document_mut(
    document: &Rc<RefCell<texform::Document>>,
) -> Result<RefMut<'_, texform::Document>, JsValue> {
    document
        .try_borrow_mut()
        .map_err(|_| edit_message_to_js("document is already borrowed"))
}

fn edit_error_to_js(error: texform::EditError) -> JsValue {
    binding_error_to_js(texform::bindings::edit_error_to_dto(error))
}

fn nodes_to_js_array(
    document: &Rc<RefCell<texform::Document>>,
    ids: Vec<texform::NodeId>,
) -> js_sys::Array {
    let out = js_sys::Array::new();
    for id in ids {
        out.push(&Node::from_parts(Rc::clone(document), id).into());
    }
    out
}

fn optional_node_to_js(
    document: &Rc<RefCell<texform::Document>>,
    id: Option<texform::NodeId>,
) -> JsValue {
    match id {
        Some(id) => Node::from_parts(Rc::clone(document), id).into(),
        None => JsValue::NULL,
    }
}

fn optional_string_to_js(value: Option<&str>) -> JsValue {
    value.map(JsValue::from).unwrap_or(JsValue::NULL)
}

fn register_node_handle(document: &Rc<RefCell<texform::Document>>, id: texform::NodeId) -> u32 {
    let handle = NEXT_NODE_HANDLE.with(|next| {
        let handle = next.get();
        next.set(handle.wrapping_add(1).max(1));
        handle
    });
    NODE_HANDLES.with(|handles| {
        handles
            .borrow_mut()
            .insert(handle, (Rc::clone(document), id));
    });
    handle
}

fn node_kind_to_string(kind: texform::NodeKind) -> &'static str {
    match kind {
        texform::NodeKind::Root => "root",
        texform::NodeKind::Group => "group",
        texform::NodeKind::Command => "command",
        texform::NodeKind::Infix => "infix",
        texform::NodeKind::Declarative => "declarative",
        texform::NodeKind::Environment => "environment",
        texform::NodeKind::Scripted => "scripted",
        texform::NodeKind::Prime => "prime",
        texform::NodeKind::Text => "text",
        texform::NodeKind::Char => "char",
        texform::NodeKind::ActiveSpace => "activeSpace",
        texform::NodeKind::AlignmentTab => "alignmentTab",
        texform::NodeKind::Error => "error",
    }
}

fn js_args(document: &SharedDocument, value: Option<JsValue>) -> Result<Vec<Arg>, JsValue> {
    match value {
        Some(value) if js_sys::Array::is_array(&value) => js_sys::Array::from(&value)
            .iter()
            .map(|arg| js_arg(document, arg))
            .collect(),
        Some(value) if !value.is_null() && !value.is_undefined() => {
            Err(edit_message_to_js("arguments must be an array"))
        }
        _ => Ok(Vec::new()),
    }
}

/// Read an optional script: `null` and `undefined` leave the slot empty.
fn js_optional_arg(document: &SharedDocument, value: JsValue) -> Result<Option<Arg>, JsValue> {
    if value.is_null() || value.is_undefined() {
        Ok(None)
    } else {
        js_arg(document, value).map(Some)
    }
}

fn js_arg(document: &SharedDocument, value: JsValue) -> Result<Arg, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(Arg::Absent);
    }
    if let Some(source) = value.as_string() {
        return Ok(Arg::Source(source));
    }
    if let Some(star) = value.as_bool() {
        return Ok(Arg::Star(star));
    }
    if !value.is_object() {
        return Err(edit_message_to_js(
            "argument must be a Node, source string, boolean, null, or paired value",
        ));
    }
    let property = |key: &str| {
        js_sys::Reflect::get(&value, &key.into())
            .map_err(|_| edit_message_to_js(format!("{key} is not readable")))
    };
    if let Some(handle) = property("__texformBindingHandle")?.as_f64() {
        return node_id(document, handle as u32).map(Arg::Node);
    }
    let (Some(open), Some(close)) = (
        property("open")?.as_string(),
        property("close")?.as_string(),
    ) else {
        return Err(edit_message_to_js("paired open and close must be strings"));
    };
    Arg::paired(js_arg(document, property("value")?)?, open, close).map_err(edit_error_to_js)
}

fn node_id(document: &SharedDocument, handle: u32) -> Result<texform::NodeId, JsValue> {
    NODE_HANDLES.with(|handles| {
        let handles = handles.borrow();
        let (owner, id) = handles
            .get(&handle)
            .ok_or_else(|| edit_message_to_js("argument node must be a live Node"))?;
        if Rc::ptr_eq(document, owner) {
            Ok(*id)
        } else {
            Err(edit_message_to_js("node belongs to a different document"))
        }
    })
}

#[wasm_bindgen]
pub struct KnowledgeBase {
    inner: texform::KnowledgeBase,
}
#[wasm_bindgen]
impl KnowledgeBase {
    #[wasm_bindgen(constructor)]
    pub fn new(options: Option<JsValue>) -> Result<KnowledgeBase, JsValue> {
        let input: texform::bindings::KnowledgeBaseInput = match options {
            Some(value) if !value.is_null() && !value.is_undefined() => {
                config::from_js(value, "knowledge base options")?
            }
            _ => Default::default(),
        };
        Ok(Self {
            inner: input.build().map_err(config_error_to_js)?,
        })
    }
    #[wasm_bindgen(js_name = isSame)]
    pub fn is_same(&self, other: &KnowledgeBase) -> bool {
        self.inner.ptr_eq(&other.inner)
    }
    #[wasm_bindgen(js_name = clone)]
    pub fn clone_handle(&self) -> KnowledgeBase {
        Self {
            inner: self.inner.clone(),
        }
    }
    pub fn packages(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.packages())
    }
    pub fn characters(&self, mode: &str) -> Result<JsValue, JsValue> {
        binding_dto_to_js(
            &self
                .inner
                .characters(parse_content_mode(mode)?)
                .into_iter()
                .map(texform::bindings::character_info_to_dto)
                .collect::<Vec<_>>(),
        )
    }
    pub fn environments(&self, mode: &str) -> Result<JsValue, JsValue> {
        binding_dto_to_js(
            &self
                .inner
                .environments(parse_content_mode(mode)?)
                .into_iter()
                .map(texform::bindings::env_info_to_dto)
                .collect::<Vec<_>>(),
        )
    }
    pub fn commands(&self, mode: &str) -> Result<JsValue, JsValue> {
        binding_dto_to_js(
            &self
                .inner
                .commands(parse_content_mode(mode)?)
                .into_iter()
                .map(texform::bindings::command_info_to_dto)
                .collect::<Vec<_>>(),
        )
    }
    pub fn delimiters(&self) -> Result<JsValue, JsValue> {
        binding_dto_to_js(
            &self
                .inner
                .delimiters()
                .into_iter()
                .map(texform::bindings::delimiter_info_to_dto)
                .collect::<Vec<_>>(),
        )
    }
    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    pub fn lookup_command(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        match self.lookup_command_meta(name, mode)? {
            Some(meta) => command_meta_to_js(meta),
            None => Ok(JsValue::NULL),
        }
    }

    pub fn lookup_explicit_command(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        match self.lookup_explicit_command_meta(name, mode)? {
            Some(meta) => command_meta_to_js(meta),
            None => Ok(JsValue::NULL),
        }
    }

    pub fn lookup_character(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        match self.lookup_character_meta(name, mode)? {
            Some(meta) => character_meta_to_js(meta),
            None => Ok(JsValue::NULL),
        }
    }

    pub fn lookup_env(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        match self.lookup_env_meta(name, mode)? {
            Some(meta) => env_meta_to_js(meta),
            None => Ok(JsValue::NULL),
        }
    }

    pub fn knows_command_name(&self, name: &str) -> bool {
        self.inner.knows_command_name(name)
    }

    pub fn knows_env_name(&self, name: &str) -> bool {
        self.inner.knows_env_name(name)
    }

    pub fn knows_character_name(&self, name: &str) -> bool {
        self.inner.knows_character_name(name)
    }
}

#[wasm_bindgen]
pub struct Parser {
    inner: texform::Parser,
}

#[wasm_bindgen]
impl Parser {
    #[wasm_bindgen(js_name = knowledgeBase)]
    pub fn knowledge_base(&self) -> KnowledgeBase {
        KnowledgeBase {
            inner: self.inner.knowledge_base().clone(),
        }
    }

    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>, kb: Option<KnowledgeBase>) -> Result<Parser, JsValue> {
        Ok(Parser {
            inner: parser_from_js(args, kb.as_ref())?,
        })
    }

    pub fn parse(&self, src: &str, config: Option<JsValue>) -> Result<JsValue, JsValue> {
        let base = self.inner.default_parse_config().clone();
        let config = parse_config_from_js(config, base)?;
        parse_result_to_js(self.inner.parse_with(src, &config))
    }

    #[wasm_bindgen(js_name = defaultParseConfig)]
    pub fn default_parse_config(&self) -> Result<JsValue, JsValue> {
        binding_dto_to_js(&ParseConfigInput::from_config(
            self.inner.default_parse_config().clone(),
        ))
    }
}

#[wasm_bindgen]
pub struct TransformEngine {
    inner: texform::TransformEngine,
}

#[wasm_bindgen]
impl TransformEngine {
    #[wasm_bindgen(js_name = knowledgeBase)]
    pub fn knowledge_base(&self) -> KnowledgeBase {
        KnowledgeBase {
            inner: self.inner.knowledge_base().clone(),
        }
    }

    #[wasm_bindgen(constructor)]
    pub fn new(
        args: Option<JsValue>,
        kb: Option<KnowledgeBase>,
    ) -> Result<TransformEngine, JsValue> {
        Ok(Self {
            inner: engine_from_js(args, kb.as_ref())?,
        })
    }

    pub fn parse(&self, src: &str, config: Option<JsValue>) -> Result<JsValue, JsValue> {
        let base = self.inner.parser().default_parse_config().clone();
        let config = parse_config_from_js(config, base)?;
        parse_result_to_js(self.inner.parser().parse_with(src, &config))
    }

    #[wasm_bindgen(js_name = defaultParseConfig)]
    pub fn default_parse_config(&self) -> Result<JsValue, JsValue> {
        binding_dto_to_js(&ParseConfigInput::from_config(
            self.inner.parser().default_parse_config().clone(),
        ))
    }

    #[wasm_bindgen(js_name = defaultTransformConfig)]
    pub fn default_transform_config(&self) -> Result<JsValue, JsValue> {
        binding_dto_to_js(&TransformConfigInput::from_config(
            *self.inner.default_transform_config(),
        ))
    }

    pub fn normalize(&self, src: &str, options: Option<JsValue>) -> Result<String, JsValue> {
        // Plain path returns text only. It shares config parsing with
        // `normalizeWithReport` and does not build a report DTO.
        let config = normalize_config_from_js(options, self.inner.default_normalize_config())?;
        self.inner.normalize_with(src, &config).map_err(|error| {
            binding_error_parts_to_js(texform::bindings::normalize_error_to_parts(error))
        })
    }

    #[wasm_bindgen(js_name = normalizeWithReport)]
    pub fn normalize_with_report(
        &self,
        src: &str,
        options: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let config = normalize_config_from_js(options, self.inner.default_normalize_config())?;
        let result = self
            .inner
            .normalize_with_report(src, &config)
            .map_err(|error| {
                binding_error_parts_to_js(texform::bindings::normalize_error_to_parts(error))
            })?;
        normalize_report_result_to_js(result.normalized, &result.report)
    }

    pub fn transform(&self, document: &Document, config: Option<JsValue>) -> Result<(), JsValue> {
        // Plain path returns undefined. It shares config parsing with
        // `transformWithReport` and does not build a report DTO.
        let config = transform_config_from_js(config, *self.inner.default_transform_config())?;
        let mut document = borrow_document_mut(&document.inner)?;
        self.inner
            .transform_with(&mut document, &config)
            .map_err(|error| {
                binding_error_parts_to_js(texform::bindings::normalize_error_to_parts(error))
            })
    }

    #[wasm_bindgen(js_name = transformWithReport)]
    pub fn transform_with_report(
        &self,
        document: &Document,
        config: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let config = transform_config_from_js(config, *self.inner.default_transform_config())?;
        let mut document = borrow_document_mut(&document.inner)?;
        let report = self
            .inner
            .transform_with_report(&mut document, &config)
            .map_err(|error| {
                binding_error_parts_to_js(texform::bindings::normalize_error_to_parts(error))
            })?;
        transform_report_to_js(&report)
    }
}

fn normalize_report_result_to_js(
    normalized: String,
    report: &texform::diagnostics::TransformReport,
) -> Result<JsValue, JsValue> {
    let value = js_sys::Object::new();
    js_set(value.as_ref(), "normalized", &normalized.into())?;
    js_set(value.as_ref(), "report", &transform_report_to_js(report)?)?;
    Ok(value.into())
}

#[wasm_bindgen]
pub fn serialize(node: JsValue, options: Option<JsValue>) -> Result<String, JsValue> {
    let node = serde_wasm_bindgen::from_value::<SyntaxNode>(node)
        .map_err(|error| parse_message_to_js(format!("invalid syntax node: {error}")))?;
    let options = serialize_options_from_js(options)?;
    texform::bindings::serialize_syntax(&node, &options).map_err(binding_error_to_js)
}

impl Parser {
    #[cfg(test)]
    fn from_options(input: ParserOptions) -> Result<Parser, JsValue> {
        Ok(Parser {
            inner: parser_from_options(input, None),
        })
    }
}

impl KnowledgeBase {
    fn lookup_command_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCommandRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_command(name, mode))
    }

    fn lookup_explicit_command_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCommandRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_explicit_command(name, mode))
    }

    fn lookup_character_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCharacterRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_character(name, mode))
    }

    fn lookup_env_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveEnvironmentRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_env(name, mode))
    }
}

#[wasm_bindgen]
pub fn validate_argspec(spec: &str) -> Result<JsValue, JsValue> {
    binding_dto_to_js(&texform::validate_argspec(spec))
}

#[wasm_bindgen(js_name = listPackages)]
pub fn list_packages() -> Result<JsValue, JsValue> {
    binding_dto_to_js(&texform::bindings::list_packages_to_dto())
}

fn transform_report_to_js(
    report: &texform::diagnostics::TransformReport,
) -> Result<JsValue, JsValue> {
    binding_dto_to_js(&transform_report_to_dto(report))
}

fn parse_result_parts(
    result: texform::ParseResult,
) -> (Option<texform::Document>, Vec<texform::ParseDiagnostic>) {
    result.into_parts()
}

fn parse_result_to_js(result: texform::ParseResult) -> Result<JsValue, JsValue> {
    let (document, diagnostics) = parse_result_parts(result);
    let value = js_sys::Object::new();
    let document = match document {
        Some(document) => Document::from_core(document).into(),
        None => JsValue::NULL,
    };
    let diagnostics = to_js_value(&diagnostics)?;
    js_set(value.as_ref(), "document", &document)?;
    js_set(value.as_ref(), "diagnostics", &diagnostics)?;
    Ok(value.into())
}

fn parse_content_mode(value: &str) -> Result<ContentMode, JsValue> {
    match value {
        "math" => Ok(ContentMode::Math),
        "text" => Ok(ContentMode::Text),
        _ => Err(config_error_to_js(format!(
            "unsupported content mode: {value}"
        ))),
    }
}

fn parse_optional_mode(mode: Option<String>) -> Result<Option<ContentMode>, JsValue> {
    mode.as_deref().map(parse_content_mode).transpose()
}

fn command_meta_to_js(meta: &ActiveCommandRecord) -> Result<JsValue, JsValue> {
    binding_dto_to_js(&texform::bindings::command_info_to_dto(meta))
}

fn env_meta_to_js(meta: &ActiveEnvironmentRecord) -> Result<JsValue, JsValue> {
    binding_dto_to_js(&texform::bindings::env_info_to_dto(meta))
}

fn character_meta_to_js(meta: &ActiveCharacterRecord) -> Result<JsValue, JsValue> {
    binding_dto_to_js(&texform::bindings::character_info_to_dto(meta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use texform::{AllowedMode, ArgRef, CommandItem, CommandKind, ParseConfig};

    #[test]
    fn package_load_build_errors_use_facade_error_text() {
        let error = texform::KnowledgeBase::builder()
            .packages(&["missing"])
            .build()
            .expect_err("missing package should fail");

        assert_eq!(error.to_string(), "unknown package: missing");
    }

    #[test]
    fn invalid_context_item_build_errors_include_item_name() {
        let error = texform::KnowledgeBase::builder()
            .packages(&[])
            .item(CommandItem::new(
                "foo",
                CommandKind::Prefix,
                AllowedMode::Math,
                "s:T",
            ))
            .build()
            .expect_err("invalid item should fail");
        let error = error.to_string();

        assert!(error.contains("foo"));
        assert!(error.contains("invalid argspec"));
    }

    #[test]
    fn knowledge_base_packages_are_explicit() {
        let empty = texform::KnowledgeBase::builder()
            .packages(&[])
            .build()
            .unwrap();
        assert!(empty.lookup_command("frac", ContentMode::Math).is_none());
        assert!(
            texform::KnowledgeBase::default()
                .lookup_command("frac", ContentMode::Math)
                .is_some()
        );
    }

    #[test]
    fn parser_none_config_uses_facade_default() {
        let parser =
            Parser::from_options(ParserOptions::default()).expect("default parser should build");

        let output = parser.inner.parse(r"\unknowncmd");
        assert!(output.document().is_some());
        assert!(output.diagnostics().is_empty());
    }

    #[test]
    fn parse_config_input_uses_supplied_base_config() {
        let config = ParseConfigInput::default().into_config(ParseConfig::STRICT);

        assert!(config.reject_unknown);
        assert!(config.abort_on_error);
    }

    #[test]
    fn transform_config_input_accepts_finalize_ast() {
        let input = TransformConfigInput {
            finalize_ast: Some(texform::bindings::FinalizeAstConfigInput {
                enabled: Some(false),
            }),
            ..Default::default()
        };
        let config = input.into_config(texform::Profile::Authoring.default_transform_config());

        assert!(!config.finalize_ast.enabled);
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn wasm_engine_transform_updates_own_document_in_place() {
        let engine = TransformEngine::new(
            Some(
                serde_wasm_bindgen::to_value(&serde_json::json!({
                    "profile": "equiv",
                }))
                .expect("options should serialize"),
            ),
            None,
        )
        .expect("engine should build");
        let document = Document::from_core(
            engine
                .inner
                .parser()
                .parse("{{x}}")
                .try_into_document()
                .expect("parse should succeed")
                .0,
        );

        let config = Some(
            serde_wasm_bindgen::to_value(&serde_json::json!({
                "rewrite": { "enabled": false },
                "lowerAttributes": { "enabled": false },
                "flattenGroups": { "enabled": true },
            }))
            .expect("config should serialize"),
        );
        engine
            .transform(&document, config.clone())
            .expect("transform should succeed");

        assert_eq!(document.to_latex(None).unwrap(), "x");

        let reported = Document::from_core(
            engine
                .inner
                .parser()
                .parse("{{x}}")
                .try_into_document()
                .expect("parse should succeed")
                .0,
        );
        let report = engine
            .transform_with_report(&reported, config)
            .expect("transform with report should succeed");
        let flatten_groups =
            js_sys::Reflect::get(&report, &JsValue::from_str("flattenGroups")).unwrap();
        assert!(js_sys::Reflect::has(&report, &JsValue::from_str("flattenGroups")).unwrap());
        assert!(js_sys::Reflect::has(&flatten_groups, &JsValue::from_str("guardHits")).unwrap());
        assert!(!js_sys::Reflect::has(&report, &JsValue::from_str("iterations")).unwrap());
        assert_eq!(reported.to_latex(None).unwrap(), "x");
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn wasm_engine_transform_rejects_different_knowledge_base() {
        let engine = TransformEngine::new(
            Some(
                serde_wasm_bindgen::to_value(&serde_json::json!({
                    "profile": "equiv",
                }))
                .expect("options should serialize"),
            ),
            None,
        )
        .expect("engine should build");
        let parsed_document = Document::from_core(
            engine
                .inner
                .parser()
                .parse("x")
                .try_into_document()
                .expect("parse should succeed")
                .0,
        );
        let syntax = parsed_document.to_syntax().expect("syntax should export");
        let document = Document::from_syntax(
            syntax,
            Some(KnowledgeBase {
                inner: texform::KnowledgeBase::builder().build().unwrap(),
            }),
        )
        .expect("syntax should rebuild document");

        let error = engine
            .transform(&document, None)
            .expect_err("different knowledge bases must not be transformed");

        assert_eq!(
            js_sys::Reflect::get(&error, &JsValue::from_str("kind"))
                .expect("kind should exist")
                .as_string()
                .as_deref(),
            Some("transform")
        );
    }

    #[test]
    fn wasm_parse_result_parts_keep_document_and_diagnostics() {
        let parser =
            Parser::from_options(ParserOptions::default()).expect("default parser should build");
        let config = ParseConfigInput {
            reject_unknown: Some(true),
            abort_on_error: Some(false),
            max_group_depth: None,
        }
        .into_config(parser.inner.default_parse_config().clone());

        let (document, diagnostics) =
            parse_result_parts(parser.inner.parse_with(r"\unknowncmd", &config));

        let document = document.expect("partial document should be retained");
        assert!(document.has_errors());
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn wasm_rejects_cross_document_nodes() {
        let first = Document::default();
        let second = Document::default();
        let root = first.root().expect("root should be available");
        let foreign = second
            .create_char("x", None)
            .expect("char should be created");

        assert!(
            first.append_child(&root, &foreign).is_err(),
            "foreign child should be rejected"
        );
    }

    #[test]
    fn wasm_create_command_with_arg_roundtrips_latex() {
        let document = Document::default();
        let arg = document
            .create_char("x", None)
            .expect("arg should be created");
        let command = document
            .create_command_with_args("sqrt", vec![Arg::Absent, Arg::Node(arg.id)])
            .expect("command should be created");

        document
            .append_child(
                &document.root().expect("root should be available"),
                &command,
            )
            .expect("command should be appended");

        let arg_kind = command
            .with_ref(|node| match node.arg(1).expect("arg should be present") {
                ArgRef::Math(node) => {
                    assert!(node.is_char('x'));
                    "Math"
                }
                _ => "Other",
            })
            .expect("arg should be readable");

        assert_eq!(arg_kind, "Math");
        assert_eq!(document.to_latex(None).unwrap(), r"\sqrt { x }");
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn wasm_node_exposes_prime_count() {
        let document = Document::from_core(
            texform::Parser::builder()
                .knowledge_base(
                    texform::KnowledgeBase::builder()
                        .packages(&["base"])
                        .build()
                        .unwrap(),
                )
                .build()
                .parse("f''")
                .try_into_document()
                .expect("parse should produce a document")
                .0,
        );
        let root = document.root().expect("root should be available");
        let scripted_id = root
            .with_ref(|node| node.children().next().expect("scripted child").id())
            .expect("scripted child should be readable");
        let scripted = Node::from_parts(Rc::clone(&document.inner), scripted_id);
        let prime_id = scripted
            .with_ref(|node| node.superscript().expect("prime superscript").id())
            .expect("prime should be readable");
        let prime = Node::from_parts(Rc::clone(&document.inner), prime_id);

        assert_eq!(prime.kind().unwrap(), "prime");
        assert_eq!(prime.prime_count().unwrap().as_f64(), Some(2.0));
    }

    #[test]
    fn normalize_config_input_can_disable_finalize_ast() {
        let base = texform::NormalizeConfig {
            parse: ParseConfig::LENIENT,
            transform: texform::Profile::Corpus.default_transform_config(),
        };
        let input = texform::bindings::NormalizeConfigInput {
            finalize_ast: Some(texform::bindings::FinalizeAstConfigInput {
                enabled: Some(false),
            }),
            ..Default::default()
        };
        let config = input.into_config(base);

        assert!(!config.transform.finalize_ast.enabled);
    }

    #[test]
    fn wasm_rejects_read_only_error_document_editing() {
        let parser =
            Parser::from_options(ParserOptions::default()).expect("default parser should build");
        let config = ParseConfigInput {
            reject_unknown: Some(true),
            abort_on_error: Some(false),
            max_group_depth: None,
        }
        .into_config(parser.inner.default_parse_config().clone());
        let (document, diagnostics) =
            parse_result_parts(parser.inner.parse_with(r"\unknowncmd", &config));
        assert!(!diagnostics.is_empty());

        let document = Document::from_core(document.expect("partial document should exist"));
        assert!(document.is_read_only().unwrap());

        assert!(
            document.create_char("x", None).is_err(),
            "read-only document edits should fail"
        );
    }

    #[test]
    fn lookup_command_is_mode_specific() {
        let ctx = KnowledgeBase {
            inner: texform::KnowledgeBase::builder()
                .packages(&["base", "textmacros"])
                .build()
                .unwrap(),
        };

        let math = ctx
            .lookup_command_meta("underline", "math")
            .expect("math lookup should succeed")
            .expect("underline should be known in math mode");
        let text = ctx
            .lookup_command_meta("underline", "text")
            .expect("text lookup should succeed")
            .expect("underline should be known in text mode");

        assert_eq!(math.argspec.source, "m");
        assert_eq!(text.argspec.source, "m:T");
        assert!(ctx.knows_command_name("underline"));
    }
}
