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

#[test]
fn serialize_options_are_constructible_from_the_facade() {
    use texform::{Parser, ScriptOrder, ScriptSpacing, SerializeOptions};

    let parser = Parser::builder().build().expect("parser should build");
    let document = parser
        .parse("x_i^2")
        .try_into_document()
        .expect("parse should produce a document")
        .0;
    let options = SerializeOptions {
        script_spacing: ScriptSpacing::Compact,
        script_order: ScriptOrder::SupFirst,
        ..SerializeOptions::default()
    };

    assert_eq!(
        document
            .to_latex_with(&options)
            .expect("document should serialize"),
        "x^{ 2 }_{ i }"
    );
}

#[test]
fn serialize_options_serde_rejects_unknown_nested_keys() {
    let error = serde_json::from_value::<SerializeOptions>(serde_json::json!({
        "math": { "scripts": { "order": "sup_first" } }
    }))
    .unwrap_err();
    assert!(
        error.to_string().contains("unknown field `math`"),
        "{error}"
    );
}

#[test]
fn middle_delimiter_serializes_bare_and_is_text_idempotent() {
    let parser = texform::Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");
    let parsed = parser.parse(r"\left\langle a\middle\vert b\right\rangle");
    assert!(
        parsed.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        parsed.diagnostics()
    );
    let document = parsed
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    let latex = document.to_latex().expect("document should serialize");
    assert!(
        latex.contains(r"\middle \vert"),
        "expected a bare middle delimiter, got {latex}"
    );
    assert!(
        !latex.contains(r"{\vert}"),
        "mandatory delimiter must not be brace-wrapped, got {latex}"
    );

    let again = parser
        .parse(&latex)
        .try_into_document()
        .expect("serialized output should parse")
        .0;
    assert_eq!(
        again
            .to_latex()
            .expect("reserialized document should serialize"),
        latex
    );
}

#[test]
fn bigl_non_ascii_token_spans_use_utf8_byte_offsets() {
    let parser = texform::Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");
    let document = parser
        .parse(r"\bigl( é \bigr)")
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    let result = document
        .to_tokenized_latex()
        .expect("document should serialize with tokens");
    assert_eq!(result.latex, document.to_latex().unwrap());
    assert_token_contract(&result);
    assert!(
        result
            .tokens
            .iter()
            .any(|token| { token.text == "é" && token.kind == SerializationTokenKind::Character })
    );
}
