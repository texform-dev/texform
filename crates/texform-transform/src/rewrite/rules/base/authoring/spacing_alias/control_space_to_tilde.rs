//! Rewrite math-mode control space to the canonical active space token.
//!
//! ```yaml
//! proposal: control-space-to-tilde
//! triggers:
//!   - 'cmd: '
//! consumes:
//!   eliminates: null
//!   touches: 'cmd: '
//! produces: null
//! rewrite_patterns:
//!   - {from: '\ ', to: '~'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{ArgumentValue, Ast, ContentMode, Node, NodeId, Slot};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static CONTROL_SPACE_TO_TILDE: ControlSpaceToTildeRule {
        key: Base / "control-space-to-tilde",
        level: Authoring,
        summary: "Rewrite math-mode control space to the canonical active space token.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::_CONTROL_SPACE],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::_CONTROL_SPACE],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::_CONTROL_SPACE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY)
                .expect_no_args(command.args, "\\ ")?;

            if content_mode(cx.ast, node_id) != Some(ContentMode::Math) {
                return Ok(RuleEffect::Skipped);
            }

            cx.ast.replace_node(node_id, Node::ActiveSpace);
            Ok(RuleEffect::Applied)
        }
    }
}

fn content_mode(ast: &Ast, node_id: NodeId) -> Option<ContentMode> {
    let parent = ast.parent(node_id)?;
    match parent.slot {
        Slot::GroupChild(_) => match ast.node(parent.parent) {
            Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
            _ => None,
        },
        Slot::Argument(index) => {
            let argument = ast.arg_slots(parent.parent).get(index)?.as_ref()?;
            match argument.value {
                ArgumentValue::MathContent(_) | ArgumentValue::OperatorNameContent(_) => {
                    Some(ContentMode::Math)
                }
                ArgumentValue::TextContent(_) => Some(ContentMode::Text),
                _ => None,
            }
        }
        Slot::ScriptBase
        | Slot::ScriptSub
        | Slot::ScriptSup
        | Slot::InfixLeft
        | Slot::InfixRight => Some(ContentMode::Math),
        Slot::EnvBody => None,
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
        rule: CONTROL_SPACE_TO_TILDE,
        level: Authoring,
        examples: [
        {
            label: math_control_space,
            packages: ["base"],
            input: r"a\ b",
            expected: r"a~b",
        },
        {
            label: text_control_space_preserved,
            packages: ["base", "textmacros"],
            input: r"\text{A\ B}",
            expected: r"\text{A\ B}",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn rewrites_control_space_in_math_script() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"x^\ ",
            &texform_core::parse::ParseConfig::STRICT,
        );

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &CONTROL_SPACE_TO_TILDE,
            RuleLevel::Authoring,
        )
        .expect("control-space-to-tilde transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        assert_eq!(output.rewrite.rules[0].applied_count, 1);
        assert!(
            ast.find(ast.root(), |node| matches!(node, Node::ActiveSpace))
                .is_some()
        );
    }

    #[test]
    fn distinguishes_math_and_text_content_arguments() {
        let math_ctx = ParseContext::from_packages(&["base"]);
        let mut math_ast = crate::parse_to_ast_for_test(
            &math_ctx,
            r"\sqrt\ ",
            &texform_core::parse::ParseConfig::STRICT,
        );
        let math_space = math_ast
            .find(math_ast.root(), |node| {
                matches!(node, Node::Command { name, .. } if name == " ")
            })
            .expect("math control space should be present");
        assert!(matches!(math_ast.slot(math_space), Some(Slot::Argument(_))));

        let math_output = run_one_rule_for_test(
            &mut math_ast,
            &math_ctx,
            &CONTROL_SPACE_TO_TILDE,
            RuleLevel::Authoring,
        )
        .expect("math control-space transform should succeed");
        assert_eq!(math_output.rewrite.rules[0].applied_count, 1);
        assert_eq!(math_ast.node(math_space), &Node::ActiveSpace);

        let text_ctx = ParseContext::from_packages(&["base", "textmacros"]);
        let mut text_ast = crate::parse_to_ast_for_test(
            &text_ctx,
            r"\text\ ",
            &texform_core::parse::ParseConfig::STRICT,
        );
        let text_space = text_ast
            .find(text_ast.root(), |node| {
                matches!(node, Node::Command { name, .. } if name == " ")
            })
            .expect("text control space should be present");
        assert!(matches!(text_ast.slot(text_space), Some(Slot::Argument(_))));

        run_one_rule_for_test(
            &mut text_ast,
            &text_ctx,
            &CONTROL_SPACE_TO_TILDE,
            RuleLevel::Authoring,
        )
        .expect("text control-space transform should succeed");
        assert!(matches!(text_ast.node(text_space), Node::Command { name, .. } if name == " "));
    }

}
