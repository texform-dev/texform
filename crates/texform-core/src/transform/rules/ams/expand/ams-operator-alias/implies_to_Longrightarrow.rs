//! Collapse implies to the explicit AMS long-right-arrow spelling with source-backed spacing.
//!
//! ```yaml
//! proposal: implies-to-Longrightarrow
//! consumes:
//!   eliminates: cmd:implies
//!   touches: null
//! produces: cmd:;
//! rewrite_patterns:
//!   - {label: implies, from: \implies, to: \;\Longrightarrow\;}
//! ```

use texform_specs::argspec;
use texform_specs::specs::{AllowedMode, BuiltinCommandRecord, CommandKind};
use texform_specs::builtin::ams;

use crate::ast::Node;
use crate::transform::helpers::replace_with_math_sequence;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

static SEMICOLON: BuiltinCommandRecord = BuiltinCommandRecord {
    name: ";",
    kind: CommandKind::Prefix,
    allowed_mode: AllowedMode::Math,
    argspec: argspec!(""),
    tags: &["spacing"],
};

fn zero_arg_command(name: &str) -> Node {
    Node::Command {
        name: name.to_string(),
        args: Vec::new(),
        known: true,
    }
}

define_rule! {
    /// Collapse implies to the explicit AMS long-right-arrow spelling with source-backed spacing.
    pub static IMPLIES_TO_LONGRIGHTARROW: ImpliesToLongrightarrowRule {
        key: Ams / "implies-to-Longrightarrow",
        tier: Expand,
        summary: "Collapse implies to the explicit AMS long-right-arrow spelling with source-backed spacing.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Ams],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&ams::cmd::IMPLIES],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&SEMICOLON],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &ams::cmd::IMPLIES) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, command.args, "\\implies")?;

            let left_spacing = cx.ast.new_node(zero_arg_command(";"));
            let right_spacing = cx.ast.new_node(zero_arg_command(";"));
            replace_with_math_sequence(
                cx,
                node_id,
                vec![left_spacing],
                zero_arg_command("Longrightarrow"),
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
    use crate::transform::TransformRule as _;
    use crate::transform::transform_examples;
    use crate::transform::{RuleTier, TransformContextBuilder, transform_ast};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: IMPLIES_TO_LONGRIGHTARROW,
        tier: Expand,
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
        let transform_ctx = TransformContextBuilder::from_tiers(&[RuleTier::Expand])
            .only(IMPLIES_TO_LONGRIGHTARROW.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"a \implies b", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("implies-to-Longrightarrow transform should succeed");

        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].count, 1);
        assert_eq!(
            output.applied[0].key.to_string(),
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
        let transform_ctx = TransformContextBuilder::from_tiers(&[RuleTier::Expand])
            .only(IMPLIES_TO_LONGRIGHTARROW.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\implies^2", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("implies-to-Longrightarrow transform should succeed");

        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].count, 1);
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
