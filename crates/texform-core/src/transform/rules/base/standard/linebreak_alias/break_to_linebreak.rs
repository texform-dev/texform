//! Collapse break to the explicit linebreak command.
//!
//! ```yaml
//! proposal: break-to-linebreak
//! consumes:
//!   eliminates: cmd:break
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \break, to: \\}
//! ```

use texform_specs::builtin::base;

use super::helpers::linebreak_command;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Collapse break to the explicit linebreak command.
    pub static BREAK_TO_LINEBREAK: BreakToLinebreakRule {
        key: Base / "break-to-linebreak",
        class: Standard,
        summary: "Collapse break to the explicit linebreak command.",
        phase: Normalize,
        safety: Semantic,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BREAK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::BREAK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.expect_no_args(rule.meta().key, command.args, "\\break")?;

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
        rule: BREAK_TO_LINEBREAK,
        class: Standard,
        examples: [
        {
            label: break_between_recurrence_lines,
            packages: ["base"],
            input: r"a_n = a_{n-1} + d \break a_{n+1} = a_n + d",
            expected: r"a_n = a_{n-1} + d \\ a_{n+1} = a_n + d",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn rewrites_break_to_parser_shaped_linebreak_command() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let transform_ctx = TransformContextBuilder::from_classes(&[RuleClass::Standard])
            .only(BREAK_TO_LINEBREAK.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\break", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("break-to-linebreak transform should succeed");

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
