//! Root-relative addressing and read-side metadata contracts.

use texform::{ArgKindRef, ArgRef, Document, EditError, NodeSlot, Parser};

fn parse(source: &str) -> Document {
    Parser::builder()
        .build()
        .parse(source)
        .try_into_document()
        .unwrap()
        .0
}

#[test]
fn paths_round_trip_all_nodes_and_match_source_span_paths() {
    for source in [
        r"\frac{a}{b}+x_i^2",
        r"{a\over b}",
        r"\begin{array}{cc}a&b\\c&d\end{array}",
        r"\operatorname{sin} x",
    ] {
        let doc = parse(source);
        let cloned = doc.clone();
        assert_eq!(doc.root().path().as_deref(), Some("root"));
        assert_eq!(doc.root().slot(), None);
        for node in std::iter::once(doc.root()).chain(doc.root().descendants()) {
            let path = node.path().unwrap();
            assert_eq!(doc.node_at(&path).unwrap().id(), node.id());
            assert_eq!(cloned.node_at(&path).unwrap().kind(), node.kind());
        }
        for entry in doc.node_spans() {
            let node = doc.node_at(&entry.id).unwrap();
            assert_eq!(node.path().as_deref(), Some(entry.id.as_str()));
            assert_eq!(node.span(), Some(entry.span));
        }
    }
}

#[test]
fn slots_identify_every_parent_edge() {
    for (source, path, slot) in [
        ("x", "root.child.0", NodeSlot::Child(0)),
        (
            r"\frac{a}{b}",
            "root.child.0.arg.1.content",
            NodeSlot::Arg(1),
        ),
        ("x_i^2", "root.child.0.base", NodeSlot::ScriptBase),
        ("x_i^2", "root.child.0.sub", NodeSlot::Subscript),
        ("x_i^2", "root.child.0.sup", NodeSlot::Superscript),
        (r"a\over b", "root.child.0.left", NodeSlot::InfixLeft),
        (r"a\over b", "root.child.0.right", NodeSlot::InfixRight),
        (
            r"\begin{matrix}x\end{matrix}",
            "root.child.0.body",
            NodeSlot::EnvBody,
        ),
    ] {
        assert_eq!(parse(source).node_at(path).unwrap().slot(), Some(slot));
    }
}

#[test]
fn malformed_missing_and_scalar_paths_are_rejected() {
    let doc = parse(r"\sqrt{x}\label{eq:x}");
    for path in [
        "",
        "Root",
        "root.",
        "root..child.0",
        "root.child",
        "root.child.-1",
        "root.child.+0",
        "root.child.00",
        "root.child. 0",
        "root.child.99999999999999999999999999",
        "root.child.99",
        "root.child.0.arg",
        "root.child.0.arg.0.content",
        "root.child.0.arg.1",
        "root.child.0.arg.1.child.0",
        "root.child.1.arg.0.content",
        "root.child.0.base",
        "root.child.0.content",
        "root.child.0.arg.1.content.",
    ] {
        assert!(
            matches!(doc.node_at(path), Err(EditError::NodeNotFound)),
            "{path}"
        );
    }
}

#[test]
fn detached_descendants_keep_slots_but_lose_paths_and_edits_shift_paths() {
    let mut doc = parse("{ab}c");
    let group = doc.node_at("root.child.0").unwrap().id();
    let child = doc.node_at("root.child.0.child.1").unwrap().id();
    let trailing = doc.node_at("root.child.1").unwrap().id();
    doc.extract(group).unwrap();
    assert_eq!(doc.node(group).unwrap().path(), None);
    assert_eq!(doc.node(group).unwrap().slot(), None);
    assert_eq!(doc.node(child).unwrap().path(), None);
    assert_eq!(doc.node(child).unwrap().slot(), Some(NodeSlot::Child(1)));
    assert_eq!(
        doc.node(trailing).unwrap().path().as_deref(),
        Some("root.child.0")
    );
    let root = doc.root().id();
    doc.append_child(root, group).unwrap();
    assert_eq!(
        doc.node(child).unwrap().path().as_deref(),
        Some("root.child.1.child.1")
    );
}

#[test]
fn argument_forms_operator_names_and_known_status_remain_distinct() {
    let doc = parse(r"\sqrt{x}\sqrt[3]{y}\operatorname{sin}\unknownpathcommand");
    let sqrt = doc.node_at("root.child.0").unwrap();
    assert!(sqrt.arg_kind(0).is_none());
    assert!(matches!(sqrt.arg_kind(1), Some(ArgKindRef::Mandatory)));
    assert!(matches!(
        doc.node_at("root.child.1").unwrap().arg_kind(0),
        Some(ArgKindRef::Optional)
    ));
    let operator = doc.node_at("root.child.2").unwrap();
    assert!(
        operator
            .arg_slots()
            .flatten()
            .any(|arg| matches!(arg, ArgRef::OperatorName(_)))
    );
    assert_eq!(sqrt.is_known(), Some(true));
    assert_eq!(doc.node_at("root.child.3").unwrap().is_known(), Some(false));
    assert_eq!(doc.root().is_known(), None);
    assert_eq!(
        parse(r"a\over b")
            .node_at("root.child.0")
            .unwrap()
            .is_known(),
        Some(true)
    );
    assert_eq!(
        parse(r"\bf x").node_at("root.child.0").unwrap().is_known(),
        Some(true)
    );
}
