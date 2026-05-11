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

use texform_specs::builtin::base;

use super::helpers::linebreak_command;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static NEWLINE_TO_LINEBREAK: NewlineToLinebreakRule {
        key: Base / "newline-to-linebreak",
        class: Standard,
        summary: "Collapse newline to the explicit linebreak command.",
        phase: Normalize,
        safety: Semantic,
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

            cx.ast.replace_node(node_id, linebreak_command());
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ArgumentKind, ArgumentValue, Node};
    use crate::parse::ParseContext;
    use crate::transform::{RuleClass, TransformContextBuilder, TransformRule as _, transform_ast};
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: NEWLINE_TO_LINEBREAK,
        class: Standard,
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
        let transform_ctx = TransformContextBuilder::from_classes(&[RuleClass::Standard])
            .only(NEWLINE_TO_LINEBREAK.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\newline", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("newline-to-linebreak transform should succeed");

        assert_eq!(output.applied.len(), 1);
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
