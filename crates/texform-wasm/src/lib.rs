use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

use texform::bindings::{ParseConfigInput, TransformConfigInput, transform_report_to_dto};
use texform::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, ArgRef, ArgValue,
    ContentMode, DelimiterRef, DelimiterValue, SyntaxNode,
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
    pub fn new() -> Document {
        Self::from_core(texform::Document::new())
    }

    #[wasm_bindgen(js_name = fromSyntax)]
    pub fn from_syntax(node: JsValue) -> Result<Document, JsValue> {
        let node = serde_wasm_bindgen::from_value::<SyntaxNode>(node)
            .map_err(|error| parse_message_to_js(format!("invalid syntax node: {error}")))?;
        texform::Document::from_syntax(&node)
            .map(Self::from_core)
            .map_err(|error| {
                binding_error_to_js(texform::bindings::from_syntax_error_to_dto(error))
            })
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
            .map_err(|error| {
                binding_error_to_js(texform::bindings::BindingErrorDto {
                    kind: "internal",
                    message: error.to_string(),
                    diagnostics: Vec::new(),
                })
            })
    }

    #[wasm_bindgen(js_name = toTokenizedLatex)]
    pub fn to_tokenized_latex(&self, options: Option<JsValue>) -> Result<JsValue, JsValue> {
        let options = serialize_options_from_js(options)?;
        let result = borrow_document(&self.inner)?
            .to_tokenized_latex_with(&options)
            .map_err(|error| {
                binding_error_to_js(texform::bindings::BindingErrorDto {
                    kind: "internal",
                    message: error.to_string(),
                    diagnostics: Vec::new(),
                })
            })?;
        binding_dto_to_js(&texform::bindings::tokenized_latex_to_dto(result))
    }

    #[wasm_bindgen(js_name = createChar)]
    pub fn create_char(&self, value: &str) -> Result<Node, JsValue> {
        let mut chars = value.chars();
        let Some(ch) = chars.next() else {
            return Err(edit_message_to_js("createChar expects one character"));
        };
        if chars.next().is_some() {
            return Err(edit_message_to_js("createChar expects one character"));
        }
        let id = borrow_document_mut(&self.inner)?
            .create_char(ch)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = createText)]
    pub fn create_text(&self, value: &str) -> Result<Node, JsValue> {
        let id = borrow_document_mut(&self.inner)?
            .create_text(value)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = createActiveSpace)]
    pub fn create_active_space(&self) -> Result<Node, JsValue> {
        let id = borrow_document_mut(&self.inner)?
            .create_active_space()
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = createAlignmentTab)]
    pub fn create_alignment_tab(&self) -> Result<Node, JsValue> {
        let id = borrow_document_mut(&self.inner)?
            .create_alignment_tab()
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = createGroup)]
    pub fn create_group(&self, mode: &str) -> Result<Node, JsValue> {
        let mode = parse_content_mode(mode)?;
        let id = borrow_document_mut(&self.inner)?
            .create_group(mode)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = createCommand)]
    pub fn create_command(&self, name: &str, args: Option<JsValue>) -> Result<Node, JsValue> {
        let args = parse_arg_values(&self.inner, args)?;
        self.create_command_with_args(name, args)
    }

    #[wasm_bindgen(js_name = createDeclarative)]
    pub fn create_declarative(&self, name: &str, args: Option<JsValue>) -> Result<Node, JsValue> {
        let args = parse_arg_values(&self.inner, args)?;
        let id = borrow_document_mut(&self.inner)?
            .create_declarative(name, args)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }

    #[wasm_bindgen(js_name = createEnvironment)]
    pub fn create_environment(
        &self,
        name: &str,
        args: Option<JsValue>,
        body: &Node,
    ) -> Result<Node, JsValue> {
        self.ensure_same_document(body)?;
        let args = parse_arg_values(&self.inner, args)?;
        let id = borrow_document_mut(&self.inner)?
            .create_environment(name, args, body.id)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
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
        let ch = parse_single_char(value, "setChar").map_err(edit_message_to_js)?;
        borrow_document_mut(&self.inner)?
            .set_char(node.id, ch)
            .map_err(edit_error_to_js)
    }

    #[wasm_bindgen(js_name = setArg)]
    pub fn set_arg(&self, node: &Node, index: usize, value: JsValue) -> Result<(), JsValue> {
        self.ensure_same_document(node)?;
        let value = parse_arg_value(&self.inner, value)?;
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

    fn ensure_same_document(&self, node: &Node) -> Result<(), JsValue> {
        if Rc::ptr_eq(&self.inner, &node.document) {
            Ok(())
        } else {
            Err(edit_message_to_js("node belongs to a different document"))
        }
    }

    fn create_command_with_args(&self, name: &str, args: Vec<ArgValue>) -> Result<Node, JsValue> {
        let id = borrow_document_mut(&self.inner)?
            .create_command(name, args)
            .map_err(edit_error_to_js)?;
        Ok(Node::from_parts(Rc::clone(&self.inner), id))
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
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
            .map(|value| parse_single_char(value, "isChar"))
            .transpose()
            .map_err(config_error_to_js)?;
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
                .map(content_mode_to_string)
                .map(JsValue::from)
                .unwrap_or(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = groupKind)]
    pub fn group_kind(&self) -> Result<JsValue, JsValue> {
        let value = self.with_ref(|node| node.group_kind().map(group_kind_to_js))?;
        Ok(value.transpose()?.unwrap_or(JsValue::NULL))
    }

    #[wasm_bindgen(js_name = argCount)]
    pub fn arg_count(&self) -> Result<usize, JsValue> {
        self.with_ref(|node| node.arg_count())
    }

    pub fn arg(&self, index: usize) -> Result<JsValue, JsValue> {
        self.with_ref(|node| {
            node.arg(index)
                .map(|arg| arg_ref_to_js(&self.document, arg))
        })
        .and_then(|value| value.transpose())
        .map(|value| value.unwrap_or(JsValue::NULL))
    }

    #[wasm_bindgen(js_name = argSlots)]
    pub fn arg_slots(&self) -> Result<js_sys::Array, JsValue> {
        let values = self.with_ref(|node| {
            node.arg_slots()
                .map(|arg| arg.map(|arg| arg_ref_to_js(&self.document, arg)))
                .collect::<Vec<_>>()
        })?;
        let out = js_sys::Array::new();
        for value in values {
            out.push(&value.transpose()?.unwrap_or(JsValue::NULL));
        }
        Ok(out)
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

fn parse_single_char(value: &str, method: &str) -> Result<char, String> {
    let mut chars = value.chars();
    let Some(ch) = chars.next() else {
        return Err(format!("{method} expects one character"));
    };
    if chars.next().is_some() {
        return Err(format!("{method} expects one character"));
    }
    Ok(ch)
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

fn parse_arg_values(
    document: &Rc<RefCell<texform::Document>>,
    value: Option<JsValue>,
) -> Result<Vec<ArgValue>, JsValue> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() || value.is_undefined() {
        return Ok(Vec::new());
    }
    let args = js_sys::Array::from(&value);
    let mut out = Vec::with_capacity(args.length() as usize);
    for arg in args.iter() {
        out.push(parse_arg_value(document, arg)?);
    }
    Ok(out)
}

fn parse_arg_value(
    document: &Rc<RefCell<texform::Document>>,
    value: JsValue,
) -> Result<ArgValue, JsValue> {
    let kind = object_string_property(&value, "kind")?;
    match kind.as_str() {
        "Math" => Ok(ArgValue::math(arg_node_id(document, &value)?)),
        "Text" => Ok(ArgValue::text(arg_node_id(document, &value)?)),
        "Delimiter" => Ok(ArgValue::delimiter(parse_delimiter_value(
            js_sys::Reflect::get(&value, &"value".into())
                .map_err(|_| edit_message_to_js("ArgValue.value is not readable"))?,
        )?)),
        "CSName" => Ok(ArgValue::cs_name(object_string_property(&value, "value")?)),
        "Dimension" => Ok(ArgValue::dimension(object_string_property(
            &value, "value",
        )?)),
        "Integer" => Ok(ArgValue::integer(object_string_property(&value, "value")?)),
        "KeyVal" => Ok(ArgValue::key_val(object_string_property(&value, "value")?)),
        "Column" => Ok(ArgValue::column(object_string_property(&value, "value")?)),
        "Boolean" => {
            let value = js_sys::Reflect::get(&value, &"value".into())
                .map_err(|_| edit_message_to_js("ArgValue.value is not readable"))?;
            value
                .as_bool()
                .map(ArgValue::boolean)
                .ok_or_else(|| edit_message_to_js("Boolean ArgValue.value must be a boolean"))
        }
        other => Err(edit_message_to_js(format!(
            "unsupported ArgValue kind: {other}"
        ))),
    }
}

fn arg_node_id(
    document: &Rc<RefCell<texform::Document>>,
    value: &JsValue,
) -> Result<texform::NodeId, JsValue> {
    let node = js_sys::Reflect::get(value, &"node".into())
        .map_err(|_| edit_message_to_js("ArgValue.node is not readable"))?;
    let handle = js_sys::Reflect::get(&node, &"__texformBindingHandle".into())
        .map_err(|_| edit_message_to_js("ArgValue.node binding handle is not readable"))?
        .as_f64()
        .ok_or_else(|| edit_message_to_js("ArgValue.node must be a Node"))? as u32;
    NODE_HANDLES.with(|handles| {
        let handles = handles.borrow();
        let (owner, id) = handles
            .get(&handle)
            .ok_or_else(|| edit_message_to_js("ArgValue.node must be a live Node"))?;
        if Rc::ptr_eq(document, owner) {
            Ok(*id)
        } else {
            Err(edit_message_to_js("node belongs to a different document"))
        }
    })
}

fn object_string_property(value: &JsValue, key: &str) -> Result<String, JsValue> {
    js_sys::Reflect::get(value, &key.into())
        .map_err(|_| edit_message_to_js(format!("{key} is not readable")))?
        .as_string()
        .ok_or_else(|| edit_message_to_js(format!("{key} must be a string")))
}

fn parse_delimiter_value(value: JsValue) -> Result<DelimiterValue, JsValue> {
    let kind = object_string_property(&value, "kind")?;
    match kind.as_str() {
        "None" => Ok(DelimiterValue::None),
        "Char" => Ok(DelimiterValue::Char(
            parse_single_char(&object_string_property(&value, "value")?, "delimiter value")
                .map_err(edit_message_to_js)?,
        )),
        "Control" => Ok(DelimiterValue::Control(object_string_property(
            &value, "value",
        )?)),
        other => Err(edit_message_to_js(format!(
            "unsupported delimiter kind: {other}"
        ))),
    }
}

fn arg_ref_to_js(
    document: &Rc<RefCell<texform::Document>>,
    arg: ArgRef<'_>,
) -> Result<JsValue, JsValue> {
    let value = js_sys::Object::new();
    match arg {
        ArgRef::Math(node) => {
            js_set(value.as_ref(), "kind", &"Math".into())?;
            js_set(
                value.as_ref(),
                "node",
                &Node::from_parts(Rc::clone(document), node.id()).into(),
            )?;
        }
        ArgRef::Text(node) => {
            js_set(value.as_ref(), "kind", &"Text".into())?;
            js_set(
                value.as_ref(),
                "node",
                &Node::from_parts(Rc::clone(document), node.id()).into(),
            )?;
        }
        ArgRef::Delimiter(delimiter) => {
            js_set(value.as_ref(), "kind", &"Delimiter".into())?;
            js_set(value.as_ref(), "value", &delimiter_ref_to_js(delimiter)?)?;
        }
        ArgRef::CSName(text) => set_scalar_arg(&value, "CSName", text)?,
        ArgRef::Dimension(text) => set_scalar_arg(&value, "Dimension", text)?,
        ArgRef::Integer(text) => set_scalar_arg(&value, "Integer", text)?,
        ArgRef::KeyVal(text) => set_scalar_arg(&value, "KeyVal", text)?,
        ArgRef::Column(text) => set_scalar_arg(&value, "Column", text)?,
        ArgRef::Boolean(flag) => {
            js_set(value.as_ref(), "kind", &"Boolean".into())?;
            js_set(value.as_ref(), "value", &JsValue::from_bool(flag))?;
        }
    }
    Ok(value.into())
}

fn set_scalar_arg(value: &js_sys::Object, kind: &str, scalar: &str) -> Result<(), JsValue> {
    js_set(value.as_ref(), "kind", &kind.into())?;
    js_set(value.as_ref(), "value", &scalar.into())
}

fn delimiter_ref_to_js(delimiter: DelimiterRef<'_>) -> Result<JsValue, JsValue> {
    let value = js_sys::Object::new();
    match delimiter {
        DelimiterRef::None => {
            js_set(value.as_ref(), "kind", &"None".into())?;
        }
        DelimiterRef::Char(ch) => {
            js_set(value.as_ref(), "kind", &"Char".into())?;
            js_set(value.as_ref(), "value", &ch.to_string().into())?;
        }
        DelimiterRef::Control(name) => {
            js_set(value.as_ref(), "kind", &"Control".into())?;
            js_set(value.as_ref(), "value", &name.into())?;
        }
    }
    Ok(value.into())
}

fn group_kind_to_js(kind: texform::GroupKindRef<'_>) -> Result<JsValue, JsValue> {
    let value = js_sys::Object::new();
    match kind {
        texform::GroupKindRef::Explicit => {
            js_set(value.as_ref(), "kind", &"Explicit".into())?;
        }
        texform::GroupKindRef::Implicit => {
            js_set(value.as_ref(), "kind", &"Implicit".into())?;
        }
        texform::GroupKindRef::Delimited { left, right } => {
            js_set(value.as_ref(), "kind", &"Delimited".into())?;
            js_set(value.as_ref(), "left", &delimiter_ref_to_js(left)?)?;
            js_set(value.as_ref(), "right", &delimiter_ref_to_js(right)?)?;
        }
        texform::GroupKindRef::InlineMath => {
            js_set(value.as_ref(), "kind", &"InlineMath".into())?;
        }
    }
    Ok(value.into())
}

#[wasm_bindgen]
pub struct Parser {
    inner: texform::Parser,
}

#[wasm_bindgen]
impl Parser {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<Parser, JsValue> {
        Ok(Parser {
            inner: parser_from_js(args)?,
        })
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
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
pub struct TransformEngine {
    inner: texform::TransformEngine,
}

#[wasm_bindgen]
impl TransformEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<TransformEngine, JsValue> {
        Ok(Self {
            inner: engine_from_js(args)?,
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

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.parser().is_delimiter_control(name)
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
        self.inner.parser().knows_command_name(name)
    }

    pub fn knows_env_name(&self, name: &str) -> bool {
        self.inner.parser().knows_env_name(name)
    }

    pub fn knows_character_name(&self, name: &str) -> bool {
        self.inner.parser().knows_character_name(name)
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
    texform::Document::from_syntax(&node)
        .map_err(|error| binding_error_to_js(texform::bindings::from_syntax_error_to_dto(error)))?
        .to_latex_with(&options)
        .map_err(|error| {
            binding_error_to_js(texform::bindings::BindingErrorDto {
                kind: "internal",
                message: error.to_string(),
                diagnostics: Vec::new(),
            })
        })
}

impl Parser {
    #[cfg(test)]
    fn from_options(input: ParserOptions) -> Result<Parser, JsValue> {
        Ok(Parser {
            inner: parser_from_options(input)?,
        })
    }

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

impl TransformEngine {
    fn lookup_command_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCommandRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.parser().lookup_command(name, mode))
    }

    fn lookup_explicit_command_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCommandRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.parser().lookup_explicit_command(name, mode))
    }

    fn lookup_character_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCharacterRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.parser().lookup_character(name, mode))
    }

    fn lookup_env_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveEnvironmentRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.parser().lookup_env(name, mode))
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

fn content_mode_to_string(mode: ContentMode) -> &'static str {
    match mode {
        ContentMode::Math => "math",
        ContentMode::Text => "text",
    }
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
    use texform::{AllowedMode, CommandItem, CommandKind, ParseConfig};

    #[test]
    fn package_load_build_errors_use_facade_error_text() {
        let error = texform::Parser::builder()
            .packages(&["missing"])
            .build()
            .expect_err("missing package should fail");

        assert_eq!(error.to_string(), "unknown package: missing");
    }

    #[test]
    fn invalid_context_item_build_errors_include_item_name() {
        let error = texform::Parser::builder()
            .empty_knowledge()
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
    fn empty_package_list_is_not_treated_like_default_packages() {
        let default_ctx =
            Parser::from_options(ParserOptions::default()).expect("default parser should build");
        let empty_packages_ctx = Parser::from_options(ParserOptions {
            packages: Some(vec![]),
            ..Default::default()
        })
        .expect("empty package list parser should build");
        let explicit_braket_ctx = Parser::from_options(ParserOptions {
            packages: Some(vec!["braket".into()]),
            ..Default::default()
        })
        .expect("explicit braket parse context should build");

        assert!(
            default_ctx
                .inner
                .lookup_command("frac", ContentMode::Math)
                .is_some()
        );
        assert!(
            default_ctx
                .inner
                .lookup_command("Bra", ContentMode::Math)
                .is_some()
        );
        assert!(
            empty_packages_ctx
                .inner
                .lookup_command("frac", ContentMode::Math)
                .is_none()
        );
        assert!(
            explicit_braket_ctx
                .inner
                .lookup_command("Bra", ContentMode::Math)
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
        let engine = TransformEngine::new(Some(
            serde_wasm_bindgen::to_value(&serde_json::json!({
                "profile": "equiv",
                "packages": ["base"],
            }))
            .expect("options should serialize"),
        ))
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
    fn wasm_engine_transform_rejects_document_without_parse_context() {
        let engine = TransformEngine::new(Some(
            serde_wasm_bindgen::to_value(&serde_json::json!({
                "profile": "equiv",
                "packages": ["base"],
            }))
            .expect("options should serialize"),
        ))
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
        let document = Document::from_syntax(syntax).expect("syntax should rebuild document");

        let error = engine
            .transform(&document, None)
            .expect_err("syntax-created documents must not be transformed");

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
        let first = Document::new();
        let second = Document::new();
        let root = first.root().expect("root should be available");
        let foreign = second.create_char("x").expect("char should be created");

        assert!(
            first.append_child(&root, &foreign).is_err(),
            "foreign child should be rejected"
        );
    }

    #[test]
    fn wasm_create_command_with_arg_roundtrips_latex() {
        let document = Document::new();
        let arg = document.create_char("x").expect("arg should be created");
        let command = document
            .create_command_with_args("sqrt", vec![ArgValue::math(arg.id)])
            .expect("command should be created");

        document
            .append_child(
                &document.root().expect("root should be available"),
                &command,
            )
            .expect("command should be appended");

        let arg_kind = command
            .with_ref(|node| match node.arg(0).expect("arg should be present") {
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
                .packages(&["base"])
                .build()
                .expect("parser should build")
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
            document.create_char("x").is_err(),
            "read-only document edits should fail"
        );
    }

    #[test]
    fn lookup_command_is_mode_specific() {
        let ctx = Parser::from_options(ParserOptions {
            packages: Some(vec!["base".into(), "textmacros".into()]),
            ..Default::default()
        })
        .expect("parse context should build");

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
