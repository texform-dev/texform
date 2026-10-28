use texform_core::ast::{Ast, Node};

use super::*;

fn parsed(source: &str) -> Ast {
    let parse_ctx = ParseContext::from_packages(DEFAULT_PACKAGES);
    parse(&parse_ctx, source).expect("source should parse")
}

#[test]
fn canonical_form_keeps_groups_in_sequences() {
    // Slot containers and the explicit/implicit distinction are representation.
    assert_eq!(
        tree_difference(&parsed(r"\frac{a}{b}"), &parsed(r"\frac a b")),
        None
    );
    // An empty group changes spacing and binding.
    let difference = tree_difference(&parsed("a{}b"), &parsed("ab")).unwrap();
    assert_eq!(difference.path, "children[1]");
    assert_eq!(
        (difference.expected.as_str(), difference.actual.as_str()),
        ("Group[0]", "Char('b')")
    );
}

#[test]
fn bare_prime_nodes_compare_as_the_prime_commands_they_serialize_to() {
    for (source, count) in [(r"x\prime", 1), (r"\sqrt{\prime\prime}", 2)] {
        let mut transformed = parsed(source);
        let primes = transformed.find_all(
            transformed.root(),
            |node| matches!(node, Node::Command { name, .. } if name == "prime"),
        );
        let first = primes[0];
        for &prime in &primes[1..] {
            transformed.remove_node(prime);
        }
        transformed.replace_node(first, Node::Prime { count });
        assert_eq!(
            tree_difference(&transformed, &parsed(source)),
            None,
            "{source}"
        );
    }
    // A superscript prime is a `'` mark, not a command.
    assert!(tree_difference(&parsed("x'"), &parsed(r"x^{\prime}")).is_some());
}

#[test]
fn differences_name_the_first_differing_edge() {
    let difference = tree_difference(&parsed(r"\sqrt{a+b}"), &parsed(r"\sqrt{a-b}")).unwrap();
    assert_eq!(difference.path, "children[0].args[1].children[1]");
    assert_eq!(
        difference.to_string(),
        "at children[0].args[1].children[1]: Char('+') vs Char('-')"
    );
}
