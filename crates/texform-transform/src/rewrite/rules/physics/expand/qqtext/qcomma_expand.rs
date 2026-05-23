//! Expand quick-quad punctuation helpers to explicit text punctuation plus trailing quad spacing.
//!
//! ```yaml
//! proposal: qcomma-expand
//! triggers:
//!   - cmd:qc
//!   - cmd:qcomma
//! consumes:
//!   eliminates: [cmd:qc, cmd:qcomma]
//!   touches: null
//! produces:
//!   - cmd:text
//!   - cmd:quad
//! rewrite_patterns:
//!   - {label: qc, from: \qc, to: '\text{,}\quad'}
//!   - {label: qcomma, from: \qcomma, to: '\text{,}\quad'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use crate::ast::{ContentMode, Node};
use crate::rewrite::helpers::{mandatory_content_slot, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static QCOMMA_EXPAND: QcommaExpandRule {
        key: Physics / "qcomma-expand",
        class: Expand,
        summary: "Expand quick-quad punctuation helpers to explicit text punctuation plus trailing quad spacing.",
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::QC, &physics::cmd::QCOMMA],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::QC, &physics::cmd::QCOMMA],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::TEXT, &base::cmd::QUAD],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx
                .match_command(node_id, &physics::cmd::QC)
                .or_else(|| cx.match_command(node_id, &physics::cmd::QCOMMA))
            else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, &command.subject())?;

            let comma = cx.ast.new_node(Node::Text(",".to_string()));
            let text_command = prefix_command_node(
                &base::cmd::TEXT,
                vec![mandatory_content_slot(comma, ContentMode::Text)],
            );
            let text_command = cx.ast.new_node(text_command);
            let quad = cx.ast.new_node(prefix_command_node(&base::cmd::QUAD, vec![]));

            cx.ast
                .replace_with_math_sequence(node_id, Vec::new(), text_command, vec![quad]);
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Parser;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RewriteRule as _, RuleClass};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: QCOMMA_EXPAND,
        class: Expand,
        examples: [
        {
            label: qcomma_between_clauses,
            packages: ["base", "physics"],
            input: r"a=b \qc c=d",
            expected: r"a=b \text{,}\quad c=d",
        },
        {
            label: qcomma_long_alias,
            packages: ["base", "physics"],
            input: r"x>0 \qcomma y>0",
            expected: r"x>0 \text{,}\quad y>0",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn groups_qcomma_expansion_when_not_a_sibling_node() {
        let parse_ctx = Parser::from_packages(&["base", "physics"]);
        let mut ast = parse_ctx
            .parse_to_ast(r"\qc^2", &texform_core::parse::ParseConfig::STRICT_NO_RECOVER)
            .expect("parse should succeed");

        let output = run_one_rule_for_test(&mut ast, &parse_ctx, &QCOMMA_EXPAND, RuleClass::Expand)
            .expect("qcomma-expand transform should succeed");

        assert_eq!(output.rewrite.applied.len(), 1);
        assert_eq!(output.rewrite.applied[0].count, 1);
    }
}
