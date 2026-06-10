//! Public Document API contract tests (facade-level).

use texform::ContentMode as M;
use texform::{
    ArgValue, DelimiterValue, Document, EditError, FromSyntaxError, NodeKind, SyntaxNode,
};

#[test]
fn new_document_is_empty_editable_and_serializes() {
    let mut doc = Document::new();
    assert_eq!(doc.root().kind(), NodeKind::Root);
    assert!(!doc.has_errors());

    let a = doc.create_char('a').unwrap();
    let b = doc.create_char('b').unwrap();
    let root = doc.root().id();
    doc.append_child(root, a).unwrap();
    doc.append_child(root, b).unwrap();

    assert_eq!(doc.to_latex().unwrap(), "a b");
}

#[test]
fn editing_root_is_rejected() {
    let mut doc = Document::new();
    let root = doc.root().id();
    assert_eq!(doc.remove(root), Err(EditError::CannotEditRoot));
}

#[test]
fn document_node_lookup() {
    let doc = Document::new();
    let root = doc.root().id();
    let reread = doc.node(root).unwrap();
    assert_eq!(reread.kind(), NodeKind::Root);
    assert_eq!(reread.id(), root);

    let other = Document::new();
    let other_root = other.root().id();
    assert!(matches!(doc.node(other_root), Err(EditError::ForeignNode)));
}

#[test]
fn node_ref_exposes_prime_count() {
    let syntax = SyntaxNode::Root {
        mode: M::Math,
        children: vec![SyntaxNode::Prime { count: 2 }],
    };
    let doc = Document::from_syntax(&syntax).expect("prime syntax should build a document");
    let prime = doc
        .root()
        .children()
        .next()
        .expect("root should have a child");

    assert_eq!(prime.kind(), NodeKind::Prime);
    assert_eq!(prime.prime_count(), Some(2));
    assert_eq!(prime.char(), None);
}

#[test]
fn from_syntax_rejects_invalid_prime_count_without_panicking() {
    let syntax = SyntaxNode::Root {
        mode: M::Math,
        children: vec![SyntaxNode::Prime { count: 0 }],
    };

    assert_eq!(
        Document::from_syntax(&syntax).expect_err("zero-count prime should be rejected"),
        FromSyntaxError::InvalidPrimeCount
    );
}

#[test]
fn from_syntax_rejects_text_mode_prime_without_panicking() {
    let syntax = SyntaxNode::Root {
        mode: M::Text,
        children: vec![SyntaxNode::Prime { count: 1 }],
    };

    assert_eq!(
        Document::from_syntax(&syntax).expect_err("text-mode prime should be rejected"),
        FromSyntaxError::PrimeInTextMode
    );
}

#[test]
fn build_command_via_arg_value() {
    let mut doc = Document::new();
    let n = doc.create_char('a').unwrap();
    let d = doc.create_char('b').unwrap();
    let frac = doc
        .create_command("frac", vec![ArgValue::math(n), ArgValue::math(d)])
        .unwrap();
    let root = doc.root().id();
    doc.append_child(root, frac).unwrap();

    assert_eq!(doc.to_latex().unwrap(), r"\frac { a } { b }");
}

#[test]
fn build_command_with_public_delimiter_value() {
    let mut doc = Document::new();
    let arg = ArgValue::delimiter(DelimiterValue::Control("langle".to_string()));
    let cmd = doc.create_command("mystery", vec![arg]).unwrap();
    let root = doc.root().id();
    doc.append_child(root, cmd).unwrap();

    assert_eq!(doc.to_latex().unwrap(), r"\mystery {\langle}");
}

#[test]
fn node_spans_exports_tree_paths_for_parsed_document() {
    let parser = texform::Parser::builder()
        .packages(&["base", "ams"])
        .build()
        .expect("parser should build");

    let src = r"\frac{a}{b} + x_i^2";
    let document = parser
        .parse(src)
        .try_into_document()
        .expect("expected document")
        .0;

    let spans: std::collections::HashMap<String, texform::Span> = document
        .node_spans()
        .into_iter()
        .map(|entry| (entry.id, entry.span))
        .collect();

    let root = spans.get("root").expect("root span");
    assert_eq!(root.start, 0);
    assert_eq!(root.end, src.len());

    // \frac{a}{b}
    let frac = spans.get("root.child.0").expect("frac span");
    assert_eq!(&src[frac.start..frac.end], r"\frac{a}{b}");
    let frac_arg0 = spans
        .get("root.child.0.arg.0.content")
        .expect("frac arg 0 content span");
    assert_eq!(&src[frac_arg0.start..frac_arg0.end], "a");

    // x_i^2 — scripted node with base/sub/sup slots
    let scripted = spans.get("root.child.2").expect("scripted span");
    assert_eq!(&src[scripted.start..scripted.end], "x_i^2");
    let base = spans.get("root.child.2.base").expect("script base span");
    assert_eq!(&src[base.start..base.end], "x");
    // Script slot spans include the `_` / `^` operator characters.
    let sub = spans.get("root.child.2.sub").expect("subscript span");
    assert_eq!(&src[sub.start..sub.end], "_i");
    let sup = spans.get("root.child.2.sup").expect("superscript span");
    assert_eq!(&src[sup.start..sup.end], "^2");
}

#[test]
fn node_spans_covers_environment_body_and_infix_operands() {
    let parser = texform::Parser::builder()
        .packages(&["base", "ams"])
        .build()
        .expect("parser should build");

    let src = r"\begin{matrix} a \over b \end{matrix}";
    let document = parser
        .parse(src)
        .try_into_document()
        .expect("expected document")
        .0;

    let spans: std::collections::HashMap<String, texform::Span> = document
        .node_spans()
        .into_iter()
        .map(|entry| (entry.id, entry.span))
        .collect();

    assert!(spans.contains_key("root.child.0"), "environment span");
    let body = spans.get("root.child.0.body").expect("env body span");
    assert!(body.start > 0 && body.end <= src.len());
    let left = spans
        .get("root.child.0.body.child.0.left")
        .expect("infix left span");
    assert_eq!(&src[left.start..left.end], "a");
    let right = spans
        .get("root.child.0.body.child.0.right")
        .expect("infix right span");
    assert_eq!(&src[right.start..right.end], "b");
}

#[test]
fn node_spans_is_empty_for_documents_built_without_parsing() {
    let mut doc = Document::new();
    let a = doc.create_char('a').unwrap();
    let root = doc.root().id();
    doc.append_child(root, a).unwrap();
    assert!(doc.node_spans().is_empty());
}
