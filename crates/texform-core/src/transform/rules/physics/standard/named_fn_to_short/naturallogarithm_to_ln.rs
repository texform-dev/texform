//! Collapse the long natural logarithm helper to the standard ln operator.
//!
//! ```yaml
//! proposal: naturallogarithm-to-ln
//! consumes:
//!   eliminates: cmd:naturallogarithm
//!   touches: null
//! produces: cmd:ln
//! rewrite_patterns:
//!   - {label: naturallogarithm, from: \naturallogarithm, to: \ln}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use crate::ast::Node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Collapse the long natural logarithm helper to the standard ln operator.
    pub static NATURALLOGARITHM_TO_LN: NaturallogarithmToLnRule {
        key: Physics / "naturallogarithm-to-ln",
        class: Standard,
        summary: "Collapse the long natural logarithm helper to the standard ln operator.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::NATURALLOGARITHM],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LN],
        },
        apply(_rule, cx, node_id) {
            let args = match cx.node(node_id) {
                Node::Command { name, args, .. }
                    if name == physics::cmd::NATURALLOGARITHM.name =>
                {
                    args.clone()
                }
                _ => return Ok(RuleEffect::Skipped),
            };

            cx.ast.replace_node(
                node_id,
                Node::Command {
                    name: base::cmd::LN.name.to_string(),
                    args,
                    known: true,
                },
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: NATURALLOGARITHM_TO_LN,
        class: Standard,
        examples: [
        {
            label: naturallogarithm_entropy,
            packages: ["base", "physics"],
            input: r"S=k_B\naturallogarithm W",
            expected: r"S=k_B\ln W",
        },
        {
            label: naturallogarithm_product,
            packages: ["base", "physics"],
            input: r"\naturallogarithm(xy)=\naturallogarithm x+\naturallogarithm y",
            expected: r"\ln(xy)=\ln x+\ln y",
        },
        ]
    }
    // END: Generated examples
}
