mod support;

use support::parser::parse;
use texform_interface::syntax_node::{ContentMode, GroupKind, SyntaxNode};

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
            Some(explicit_math_group(vec![SyntaxNode::Prime { count: 1 }])),
        )])
    );

    assert_eq!(
        parse_math(r"A^{'\alpha}"),
        root(vec![scripted(
            SyntaxNode::Char('A'),
            None,
            Some(explicit_math_group(vec![
                SyntaxNode::Prime { count: 1 },
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
fn leading_prime_is_a_math_atom_not_an_empty_base_script() {
    assert_eq!(
        parse_math("'x"),
        root(vec![SyntaxNode::Prime { count: 1 }, SyntaxNode::Char('x')])
    );
}

#[test]
fn prime_atoms_inside_script_groups_can_receive_scripts() {
    assert_eq!(
        parse_math("x^{'_{a}}"),
        root(vec![scripted(
            SyntaxNode::Char('x'),
            None,
            Some(explicit_math_group(vec![scripted(
                SyntaxNode::Prime { count: 1 },
                Some(explicit_math_group(vec![SyntaxNode::Char('a')])),
                None,
            )])),
        )])
    );

    assert_eq!(
        parse_math("x^{'^{a}}"),
        root(vec![scripted(
            SyntaxNode::Char('x'),
            None,
            Some(explicit_math_group(vec![scripted(
                SyntaxNode::Prime { count: 1 },
                None,
                Some(explicit_math_group(vec![SyntaxNode::Char('a')])),
            )])),
        )])
    );
}
