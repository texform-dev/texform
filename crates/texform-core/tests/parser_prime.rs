mod support;

use support::parser::parse;
use texform_interface::syntax_node::{ArgumentValue, ContentMode, GroupKind, SyntaxNode};

fn parse_math(src: &str) -> SyntaxNode {
    parse(src, true).expect("expected parse success").0
}

fn command(name: &str) -> SyntaxNode {
    SyntaxNode::Command {
        name: name.to_string(),
        args: Vec::new(),
        known: true,
    }
}

fn explicit_math_group(children: Vec<SyntaxNode>) -> SyntaxNode {
    SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: GroupKind::Explicit,
        children,
    }
}

fn implicit_math_group(children: Vec<SyntaxNode>) -> SyntaxNode {
    SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: GroupKind::Implicit,
        children,
    }
}

fn scripted(
    base: SyntaxNode,
    subscript: Option<SyntaxNode>,
    superscript: Option<SyntaxNode>,
) -> SyntaxNode {
    SyntaxNode::Scripted {
        base: Box::new(base),
        subscript: subscript.map(Box::new),
        superscript: superscript.map(Box::new),
    }
}

fn empty_prime_script(count: usize) -> SyntaxNode {
    scripted(
        implicit_math_group(vec![]),
        None,
        Some(SyntaxNode::Prime { count }),
    )
}

fn root(children: Vec<SyntaxNode>) -> SyntaxNode {
    SyntaxNode::Root {
        mode: ContentMode::Math,
        children,
    }
}

#[test]
fn ascii_prime_scripts_parse_as_prime_nodes() {
    assert_eq!(
        parse_math("f'"),
        root(vec![scripted(
            SyntaxNode::Char('f'),
            None,
            Some(SyntaxNode::Prime { count: 1 }),
        )])
    );

    assert_eq!(
        parse_math("f''"),
        root(vec![scripted(
            SyntaxNode::Char('f'),
            None,
            Some(SyntaxNode::Prime { count: 2 }),
        )])
    );
}

#[test]
fn prime_then_explicit_superscript_keeps_single_superscript_slot() {
    assert_eq!(
        parse_math("f'^2"),
        root(vec![scripted(
            SyntaxNode::Char('f'),
            None,
            Some(implicit_math_group(vec![
                SyntaxNode::Prime { count: 1 },
                SyntaxNode::Char('2'),
            ])),
        )])
    );
}

#[test]
fn braced_superscript_prime_content_stays_inside_the_group() {
    assert_eq!(
        parse_math("f^{'}"),
        root(vec![scripted(
            SyntaxNode::Char('f'),
            None,
            Some(explicit_math_group(vec![empty_prime_script(1)])),
        )])
    );

    assert_eq!(
        parse_math(r"A^{'\alpha}"),
        root(vec![scripted(
            SyntaxNode::Char('A'),
            None,
            Some(explicit_math_group(vec![
                empty_prime_script(1),
                command("alpha"),
            ])),
        )])
    );
}

#[test]
fn command_prime_is_not_collapsed_by_the_parser() {
    assert_eq!(
        parse_math(r"f^{\prime}"),
        root(vec![scripted(
            SyntaxNode::Char('f'),
            None,
            Some(explicit_math_group(vec![command("prime")])),
        )])
    );
}

#[test]
fn leading_prime_is_an_empty_base_script() {
    assert_eq!(
        parse_math("'x"),
        root(vec![empty_prime_script(1), SyntaxNode::Char('x')])
    );
}

#[test]
fn empty_base_prime_scripts_inside_groups_can_receive_scripts() {
    assert_eq!(
        parse_math("x^{'_{a}}"),
        root(vec![scripted(
            SyntaxNode::Char('x'),
            None,
            Some(explicit_math_group(vec![scripted(
                implicit_math_group(vec![]),
                Some(explicit_math_group(vec![SyntaxNode::Char('a')])),
                Some(SyntaxNode::Prime { count: 1 }),
            )])),
        )])
    );

    assert_eq!(
        parse_math("x^{'^{a}}"),
        root(vec![scripted(
            SyntaxNode::Char('x'),
            None,
            Some(explicit_math_group(vec![scripted(
                implicit_math_group(vec![]),
                None,
                Some(implicit_math_group(vec![
                    SyntaxNode::Prime { count: 1 },
                    explicit_math_group(vec![SyntaxNode::Char('a')]),
                ])),
            )])),
        )])
    );
}

#[test]
fn whitespace_separated_primes_share_one_superscript() {
    let f = || SyntaxNode::Char('f');
    let prime = |count| Some(SyntaxNode::Prime { count });
    for (source, expected) in [
        ("f' '", scripted(f(), None, prime(2))),
        ("f' ' '", scripted(f(), None, prime(3))),
        ("f'  '\n\u{2019}", scripted(f(), None, prime(3))),
        ("' ' '", empty_prime_script(3)),
        (
            "f_n' ' '",
            scripted(f(), Some(SyntaxNode::Char('n')), prime(3)),
        ),
        (
            "f' ' '^2",
            scripted(
                f(),
                None,
                Some(implicit_math_group(vec![
                    SyntaxNode::Prime { count: 3 },
                    SyntaxNode::Char('2'),
                ])),
            ),
        ),
    ] {
        assert_eq!(parse_math(source), root(vec![expected]), "{source}");
    }
}

#[test]
fn unbraced_command_argument_takes_only_the_quote() {
    let SyntaxNode::Root { children, .. } = parse_math(r"\sqrt'_e") else {
        panic!("expected math root");
    };
    let [
        SyntaxNode::Scripted {
            base,
            subscript: Some(subscript),
            superscript: None,
        },
    ] = children.as_slice()
    else {
        panic!("expected outer subscript: {children:?}");
    };
    assert_eq!(**subscript, SyntaxNode::Char('e'));
    let SyntaxNode::Command { args, .. } = base.as_ref() else {
        panic!("expected command base");
    };
    assert!(matches!(
        &args[1].as_ref().unwrap().value,
        ArgumentValue::MathContent(value) if *value == empty_prime_script(1)
    ));
}
