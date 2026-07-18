//! Drop enspace in math mode and collapse each text-mode enspace run to one ordinary space.
//!
//! ```yaml
//! proposal: enspace-drop
//! triggers:
//!   - cmd:enspace
//! consumes:
//!   eliminates: cmd:enspace
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: math-enspace, from: \enspace, to: ''}
//!   - {label: text-enspace-run, from: \enspace\enspace, to: ' '}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_spacer;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static ENSPACE_DROP: EnspaceDropRule {
        key: Base / "enspace-drop",
        level: Equiv,
        summary: "Drop enspace in math mode and collapse each text-mode enspace run to one ordinary space.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::ENSPACE],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ENSPACE],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::ENSPACE) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\enspace")?;

            Ok(if drop_spacer(cx.ast, node_id, &[base::cmd::ENSPACE.name]) {
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
        rule: ENSPACE_DROP,
        level: Equiv,
        examples: [
        {
            label: math_enspace,
            packages: ["base"],
            input: r"A\enspace B",
            expected: r"AB",
        },
        {
            label: text_enspace_run,
            packages: ["base", "textmacros"],
            input: r"\text{A\enspace\enspace B}",
            expected: r"\text{A B}",
        },
        {
            label: text_enspace_singleton,
            packages: ["base", "textmacros"],
            input: r"\text{A\enspace B}",
            expected: r"\text{A B}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: ENSPACE_DROP,
        level: Equiv,
        examples: [
        {
            label: math_script_base,
            packages: ["base"],
            input: r"\enspace^2",
            expected: r"{}^2",
        },
        {
            label: text_nodes_split_runs,
            packages: ["base", "textmacros"],
            input: r"\text{A\enspace X\enspace B}",
            expected: r"\text{A X B}",
        },
        {
            label: unbraced_text_argument_separator,
            packages: ["base", "textmacros"],
            input: r"\text{A\u\enspace B}",
            expected: r"\text{A\u{ }B}",
        },
        ]
    }
}
