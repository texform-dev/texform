//! Collapse runs of the same fixed small Spacer command for equivalence normalization.
//!
//! ```yaml
//! proposal: repeat-spacer-collapse
//! triggers:
//!   - 'cmd:,'
//!   - 'cmd::'
//!   - cmd:>
//!   - cmd:;
//!   - cmd:!
//! consumes:
//!   eliminates: null
//!   touches: ['cmd:,', 'cmd::', cmd:>, cmd:;, cmd:!]
//! produces: null
//! rewrite_patterns:
//!   - {label: comma-space, from: '\,\,', to: '\,'}
//!   - {label: colon-space, from: '\:\:', to: '\:'}
//!   - {label: gt-space, from: \>\>, to: \>}
//!   - {label: semicolon-space, from: \;\;, to: \;}
//!   - {label: negative-comma-space, from: \!\!, to: \!}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static REPEAT_SPACER_COLLAPSE: RepeatSpacerCollapseRule {
        key: Base / "repeat-spacer-collapse",
        level: Equiv,
        summary: "Collapse runs of the same fixed small Spacer command for equivalence normalization.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::_COMMA, &base::cmd::_COLON, &base::cmd::_GREATER_THAN, &base::cmd::_SEMICOLON, &base::cmd::_EXCLAMATION],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::_COMMA, &base::cmd::_COLON, &base::cmd::_GREATER_THAN, &base::cmd::_SEMICOLON, &base::cmd::_EXCLAMATION],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = [
                &base::cmd::_COMMA,
                &base::cmd::_COLON,
                &base::cmd::_GREATER_THAN,
                &base::cmd::_SEMICOLON,
                &base::cmd::_EXCLAMATION,
            ]
            .into_iter()
            .find_map(|record| cx.match_command(node_id, record))
            else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, &command.subject())?;
            let name = command.name.to_owned();

            let mut next = cx.ast.next_sibling(node_id);
            let mut collapsed = false;
            while let Some(sibling) = next {
                let is_same_spacer = matches!(
                    cx.node(sibling),
                    Node::Command { name: sibling_name, args, .. }
                        if sibling_name == &name && args.is_empty()
                );
                if !is_same_spacer {
                    break;
                }

                next = cx.ast.next_sibling(sibling);
                cx.ast.remove_node(sibling);
                collapsed = true;
            }

            Ok(if collapsed {
                RuleEffect::Applied
            } else {
                RuleEffect::Skipped
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: REPEAT_SPACER_COLLAPSE,
        level: Equiv,
        examples: [
        {
            label: comma_space,
            packages: ["base"],
            input: r"A\,\,\,B",
            expected: r"A\,B",
        },
        {
            label: colon_space,
            packages: ["base"],
            input: r"A\:\:B",
            expected: r"A\:B",
        },
        {
            label: gt_space,
            packages: ["base"],
            input: r"A\>\>\>B",
            expected: r"A\>B",
        },
        {
            label: semicolon_space,
            packages: ["base"],
            input: r"A\;\;B",
            expected: r"A\;B",
        },
        {
            label: negative_comma_space,
            packages: ["base"],
            input: r"A\!\!\!B",
            expected: r"A\!B",
        },
        {
            label: text_comma_space,
            packages: ["base", "textmacros"],
            input: r"\text{A\,\,B}",
            expected: r"\text{A\,B}",
        },
        {
            label: text_colon_space,
            packages: ["base", "textmacros"],
            input: r"\text{A\:\:B}",
            expected: r"\text{A\:B}",
        },
        {
            label: text_gt_space,
            packages: ["base", "textmacros"],
            input: r"\text{A\>\>B}",
            expected: r"\text{A\>B}",
        },
        {
            label: text_semicolon_space,
            packages: ["base", "textmacros"],
            input: r"\text{A\;\;B}",
            expected: r"\text{A\;B}",
        },
        {
            label: text_negative_comma_space,
            packages: ["base", "textmacros"],
            input: r"\text{A\!\!B}",
            expected: r"\text{A\!B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: REPEAT_SPACER_COLLAPSE,
        level: Equiv,
        examples: [
        {
            label: singleton_is_preserved,
            packages: ["base"],
            input: r"A\,B",
            expected: r"A\,B",
        },
        {
            label: mixed_small_spacers_are_preserved,
            packages: ["base"],
            input: r"A\,\;B",
            expected: r"A\,\;B",
        },
        {
            label: larger_spacers_are_preserved,
            packages: ["base"],
            input: r"A\quad\quad\enspace\enspace B",
            expected: r"A\quad\quad\enspace\enspace B",
        },
        {
            label: explicit_group_boundary_is_preserved,
            packages: ["base"],
            input: r"A\,{\,}B",
            expected: r"A\,{\,}B",
        },
        {
            label: collapses_inside_script_group,
            packages: ["base"],
            input: r"x^{\,\,}",
            expected: r"x^{\,}",
        },
        {
            label: argument_slots_do_not_share_siblings,
            packages: ["base"],
            input: r"\frac{\,}{\,}",
            expected: r"\frac{\,}{\,}",
        },
        ]
    }
}
