//! Collapse quantity to the shorter qty helper.
//!
//! ```yaml
//! proposal: quantity-to-qty
//! triggers:
//!   - cmd:quantity
//! consumes:
//!   eliminates: cmd:quantity
//!   touches: null
//! produces: cmd:qty
//! rewrite_patterns:
//!   - {label: paren, from: \quantity(#1), to: \qty(#1)}
//!   - {label: bracket, from: '\quantity[#1]', to: '\qty[#1]'}
//!   - {label: brace, from: '\quantity{#1}', to: '\qty{#1}'}
//!   - {label: vertical-bar, from: \quantity|#1|, to: \qty|#1|}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static QUANTITY_TO_QTY: QuantityToQtyRule {
        key: Physics / "quantity-to-qty",
        class: Standard,
        summary: "Collapse quantity to the shorter qty helper.",
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::QTY,
        aliases: [&physics::cmd::QUANTITY],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentValue, Ast, Node, NodeId};
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RuleClass};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: QUANTITY_TO_QTY,
        class: Standard,
        examples: [
        {
            label: qty_alias,
            packages: ["base", "physics"],
            input: r"\quantity(a+b)",
            expected: r"\qty(a+b)",
        },
        {
            label: qty_alias_brace_context,
            packages: ["base", "physics"],
            input: r"g=\quantity{\frac{a}{b}}",
            expected: r"g=\qty{\frac{a}{b}}",
        },
        {
            label: qty_alias_bracket_context,
            packages: ["base", "physics"],
            input: r"\quantity[a+b]^{-1}",
            expected: r"\qty[a+b]^{-1}",
        },
        {
            label: qty_alias_vertical_bar_context,
            packages: ["base", "physics"],
            input: r"\quantity|x-y|<\varepsilon",
            expected: r"\qty|x-y|<\varepsilon",
        },
        ]
    }
    // END: Generated examples

    fn assert_subtree_contains_char(ast: &Ast, node_id: NodeId, expected: char) {
        match ast.node(node_id) {
            Node::Char(actual) => assert_eq!(*actual, expected),
            Node::Group { .. } => {
                let children = ast.children(node_id);
                assert_eq!(children.len(), 1);
                assert_eq!(ast.node(children[0]), &Node::Char(expected));
            }
            other => panic!(
                "expected subtree containing char {:?}, got {:?}",
                expected, other
            ),
        }
    }

    #[test]
    fn rewrites_quantity_to_qty_and_preserves_argument_content() {
        let parse_ctx = ParseContext::from_packages(&["physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"\quantity{a}", &texform_core::parse::ParseConfig::STRICT);

        let output =
            run_one_rule_for_test(&mut ast, &parse_ctx, &QUANTITY_TO_QTY, RuleClass::Standard)
            .expect("quantity-to-qty transform should succeed");

        assert_eq!(output.rewrite.applied.len(), 1);
        assert_eq!(output.rewrite.applied[0].key.to_string(), "physics/quantity-to-qty");

        let root = ast.root();
        let children = ast.children(root);
        assert_eq!(children.len(), 1);

        match ast.node(children[0]) {
            Node::Command { name, args, .. } => {
                assert_eq!(name, "qty");
                assert_eq!(args.len(), 1);

                let argument = args[0].as_ref().expect("qty argument should exist");
                let content_id = match argument.value {
                    ArgumentValue::MathContent(id) => id,
                    ref other => panic!("expected content arg, got {:?}", other),
                };
                assert_subtree_contains_char(&ast, content_id, 'a');
            }
            other => panic!("expected qty command after transform, got {:?}", other),
        }
    }
}
