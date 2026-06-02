//! Collapse implies to the explicit AMS long-right-arrow spelling with source-backed spacing.
//!
//! ```yaml
//! proposal: implies-to-Longrightarrow
//! triggers:
//!   - cmd:implies
//! consumes:
//!   eliminates: cmd:implies
//!   touches: null
//! produces: cmd:;
//! rewrite_patterns:
//!   - {from: \implies, to: \;\Longrightarrow\;}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

fn zero_arg_command(name: &str) -> Node {
    bare_command_node(name)
}

define_rule! {
    pub static IMPLIES_TO_LONGRIGHTARROW: ImpliesToLongrightarrowRule {
        key: Ams / "implies-to-Longrightarrow",
        class: Expand,
        summary: "Collapse implies to the explicit AMS long-right-arrow spelling with source-backed spacing.",
        safety: Lossless,
        enabled_by_packages: [Ams],
        triggers: cmd_targets![&ams::cmd::IMPLIES],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&ams::cmd::IMPLIES],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::_SEMICOLON],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &ams::cmd::IMPLIES) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\implies")?;

            let left_spacing = cx.ast.new_node(zero_arg_command(base::cmd::_SEMICOLON.name));
            let arrow = cx.ast.new_node(zero_arg_command("Longrightarrow"));
            let right_spacing = cx.ast.new_node(zero_arg_command(base::cmd::_SEMICOLON.name));
            cx.ast.replace_with_math_sequence(
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
    use crate::rewrite::{run_one_rule_for_test, RuleClass};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: IMPLIES_TO_LONGRIGHTARROW,
        class: Expand,
        examples: [
        {
            label: implies_between_inequalities,
            packages: ["base", "ams"],
            input: r"n>1 \implies n^2>1",
            expected: r"n>1 \;\Longrightarrow\; n^2>1",
        },
        {
            label: implies_between_set_statements,
            packages: ["base", "ams"],
            input: r"A\subset B \implies |A|\le |B|",
            expected: r"A\subset B \;\Longrightarrow\; |A|\le |B|",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn rewrites_implies_as_spaced_sibling_commands() {
        let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"a \implies b", &texform_core::parse::ParseConfig::STRICT);

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &IMPLIES_TO_LONGRIGHTARROW,
            RuleClass::Expand,
        )
            .expect("implies-to-Longrightarrow transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        assert_eq!(output.rewrite.rules[0].applied_count, 1);
        assert_eq!(
            output.rewrite.rules[0].key.to_string(),
            "ams/implies-to-Longrightarrow"
        );

        let children = ast.children(ast.root());
        assert_eq!(children.len(), 5);
        assert_eq!(ast.node(children[0]), &Node::Char('a'));
        assert_command(ast.node(children[1]), ";");
        assert_command(ast.node(children[2]), "Longrightarrow");
        assert_command(ast.node(children[3]), ";");
        assert_eq!(ast.node(children[4]), &Node::Char('b'));
    }

    #[test]
    fn groups_spaced_implies_when_used_as_script_base() {
        let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"\implies^2", &texform_core::parse::ParseConfig::STRICT);

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &IMPLIES_TO_LONGRIGHTARROW,
            RuleClass::Expand,
        )
            .expect("implies-to-Longrightarrow transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        assert_eq!(output.rewrite.rules[0].applied_count, 1);
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
