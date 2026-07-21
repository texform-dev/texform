use texform::{
    ContentMode, Document, SerializationTokenKind, SerializeOptions, SyntaxNode, TokenizedLatex,
};

fn assert_token_contract(result: &TokenizedLatex) {
    let mut cursor = 0;
    for token in &result.tokens {
        assert!(token.span.start < token.span.end);
        assert!(cursor <= token.span.start);
        assert!(
            result.latex[cursor..token.span.start]
                .chars()
                .all(char::is_whitespace)
        );
        assert_eq!(token.text, result.latex[token.span.clone()]);
        cursor = token.span.end;
    }
    assert!(result.latex[cursor..].chars().all(char::is_whitespace));
}

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

#[test]
fn document_exposes_stable_tokenized_serialization_contract() {
    let parser = texform::Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");
    let document = parser
        .parse(r"\text{a\% $𝒜_i$}")
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    let result = document
        .to_tokenized_latex()
        .expect("document should serialize with tokens");
    assert_eq!(result.latex, document.to_latex().unwrap());
    assert_token_contract(&result);
    assert!(result.tokens.iter().any(|token| {
        token.text == r"\%"
            && token.kind == SerializationTokenKind::Character
            && token.mode == ContentMode::Text
    }));
    assert!(result.tokens.iter().any(|token| {
        token.text == "_"
            && token.kind == SerializationTokenKind::Character
            && token.mode == ContentMode::Math
    }));
    let options = SerializeOptions::default();
    assert_eq!(
        document.to_tokenized_latex_with(&options).unwrap().latex,
        document.to_latex_with(&options).unwrap()
    );
}
