//! Drop the fixed delimiter size imposed by \bigg.
//!
//! ```yaml
//! proposal: bigg-delimiter-size-drop
//! triggers:
//!   - cmd:bigg
//! consumes:
//!   eliminates: cmd:bigg
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: ordinary-delimiter, from: \bigg (, to: (}
//!   - {label: scripted-ordinary-delimiter, from: '\bigg (_{#1}', to: '(_{#1}'}
//!   - {label: literal-left-angle, from: \bigg <, to: \langle}
//!   - {label: literal-right-angle, from: \bigg >, to: \rangle}
//!   - {label: control-left-angle, from: \bigg\lt, to: \langle}
//!   - {label: control-right-angle, from: \bigg\gt, to: \rangle}
//!   - {label: null-delimiter, from: \bigg ., to: ''}
//!   - {label: scripted-null-delimiter, from: '\bigg ._{#1}', to: '{}_{#1}'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BIGG_DELIMITER_SIZE_DROP: BiggDelimiterSizeDropRule {
        key: Base / "bigg-delimiter-size-drop",
        level: Equiv,
        summary: "Drop the fixed delimiter size imposed by \\bigg.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::BIGG],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BIGG],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::BIGG) else {
                return Ok(RuleEffect::Skipped);
            };
            let scoped = cx.for_rule(Self::KEY);
            scoped.expect_arg_len(command.args, 1, r"\bigg")?;
            let delimiter =
                scoped.mandatory_delimiter(&command.args[0], r"\bigg", "argument")?;

            super::helpers::drop_fixed_delimiter_size(cx, node_id, delimiter);
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
        rule: BIGG_DELIMITER_SIZE_DROP,
        level: Equiv,
        examples: [
        {
            label: ordinary_delimiter,
            packages: ["base"],
            input: r"a\bigg(b",
            expected: r"a(b",
        },
        {
            label: scripted_ordinary_delimiter,
            packages: ["base"],
            input: r"a\bigg(_i b",
            expected: r"a(_i b",
        },
        {
            label: literal_left_angle,
            packages: ["base"],
            input: r"a\bigg<b",
            expected: r"a\langle b",
        },
        {
            label: literal_right_angle,
            packages: ["base"],
            input: r"a\bigg>b",
            expected: r"a\rangle b",
        },
        {
            label: control_left_angle,
            packages: ["base"],
            input: r"a\bigg\lt b",
            expected: r"a\langle b",
        },
        {
            label: control_right_angle,
            packages: ["base"],
            input: r"a\bigg\gt b",
            expected: r"a\rangle b",
        },
        {
            label: null_delimiter,
            packages: ["base"],
            input: r"a\bigg.b",
            expected: r"ab",
        },
        {
            label: scripted_null_delimiter,
            packages: ["base"],
            input: r"a\bigg._i b",
            expected: r"a{}_i b",
        },
        ]
    }
    // END: Generated examples

}
