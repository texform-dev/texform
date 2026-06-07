//! Expand substack directly to the explicit centered subarray environment.
//!
//! ```yaml
//! proposal: substack-expand
//! triggers:
//!   - cmd:substack
//! consumes:
//!   eliminates: cmd:substack
//!   touches: null
//! produces: env:subarray
//! rewrite_patterns:
//!   - {from: '\substack{#1\\#2}', to: '\begin{subarray}{c}#1\\#2\end{subarray}'}
//! ```

use texform_knowledge::builtin::ams;

use crate::ast::{Argument, ArgumentKind, ArgumentValue, Node, NodeId};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule, env_targets};

define_rule! {
    pub static SUBSTACK_EXPAND: SubstackExpandRule {
        key: Ams / "substack-expand",
        level: Expand,
        summary: "Expand substack directly to the explicit centered subarray environment.",
        fidelity: Lossless,
        enabled_by_packages: [Ams],
        triggers: cmd_targets![&ams::cmd::SUBSTACK],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&ams::cmd::SUBSTACK],
            touches: &[],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::SUBARRAY],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &ams::cmd::SUBSTACK) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 1, r"\substack")?;
            let body = cx.for_rule(Self::KEY).mandatory_math_content(&command.args[0], r"\substack", "body")?;
            let body = environment_body(cx, body);
            let replacement = Node::Environment {
                name: ams::env::SUBARRAY.name.to_string(),
                args: vec![center_column_arg()],
                known: true,
                body,
            };

            cx.ast.replace_node_drop_detached_children(node_id, replacement);
            Ok(RuleEffect::Applied)
        }
    }
}

fn environment_body(cx: &mut crate::rewrite::rule_context::RuleContext<'_>, body: NodeId) -> NodeId {
    let body = cx.ast.clone_subtree(body);
    match cx.ast.node(body) {
        Node::Group { .. } => body,
        _ => cx.ast.implicit_math_group(vec![body]),
    }
}

fn center_column_arg() -> Option<Argument> {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::Column("c".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SUBSTACK_EXPAND,
        level: Expand,
        examples: [
        {
            label: three_row_substack,
            packages: ["base", "ams"],
            input: r"\sum_{\substack{1\le i\le n\\ i\text{ odd}\\ i\ne 5}} a_i",
            expected: r"\sum_{\begin{subarray}{c}1\le i\le n\\ i\text{ odd}\\ i\ne 5\end{subarray}} a_i",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: SUBSTACK_EXPAND,
        level: Expand,
        examples: [
        {
            label: preserves_linebreak_spacing_argument,
            packages: ["base", "ams"],
            input: r"\substack{i=1\\[2pt]j=2}",
            expected: r"\begin{subarray}{c}i=1\\[2pt]j=2\end{subarray}",
        },
        ]
    }
}
