use texform::{ContentMode, Document, SerializeOptions, SyntaxNode};

#[test]
fn document_serializes_parsed_latex() {
    let parser = texform::Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");
    let document = parser
        .parse(r"\frac{a}{b}")
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    assert_eq!(
        document.to_latex().expect("document should serialize"),
        r"\frac { a } { b }"
    );
}

#[test]
fn document_serializes_syntax_root() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![
            SyntaxNode::Char('a'),
            SyntaxNode::Char('+'),
            SyntaxNode::Char('b'),
        ],
    );
    let document = Document::from_syntax(&node).expect("syntax root should become a document");

    assert_eq!(
        document.to_latex().expect("document should serialize"),
        "a + b"
    );
    assert_eq!(
        document
            .to_latex_with(&SerializeOptions::default())
            .expect("document should serialize with options"),
        "a + b"
    );
}
