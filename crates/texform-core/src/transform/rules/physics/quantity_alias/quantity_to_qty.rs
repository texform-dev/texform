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
    use crate::context::ParseContext;
    use crate::transform::{RuleAvailability, TransformProfile};

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
        let ctx = ParseContext::from_packages(&["physics"]);
        let output = ctx
            .parse_and_transform(r"\quantity{a}", true, &TransformProfile::default())
            .expect("quantity-to-qty transform should succeed");

        assert_eq!(output.transform_report.applied.len(), 1);
        assert_eq!(
            output.transform_report.applied[0].key.to_string(),
            "physics/quantity-to-qty"
        );

        let root = output.ast.root();
        let children = output.ast.children(root);
        assert_eq!(children.len(), 1);

        match output.ast.node(children[0]) {
            Node::Command { name, args, .. } => {
                assert_eq!(name, "qty");
                assert_eq!(args.len(), 1);

                let argument = args[0].as_ref().expect("qty argument should exist");
                let content_id = match argument.value {
                    ArgumentValue::MathContent(id) => id,
                    ref other => panic!("expected content arg, got {:?}", other),
                };
                assert_subtree_contains_char(&output.ast, content_id, 'a');
            }
            other => panic!("expected qty command after transform, got {:?}", other),
        }
    }

    #[test]
    fn reports_rule_as_available_for_physics_profile() {
        let ctx = ParseContext::from_packages(&["physics"]);
        let statuses = ctx
            .transform_rule_statuses(&TransformProfile::default())
            .expect("profile compilation should succeed");

        let status = statuses
            .iter()
            .find(|status| status.key.to_string() == "physics/quantity-to-qty")
            .expect("quantity-to-qty status should exist");
        assert!(matches!(status.availability, RuleAvailability::Available));
    }
}
