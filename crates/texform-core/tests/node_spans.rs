use texform_core::ast::NodeKind;
use texform_core::document::{ArgRef, Document, NodeRef};
use texform_core::parse::{ParseConfig, ParseContext, Span};
use texform_interface::syntax_node::SyntaxNode;

fn parse_ok(src: &str) -> Document {
    ParseContext::shared()
        .parse(src, &ParseConfig::default())
        .try_into_document()
        .expect("expected parse result")
        .0
}

fn assert_span(node: NodeRef<'_>, start: usize, end: usize) {
    assert_eq!(node.span(), Some(Span { start, end }));
}

fn first_root_child(document: &Document) -> NodeRef<'_> {
    document
        .root()
        .children()
        .next()
        .expect("expected a root child")
}

fn assert_scripted_superscript_prime_count(document: &Document, expected_count: usize) {
    match document.to_syntax() {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Scripted { superscript, .. } => {
                assert_eq!(
                    superscript.as_deref(),
                    Some(&SyntaxNode::Prime {
                        count: expected_count,
                    })
                );
            }
            other => panic!("expected scripted node, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

#[test]
fn parse_result_root_span_covers_smoke_cases() {
    for src in [
        "",
        "abc",
        r"\frac{a}{b}",
        r"\sqrt[3]{x}",
        r" a + b ",
        r"\begin{matrix}x\end{matrix}",
        "x^{2}_{i}",
    ] {
        let document = parse_ok(src);
        assert_span(document.root(), 0, src.len());
    }
}

#[test]
fn node_refs_expose_child_spans() {
    let document = parse_ok("x+y");
    let children: Vec<_> = document.root().children().collect();

    assert_eq!(children.len(), 3);
    assert_eq!(children[0].char(), Some('x'));
    assert_span(children[0], 0, 1);
    assert_eq!(children[1].char(), Some('+'));
    assert_span(children[1], 1, 2);
    assert_eq!(children[2].char(), Some('y'));
    assert_span(children[2], 2, 3);
}

#[test]
fn node_refs_expose_command_and_argument_spans() {
    let document = parse_ok(r"\frac{a}{bc}");
    let frac = document
        .root()
        .children()
        .next()
        .expect("expected frac command");

    assert_eq!(frac.command_name(), Some("frac"));
    assert_span(frac, 0, r"\frac{a}{bc}".len());

    let numerator = frac
        .arg(0)
        .and_then(ArgRef::as_node)
        .expect("expected numerator node");
    let denominator = frac
        .arg(1)
        .and_then(ArgRef::as_node)
        .expect("expected denominator node");

    assert_span(numerator, 6, 7);
    assert_span(denominator, 9, 11);
    let denominator_children: Vec<_> = denominator.children().collect();
    assert_span(denominator_children[0], 9, 10);
    assert_span(denominator_children[1], 10, 11);
}

#[test]
fn node_refs_expose_script_and_environment_spans() {
    let scripted_doc = parse_ok("x_i^2");
    let scripted = scripted_doc
        .root()
        .children()
        .next()
        .expect("expected scripted node");
    assert_span(scripted, 0, 5);
    assert_span(scripted.script_base().expect("expected base"), 0, 1);
    assert_span(scripted.subscript().expect("expected subscript"), 1, 3);
    assert_span(scripted.superscript().expect("expected superscript"), 3, 5);

    let env_doc = parse_ok(r"\begin{matrix}x\end{matrix}");
    let env = env_doc
        .root()
        .children()
        .next()
        .expect("expected environment");
    assert_eq!(env.env_name(), Some("matrix"));
    assert_span(env, 0, 27);
    let body = env.env_body().expect("expected environment body");
    assert_span(body, 14, 15);
    let body_children: Vec<_> = body.children().collect();
    assert_span(body_children[0], 14, 15);
}

#[test]
fn node_refs_expose_prime_superscript_spans() {
    let single = parse_ok("f'");
    let scripted = first_root_child(&single);
    let prime = scripted.superscript().expect("expected prime superscript");
    assert_eq!(prime.kind(), NodeKind::Prime);
    assert_span(prime, 1, 2);
    assert_scripted_superscript_prime_count(&single, 1);

    let double = parse_ok("f''");
    let scripted = first_root_child(&double);
    let prime = scripted.superscript().expect("expected prime superscript");
    assert_eq!(prime.kind(), NodeKind::Prime);
    assert_span(prime, 1, 3);
    assert_scripted_superscript_prime_count(&double, 2);
}

#[test]
fn node_refs_expose_unicode_prime_byte_span() {
    let src = "f\u{2019}";
    let document = parse_ok(src);
    let scripted = first_root_child(&document);
    let prime = scripted.superscript().expect("expected prime superscript");
    assert_eq!(prime.kind(), NodeKind::Prime);
    assert_span(prime, 1, src.len());
    assert_scripted_superscript_prime_count(&document, 1);
}

#[test]
fn node_refs_expose_prefix_shorthand_argument_and_outer_script_spans() {
    let src = r"\vec A_\mu";
    let document = parse_ok(src);
    let scripted = first_root_child(&document);

    assert_span(scripted, 0, src.len());

    let vec_command = scripted
        .script_base()
        .expect("expected vec command as scripted base");
    assert_eq!(vec_command.command_name(), Some("vec"));

    let argument = vec_command
        .arg(0)
        .and_then(ArgRef::as_node)
        .expect("expected vec math argument");
    assert_span(argument, 5, 6);

    let subscript = scripted.subscript().expect("expected outer subscript");
    assert_span(subscript, 6, src.len());
}

#[test]
fn node_refs_expose_infix_operand_spans() {
    let document = parse_ok(r"a+b \over c+d");
    let infix = document
        .root()
        .children()
        .next()
        .expect("expected infix node");
    let left = infix.infix_left().expect("expected left operand");
    let right = infix.infix_right().expect("expected right operand");

    assert_span(left, 0, 3);
    let left_children: Vec<_> = left.children().collect();
    assert_span(left_children[0], 0, 1);
    assert_span(left_children[2], 2, 3);

    assert_span(right, 10, 13);
    let right_children: Vec<_> = right.children().collect();
    assert_span(right_children[0], 10, 11);
    assert_span(right_children[2], 12, 13);
}
