//! Collapse newline to the explicit linebreak command.
//!
//! ```yaml
//! proposal: newline-to-linebreak
//! triggers:
//!   - cmd:newline
//! consumes:
//!   eliminates: cmd:newline
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \newline, to: \\}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::helpers::linebreak_command_node;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NEWLINE_TO_LINEBREAK: NewlineToLinebreakRule {
        key: Base / "newline-to-linebreak",
        level: Authoring,
        summary: "Collapse newline to the explicit linebreak command.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NEWLINE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::NEWLINE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::NEWLINE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\newline")?;

            cx.ast.replace_node(node_id, linebreak_command_node());
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentKind, ArgumentValue, Node};
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RuleLevel};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: NEWLINE_TO_LINEBREAK,
        level: Authoring,
        examples: [
        {
            label: newline_between_equation_lines,
            packages: ["base"],
            input: r"x_1 + y_1 = z_1 \newline x_2 + y_2 = z_2",
            expected: r"x_1 + y_1 = z_1 \\ x_2 + y_2 = z_2",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn rewrites_newline_to_parser_shaped_linebreak_command() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"\newline", &texform_core::parse::ParseConfig::STRICT);

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &NEWLINE_TO_LINEBREAK,
            RuleLevel::Authoring,
        )
            .expect("newline-to-linebreak transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        let children = ast.children(ast.root());
        assert_eq!(children.len(), 1);
        assert_linebreak_command(ast.node(children[0]));
    }

    fn assert_linebreak_command(node: &Node) {
        let Node::Command { name, args, known } = node else {
            panic!("expected linebreak command, got {node:?}");
        };

        assert_eq!(name, "\\");
        assert!(*known);
        assert_eq!(args.len(), 2);
        let star_arg = args[0].as_ref().expect("linebreak should carry a star slot");
        assert_eq!(star_arg.kind, ArgumentKind::Star);
        assert_eq!(star_arg.value, ArgumentValue::Boolean(false));
        assert!(args[1].is_none());
    }
}
