use texform_core::ast::{ContentMode, NodeKind};
use texform_core::document::{Document, EditError};
use texform_core::parse::{ParseConfig, ParseContext};

fn parse(source: &str) -> Document {
    ParseContext::shared()
        .parse(source, &ParseConfig::default())
        .try_into_document()
        .expect("valid fixture")
        .0
}

/// Assert that `edit` fails a conformance check and leaves the tree unchanged.
fn assert_rejected_atomically<T: std::fmt::Debug>(
    doc: &mut Document,
    edit: impl FnOnce(&mut Document) -> Result<T, EditError>,
) {
    let before = doc.to_syntax();
    let result = edit(doc);
    assert!(
        matches!(result, Err(EditError::Conformance(_))),
        "{result:?}"
    );
    assert_eq!(doc.to_syntax(), before);
}

#[test]
fn infix_siblings_are_rejected_without_consuming_the_detached_node() {
    let mut doc = parse(r"a\over b");
    let root = doc.root().id();
    let infix = doc.root().children().next().unwrap().id();
    let sibling = doc.create_char('x').unwrap();
    assert_rejected_atomically(&mut doc, |doc| doc.append_child(root, sibling));
    assert_rejected_atomically(&mut doc, |doc| doc.insert_before(infix, sibling));
    assert_rejected_atomically(&mut doc, |doc| doc.insert_after(infix, sibling));
    assert_rejected_atomically(&mut doc, |doc| doc.insert_child(root, 0, sibling));
    let group = doc.create_group(ContentMode::Math, []).unwrap();
    doc.append_child(group, sibling).unwrap();
}

#[test]
fn unwrapping_inline_math_into_text_is_atomic() {
    let mut doc = parse(r"\text{a$x$}");
    let inline = doc
        .root()
        .descendants()
        .find(|node| {
            matches!(
                node.group_kind(),
                Some(texform_core::document::GroupKindRef::InlineMath)
            )
        })
        .unwrap()
        .id();
    assert_rejected_atomically(&mut doc, |doc| doc.unwrap(inline));
}

#[test]
fn wrapping_infix_with_existing_siblings_is_atomic() {
    let mut doc = parse(r"a\over b");
    let infix = doc.root().children().next().unwrap().id();
    let wrapper = doc.create_group(ContentMode::Math, []).unwrap();
    let sibling = doc.create_char('x').unwrap();
    doc.append_child(wrapper, sibling).unwrap();
    assert_rejected_atomically(&mut doc, |doc| doc.wrap(infix, wrapper));
    assert_eq!(doc.node(wrapper).unwrap().children().count(), 1);
}

#[test]
fn replacing_a_script_base_with_infix_is_atomic() {
    let mut doc = parse(r"x_i {a\over b}");
    let scripted = doc.root().children().next().unwrap();
    let base = scripted.script_base().unwrap().id();
    let infix = doc
        .root()
        .descendants()
        .find(|node| node.kind() == NodeKind::Infix)
        .unwrap()
        .id();
    let infix = doc.extract(infix).unwrap();
    assert_rejected_atomically(&mut doc, |doc| doc.replace_with(base, infix));
    assert!(doc.node(infix).unwrap().parent().is_none());
}

#[test]
fn invalid_character_and_name_edits_preserve_the_tree() {
    let mut doc = parse(r"x\alpha");
    let ch = doc.root().children().next().unwrap().id();
    let command = doc.root().children().nth(1).unwrap().id();
    assert_rejected_atomically(&mut doc, |doc| doc.set_char(ch, '^'));
    assert_rejected_atomically(&mut doc, |doc| doc.set_command_name(command, "frac"));
    doc.set_command_name(command, "beta").unwrap();
}

#[test]
fn replacing_environment_body_with_a_leaf_preserves_both_nodes() {
    let mut doc = parse(r"\begin{matrix}x\end{matrix}");
    let body = doc
        .root()
        .children()
        .next()
        .unwrap()
        .env_body()
        .unwrap()
        .id();
    let leaf = doc.create_char('y').unwrap();
    assert_rejected_atomically(&mut doc, |doc| doc.replace_with(body, leaf));
    assert!(doc.node(leaf).unwrap().parent().is_none());
}

#[test]
fn extracted_math_content_keeps_its_context_mode() {
    let mut doc = parse(r"\text{$x$}");
    let ch = doc
        .root()
        .descendants()
        .find(|node| node.char() == Some('x'))
        .unwrap()
        .id();
    let ch = doc.extract(ch).unwrap();
    let text_group = doc.create_group(ContentMode::Text, []).unwrap();
    assert_rejected_atomically(&mut doc, |doc| doc.append_child(text_group, ch));
    doc.append_child(doc.root().id(), ch).unwrap();
}

#[test]
fn invalid_text_edit_preserves_the_original_payload() {
    let mut doc = parse(r"\text{hello}");
    let text = doc
        .root()
        .descendants()
        .find(|node| node.kind() == NodeKind::Text)
        .unwrap()
        .id();
    assert_rejected_atomically(&mut doc, |doc| doc.set_text(text, "bad%text"));
    doc.set_text(text, "world").unwrap();
}

#[test]
fn argument_edits_support_sources_absence_and_atomic_failures() {
    use texform_core::document::Arg;
    let mut doc = parse(r"\sqrt[3]{x}");
    let command = doc.root().children().next().unwrap().id();
    doc.set_arg(command, 0, Arg::Absent).unwrap();
    doc.set_arg(command, 1, "y+z").unwrap();
    assert_rejected_atomically(&mut doc, |doc| doc.set_arg(command, 1, Arg::Absent));
    let before = doc.to_syntax();
    assert!(matches!(
        doc.set_arg(command, 1, "{"),
        Err(EditError::InvalidSource(_))
    ));
    assert_eq!(doc.to_syntax(), before);
}

#[test]
fn removing_and_clearing_group_children_preserve_valid_containers() {
    let mut doc = parse(r"{x+y}");
    let group = doc.root().children().next().unwrap().id();
    let first = doc.node(group).unwrap().children().next().unwrap().id();
    doc.remove(first).unwrap();
    assert!(matches!(doc.node(first), Err(EditError::NodeNotFound)));
    doc.clear(group).unwrap();
    assert_eq!(doc.node(group).unwrap().children().count(), 0);
}

#[test]
fn environment_rename_checks_the_existing_signature() {
    let mut doc = parse(r"\begin{matrix}x\end{matrix}");
    let environment = doc.root().children().next().unwrap().id();
    assert_rejected_atomically(&mut doc, |doc| doc.set_env_name(environment, "array"));
    doc.set_env_name(environment, "pmatrix").unwrap();
}
