//! Expand substack directly to the explicit centered subarray environment.
//!
//! ```yaml
//! proposal: substack-expand
//! consumes:
//!   eliminates: cmd:substack
//!   touches: null
//! produces: env:subarray
//! rewrite_patterns:
//!   - {from: '\substack{#1\\#2}', to: '\begin{subarray}{c}#1\\#2\end{subarray}'}
//! ```

use texform_specs::builtin::ams;

use crate::ast::{Argument, ArgumentKind, ArgumentValue, Node, NodeId};
use crate::transform::helpers::{
    implicit_math_group, replace_node_discarding_detached_children, required_math_content,
};
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule, env_targets};

define_rule! {
    /// Expand substack directly to the explicit centered subarray environment.
    pub static SUBSTACK_EXPAND: SubstackExpandRule {
        key: Ams / "substack-expand",
        class: Expand,
        summary: "Expand substack directly to the explicit centered subarray environment.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Ams],
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
            cx.expect_arg_len(rule.meta().key, command.args, 1, r"\substack")?;
            let body = required_math_content(rule.meta().key, cx, &command.args[0], r"\substack", "body")?;
            let body = environment_body(cx, body);
            let replacement = Node::Environment {
                name: ams::env::SUBARRAY.name.to_string(),
                args: vec![center_column_arg()],
                known: true,
                body,
            };

            replace_node_discarding_detached_children(cx, node_id, replacement);
            Ok(RuleEffect::Applied)
        }
    }
}

fn environment_body(cx: &mut crate::transform::rule_context::RuleContext<'_>, body: NodeId) -> NodeId {
    let body = cx.ast.clone_subtree(body);
    match cx.ast.node(body) {
        Node::Group { .. } => body,
        _ => implicit_math_group(cx, vec![body]),
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
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SUBSTACK_EXPAND,
        class: Expand,
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
        class: Expand,
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
