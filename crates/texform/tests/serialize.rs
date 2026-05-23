use texform::{
    ContentMode, SerializeError, SerializeOptions, SyntaxNode, serialize, serialize_with,
};

#[test]
fn serialize_accepts_ast_and_syntax_root() {
    let parser = texform::Parser::builder()
        .packages(&["base"])
        .build()
        .expect("parser should build");
    let ast = parser
        .parse_to_ast(r"\frac{a}{b}", &texform::ParseConfig::STRICT_NO_RECOVER)
        .expect("parse should produce an AST");

    assert_eq!(
        serialize(&ast).expect("AST should serialize"),
        r"\frac { a } { b }"
    );

    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![
            SyntaxNode::Char('a'),
            SyntaxNode::Char('+'),
            SyntaxNode::Char('b'),
        ],
    );
    assert_eq!(
        serialize(&node).expect("syntax root should serialize"),
        "a + b"
    );
}

#[test]
fn serialize_rejects_non_root_syntax_node() {
    let node = SyntaxNode::implicit_group(ContentMode::Math, vec![SyntaxNode::Char('x')]);

    assert!(matches!(
        serialize(&node),
        Err(SerializeError::ExpectedRoot)
    ));
    assert!(matches!(
        serialize_with(&node, &SerializeOptions::default()),
        Err(SerializeError::ExpectedRoot)
    ));
}
