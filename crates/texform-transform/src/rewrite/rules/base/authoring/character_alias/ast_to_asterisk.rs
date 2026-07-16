//! Canonicalize the math binary operator \ast to the pixel-identical literal asterisk form.
//!
//! ```yaml
//! proposal: ast-to-asterisk
//! triggers:
//!   - char:ast
//! consumes:
//!   eliminates: char:ast
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \ast, to: '*'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static AST_TO_ASTERISK: AstToAsteriskRule {
        key: Base / "ast-to-asterisk",
        level: Authoring,
        summary: "Canonicalize the math binary operator \\ast to the pixel-identical literal asterisk form.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::AST],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::AST],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let args = match cx.node(node_id) {
                Node::Command { name, args, .. } if name == base::chars::AST.name => args,
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, r"\ast")?;

            cx.ast.replace_node(node_id, Node::Char('*'));
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{RuleLevel, run_one_rule_for_test};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: AST_TO_ASTERISK,
        level: Authoring,
        examples: [
        {
            label: binary_operation,
            packages: ["base"],
            input: r"A \ast B",
            expected: r"A * B",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn replaces_ast_command_with_character_node() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"\ast",
            &texform_core::parse::ParseConfig::STRICT,
        );

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &AST_TO_ASTERISK,
            RuleLevel::Authoring,
        )
        .expect("ast-to-asterisk transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        assert_eq!(output.rewrite.rules[0].applied_count, 1);
        let children = ast.children(ast.root());
        assert_eq!(children.len(), 1);
        assert_eq!(ast.node(children[0]), &Node::Char('*'));
    }

    #[test]
    fn leaves_literal_asterisk_unchanged() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            "*",
            &texform_core::parse::ParseConfig::STRICT,
        );

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &AST_TO_ASTERISK,
            RuleLevel::Authoring,
        )
        .expect("literal asterisk transform should succeed");

        assert!(output.rewrite.rules.is_empty());
        let children = ast.children(ast.root());
        assert_eq!(children.len(), 1);
        assert_eq!(ast.node(children[0]), &Node::Char('*'));
    }
}
