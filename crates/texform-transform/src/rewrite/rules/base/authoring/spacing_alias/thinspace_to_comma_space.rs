//! Collapse thinspace to the explicit thin math space command.
//!
//! ```yaml
//! proposal: thinspace-to-comma-space
//! triggers:
//!   - cmd:thinspace
//! consumes:
//!   eliminates: cmd:thinspace
//!   touches: null
//! produces: 'cmd:,'
//! rewrite_patterns:
//!   - {from: \thinspace, to: '\,'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::consume_following_text_separator;
use crate::rewrite::helpers::prefix_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static THINSPACE_TO_COMMA_SPACE: ThinspaceToCommaSpaceRule {
        key: Base / "thinspace-to-comma-space",
        level: Authoring,
        summary: "Collapse thinspace to the explicit thin math space command.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::THINSPACE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::THINSPACE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::_COMMA],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::THINSPACE) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            cx.for_rule(Self::KEY)
                .expect_no_args(command.args, &subject)?;

            cx.ast.replace_node(
                node_id,
                prefix_command_node(&base::cmd::_COMMA, Vec::new()),
            );
            consume_following_text_separator(cx.ast, node_id);
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: THINSPACE_TO_COMMA_SPACE,
        level: Authoring,
        examples: [
        {
            label: thinspace_around_equals,
            packages: ["base"],
            input: r"f(x) \thinspace = \thinspace g(x+1)",
            expected: r"f(x) \, = \, g(x+1)",
        },
        {
            label: text_thinspace_before_number,
            packages: ["base", "textmacros"],
            input: r"\text{Section\thinspace 3.2}",
            expected: r"\text{Section\,3.2}",
        },
        ]
    }
    // END: Generated examples
}
