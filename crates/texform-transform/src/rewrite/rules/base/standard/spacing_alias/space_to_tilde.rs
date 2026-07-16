//! Collapse space to the explicit nonbreaking space token.
//!
//! ```yaml
//! proposal: space-to-tilde
//! triggers:
//!   - cmd:space
//! consumes:
//!   eliminates: cmd:space
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \space, to: '~'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static SPACE_TO_TILDE: SpaceToTildeRule {
        key: Base / "space-to-tilde",
        level: Standard,
        summary: "Collapse space to the explicit nonbreaking space token.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::SPACE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::SPACE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::SPACE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\space")?;

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
    use crate::rewrite::{NormalizationLevel, run_one_rule_for_test};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SPACE_TO_TILDE,
        level: Standard,
        examples: [
        {
            label: space_before_number,
            packages: ["base"],
            input: r"Figure\space 2 shows the result",
            expected: r"Figure~2 shows the result",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn replaces_space_command_with_active_space_node() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"\space",
            &texform_core::parse::ParseConfig::STRICT,
        );

        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &SPACE_TO_TILDE,
            NormalizationLevel::Standard,
        )
        .expect("space-to-tilde transform should succeed");

        assert_eq!(output.rewrite.rules.len(), 1);
        let children = ast.children(ast.root());
        assert_eq!(children.len(), 1);
        assert_eq!(ast.node(children[0]), &Node::ActiveSpace);
    }
}
