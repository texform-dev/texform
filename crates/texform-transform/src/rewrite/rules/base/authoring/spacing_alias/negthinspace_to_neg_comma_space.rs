//! Collapse negthinspace to the explicit negative thin math space command.
//!
//! ```yaml
//! proposal: negthinspace-to-neg-comma-space
//! triggers:
//!   - cmd:negthinspace
//! consumes:
//!   eliminates: cmd:negthinspace
//!   touches: null
//! produces: cmd:!
//! rewrite_patterns:
//!   - {from: \negthinspace, to: \!}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::consume_following_text_separator;
use crate::rewrite::helpers::prefix_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NEGTHINSPACE_TO_NEG_COMMA_SPACE: NegthinspaceToNegCommaSpaceRule {
        key: Base / "negthinspace-to-neg-comma-space",
        level: Authoring,
        summary: "Collapse negthinspace to the explicit negative thin math space command.",
        fidelity: Render,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NEGTHINSPACE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::NEGTHINSPACE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::_EXCLAMATION],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::NEGTHINSPACE) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = command.subject();
            cx.for_rule(Self::KEY)
                .expect_no_args(command.args, &subject)?;

            cx.ast.replace_node(
                node_id,
                prefix_command_node(&base::cmd::_EXCLAMATION, Vec::new()),
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
        rule: NEGTHINSPACE_TO_NEG_COMMA_SPACE,
        level: Authoring,
        examples: [
        {
            label: negthinspace_before_subscript,
            packages: ["base"],
            input: r"A\negthinspace_{i,j}+B_{i,j}",
            expected: r"A\!_{i,j}+B_{i,j}",
        },
        {
            label: text_negthinspace_in_abbreviation,
            packages: ["base", "textmacros"],
            input: r"\text{A\negthinspace/V}",
            expected: r"\text{A\!/V}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NEGTHINSPACE_TO_NEG_COMMA_SPACE,
        level: Authoring,
        examples: [
        {
            label: text_negthinspace_consumes_control_word_separator,
            packages: ["base", "textmacros"],
            input: r"\text{A\negthinspace B}",
            expected: r"\text{A\!B}",
        },
        ]
    }
}
