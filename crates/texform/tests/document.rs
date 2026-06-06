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
