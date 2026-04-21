//! Canonicalize `\quantity` into the shorter `\qty` command.

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Canonicalize `\quantity` into `\qty`.
    pub static QUANTITY_TO_QTY: QuantityToQtyRule {
        key: Physics / "quantity-to-qty",
        summary: "Canonicalize \\quantity into \\qty",
        phase: Normalize,
        safety: Lossless,
        canonical: &physics::cmd::QTY,
        aliases: [&physics::cmd::QUANTITY],
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{ArgumentValue, Ast, Node, NodeId};
    use crate::parse::ParseContext;
    use crate::transform::{transform_ast, BuiltinRuleSetId, TransformContextBuilder};

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
        let transform_ctx = TransformContextBuilder::new(BuiltinRuleSetId::Normalize)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\quantity{a}", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("quantity-to-qty transform should succeed");

        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].key.to_string(), "physics/quantity-to-qty");

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
