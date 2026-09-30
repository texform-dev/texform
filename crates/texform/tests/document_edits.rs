use texform::{Arg, ContentMode, Document, NodeKind, Parser};

fn parse(src: &str) -> Document {
    Parser::builder()
        .build()
        .parse(src)
        .try_into_document()
        .unwrap()
        .0
}

#[test]
fn scripts_resolve_bases_and_collapse_without_changing_base_identity() {
    let mut doc = parse("x_i");
    let script = doc.root().children().next().unwrap().id();
    let base = doc.node(script).unwrap().script_base().unwrap().id();
    assert_eq!(doc.set_superscript(base, Some("2".into())).unwrap(), script);
    assert_eq!(doc.node(script).unwrap().script_base().unwrap().id(), base);
    doc.set_subscript(base, None).unwrap();
    assert_eq!(doc.set_superscript(script, None).unwrap(), base);
    assert_eq!(doc.to_latex().unwrap(), "x");
    assert!(doc.node(script).is_err());
}

#[test]
fn detached_script_operations_preserve_modes_and_handles() {
    let mut doc = Document::new();
    let base = doc.create_char('x').unwrap();
    assert_eq!(doc.set_subscript(base, None).unwrap(), base);
    let script = doc.set_subscript(base, Some("i".into())).unwrap();
    assert!(doc.node(script).unwrap().path().is_none());
    assert_eq!(doc.set_subscript(script, None).unwrap(), base);
    doc.append_child(doc.root().id(), base).unwrap();
    assert_eq!(doc.to_latex().unwrap(), "x");
}

#[test]
fn script_wrapping_checks_parent_shape_and_preserves_failed_inputs() {
    for source in [r"\begin{matrix}x\end{matrix}", "{x \\over y}"] {
        let mut doc = parse(source);
        let target = if source.contains("matrix") {
            doc.root()
                .children()
                .next()
                .unwrap()
                .env_body()
                .unwrap()
                .id()
        } else {
            doc.root()
                .descendants()
                .find(|n| n.kind() == NodeKind::Infix)
                .unwrap()
                .id()
        };
        let before = doc.to_syntax();
        let candidate = doc.create_char('i').unwrap();
        assert!(
            doc.set_subscript(target, Some(Arg::Node(candidate)))
                .is_err()
        );
        assert_eq!(doc.to_syntax(), before);
        assert!(doc.node(candidate).unwrap().parent().is_none());
        assert!(doc.node(candidate).unwrap().path().is_none());
    }
    let mut text = Document::with_mode(ContentMode::Text);
    let x = text.create_char('x').unwrap();
    assert!(text.set_subscript(x, Some("i".into())).is_err());
    assert_eq!(text.node(x).unwrap().char(), Some('x'));
}

#[test]
fn editing_scripts_in_required_content_slots_keeps_their_positions() {
    let mut doc = parse(r"\frac{x}{y}");
    let numerator = doc.node_at("root.child.0.arg.0.content").unwrap().id();
    let script = doc.set_superscript(numerator, Some("2".into())).unwrap();
    assert_eq!(
        doc.node(script).unwrap().path().as_deref(),
        Some("root.child.0.arg.0.content")
    );
    assert_eq!(doc.set_superscript(script, None).unwrap(), numerator);
    doc.__validate_conformance().unwrap();
}

#[test]
fn delimiter_and_prime_updates_are_atomic() {
    let mut doc = parse(r"\left(x\right) + f'");
    let group = doc.root().children().next().unwrap().id();
    doc.set_delimiters(group, r"\langle", r"\rangle").unwrap();
    let prime = doc
        .root()
        .descendants()
        .find(|node| node.kind() == NodeKind::Prime)
        .unwrap()
        .id();
    doc.set_prime_count(prime, 3).unwrap();
    let before = doc.to_syntax();
    assert!(doc.set_prime_count(prime, 0).is_err());
    assert!(doc.set_delimiters(group, "x", ")").is_err());
    assert_eq!(doc.to_syntax(), before);
}
