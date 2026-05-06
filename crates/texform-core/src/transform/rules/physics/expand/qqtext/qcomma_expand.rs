//! Expand quick-quad punctuation helpers to explicit text punctuation plus trailing quad spacing.
//!
//! ```yaml
//! proposal: qcomma-expand
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

use crate::ast::{ContentMode, GroupKind, Node, Slot};
use crate::transform::helpers::{mandatory_content, prefix_command};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Expand quick-quad punctuation helpers to explicit text punctuation plus trailing quad spacing.
    pub static QCOMMA_EXPAND: QcommaExpandRule {
        key: Physics / "qcomma-expand",
        tier: Expand,
        summary: "Expand quick-quad punctuation helpers to explicit text punctuation plus trailing quad spacing.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
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
            cx.expect_no_args(rule.meta().key, command.args, &format!(r"\{}", command.name))?;

            let comma = cx.ast.new_node(Node::Text(",".to_string()));
            let text_command = prefix_command(
                &base::cmd::TEXT,
                vec![mandatory_content(comma, ContentMode::Text)],
            );
            let quad = cx.ast.new_node(prefix_command(&base::cmd::QUAD, vec![]));

            match cx.ast.parent(node_id).map(|link| link.slot) {
                Some(Slot::GroupChild(index)) => {
                    let parent = cx
                        .ast
                        .parent_id(node_id)
                        .expect("group child should have a parent");
                    cx.ast.replace_node(node_id, text_command);
                    cx.ast.insert_child(parent, index + 1, quad);
                }
                _ => {
                    let text = cx.ast.new_node(text_command);
                    cx.ast.replace_node(
                        node_id,
                        Node::Group {
                            children: vec![text, quad],
                            kind: GroupKind::Implicit,
                            mode: ContentMode::Math,
                        },
                    );
                }
            }
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::transform::TransformRule as _;
    use crate::transform::transform_examples;
    use crate::transform::{transform_ast, RuleTier, TransformContextBuilder};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: QCOMMA_EXPAND,
        tier: Expand,
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
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let transform_ctx = TransformContextBuilder::from_tiers(&[RuleTier::Expand])
            .only(QCOMMA_EXPAND.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\qc^2", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("qcomma-expand transform should succeed");

        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].count, 1);
    }
}
