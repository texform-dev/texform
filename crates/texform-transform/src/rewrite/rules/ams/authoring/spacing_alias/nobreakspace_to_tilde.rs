//! Collapse nobreakspace to the canonical nonbreaking space token.
//!
//! ```yaml
//! proposal: nobreakspace-to-tilde
//! triggers:
//!   - cmd:nobreakspace
//! consumes:
//!   eliminates: cmd:nobreakspace
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \nobreakspace, to: '~'}
//! ```

use texform_knowledge::builtin::ams;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NOBREAKSPACE_TO_TILDE: NobreakspaceToTildeRule {
        key: Ams / "nobreakspace-to-tilde",
        level: Authoring,
        summary: "Collapse nobreakspace to the canonical nonbreaking space token.",
        fidelity: Render,
        enabled_by_packages: [Ams],
        triggers: cmd_targets![&ams::cmd::NOBREAKSPACE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&ams::cmd::NOBREAKSPACE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &ams::cmd::NOBREAKSPACE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY)
                .expect_no_args(command.args, "\\nobreakspace")?;

            cx.ast.replace_node(node_id, Node::ActiveSpace);
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
        rule: NOBREAKSPACE_TO_TILDE,
        level: Authoring,
        examples: [
        {
            label: nobreakspace_between_symbols,
            packages: ["ams"],
            input: r"A\nobreakspace B",
            expected: r"A~B",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn replaces_nobreakspace_command_with_active_space_node() {
        let parse_ctx = ParseContext::from_packages(&["ams"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"\nobreakspace",
            &texform_core::parse::ParseConfig::STRICT,
        );

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &NOBREAKSPACE_TO_TILDE,
            RuleLevel::Authoring,
        )
        .expect("nobreakspace-to-tilde transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        let children = ast.children(ast.root());
        assert_eq!(children.len(), 1);
        assert_eq!(ast.node(children[0]), &Node::ActiveSpace);
    }
}
