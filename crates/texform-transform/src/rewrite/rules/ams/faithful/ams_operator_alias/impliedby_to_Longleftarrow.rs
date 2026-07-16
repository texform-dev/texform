//! Collapse impliedby to the explicit AMS long-left-arrow spelling with source-backed spacing.
//!
//! ```yaml
//! proposal: impliedby-to-Longleftarrow
//! triggers:
//!   - cmd:impliedby
//! consumes:
//!   eliminates: cmd:impliedby
//!   touches: null
//! produces:
//!   - cmd:;
//!   - char:Longleftarrow
//! rewrite_patterns:
//!   - {from: \impliedby, to: \;\Longleftarrow\;}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces, RuleTarget};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static IMPLIEDBY_TO_LONGLEFTARROW: ImpliedbyToLongleftarrowRule {
        key: Ams / "impliedby-to-Longleftarrow",
        level: Faithful,
        summary: "Collapse impliedby to the explicit AMS long-left-arrow spelling with source-backed spacing.",
        fidelity: Render,
        enabled_by_packages: [Ams],
        triggers: cmd_targets![&ams::cmd::IMPLIEDBY],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&ams::cmd::IMPLIEDBY],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[
                RuleTarget::Command(&base::cmd::_SEMICOLON),
                RuleTarget::Character(&base::chars::LONGLEFTARROW_2),
            ],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &ams::cmd::IMPLIEDBY) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\impliedby")?;

            let left_spacing = cx.ast.new_node(bare_command_node(base::cmd::_SEMICOLON.name));
            let arrow = cx
                .ast
                .new_node(bare_command_node(base::chars::LONGLEFTARROW_2.name));
            let right_spacing = cx.ast.new_node(bare_command_node(base::cmd::_SEMICOLON.name));
            cx.ast.replace_with_math_sequence_preserving_scripts(
                node_id,
                vec![left_spacing],
                arrow,
                vec![right_spacing],
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Node;
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{RuleLevel, run_one_rule_for_test};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: IMPLIEDBY_TO_LONGLEFTARROW,
        level: Faithful,
        examples: [
        {
            label: impliedby_between_derivative_statements,
            packages: ["base", "ams"],
            input: r"f'(x)=0 \impliedby f(x)=c",
            expected: r"f'(x)=0 \;\Longleftarrow\; f(x)=c",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn rewrites_impliedby_as_spaced_sibling_commands() {
        let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"a \impliedby b",
            &texform_core::parse::ParseConfig::STRICT,
        );

        run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &IMPLIEDBY_TO_LONGLEFTARROW,
            RuleLevel::Faithful,
        )
        .expect("impliedby-to-Longleftarrow transform should succeed");

        let children = ast.children(ast.root());
        assert_eq!(children.len(), 5);
        assert_eq!(ast.node(children[0]), &Node::Char('a'));
        assert_command(ast.node(children[1]), ";");
        assert_command(ast.node(children[2]), "Longleftarrow");
        assert_command(ast.node(children[3]), ";");
        assert_eq!(ast.node(children[4]), &Node::Char('b'));
    }

    transform_examples! {
        rule: IMPLIEDBY_TO_LONGLEFTARROW,
        level: Faithful,
        examples: [
        {
            label: scripted_impliedby_preserves_token_attachment,
            packages: ["base", "ams"],
            input: r"\impliedby_i^2",
            expected: r"\;\Longleftarrow\;_i^2",
        },
        ]
    }

    fn assert_command(node: &Node, expected_name: &str) {
        match node {
            Node::Command { name, args, known } => {
                assert_eq!(name, expected_name);
                assert!(args.is_empty());
                assert!(*known);
            }
            other => panic!("expected command {expected_name}, got {other:?}"),
        }
    }
}
