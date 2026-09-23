//! Convert the prime control sequence to the Prime AST node.
//!
//! ```yaml
//! proposal: prime-to-prime-node
//! triggers:
//!   - char:prime
//! consumes:
//!   eliminates: char:prime
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: ordinary-symbol, from: \prime, to: \prime}
//!   - {label: pure-superscript, from: f^\prime, to: f'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static PRIME_TO_PRIME_NODE: PrimeToPrimeNodeRule {
        key: Base / "prime-to-prime-node",
        level: Authoring,
        summary: "Convert the prime control sequence to the Prime AST node.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::PRIME],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::PRIME],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let args = match cx.node(node_id) {
                Node::Command { name, args, .. } if name == base::chars::PRIME.name => args,
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, r"\prime")?;

            cx.ast.replace_node(node_id, Node::Prime { count: 1 });
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PRIME_TO_PRIME_NODE,
        level: Authoring,
        examples: [
        {
            label: script_superscript_shorthand,
            packages: ["base"],
            input: r"f^\prime",
            expected: r"f'",
        },
        {
            label: subscripted_symbol_prime,
            packages: ["base"],
            input: r"F_\nu^\prime",
            expected: r"F_\nu'",
        },
        {
            label: ordinary_prime_symbol,
            packages: ["base"],
            input: r"\prime",
            expected: r"\prime",
        },
        {
            label: prime_after_superscript,
            packages: ["base"],
            input: r"x^2\prime",
            expected: r"x ^ { 2 } \prime",
        },
        {
            label: mixed_superscript_symbol,
            packages: ["base"],
            input: r"f^{\prime 2}",
            expected: r"f ^ { \prime 2 }",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn ordinary_symbol_rewrite_changes_ast_without_changing_latex() {
        let parse_ctx = crate::parse::ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"x\prime",
            &crate::parse::ParseConfig::STRICT,
        );
        crate::rewrite::run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &PRIME_TO_PRIME_NODE,
            crate::RuleLevel::Authoring,
        )
        .unwrap();

        let [_, symbol] = ast.children(ast.root()) else {
            panic!("expected two math symbols");
        };
        assert!(matches!(ast.node(*symbol), Node::Prime { count: 1 }));
        assert_eq!(crate::serialize::serialize(&ast), r"x \prime");
    }
}
