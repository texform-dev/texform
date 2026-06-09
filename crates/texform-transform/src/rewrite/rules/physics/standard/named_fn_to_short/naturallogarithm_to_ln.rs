//! Collapse the long natural logarithm helper to the standard ln operator.
//!
//! ```yaml
//! proposal: naturallogarithm-to-ln
//! triggers:
//!   - cmd:naturallogarithm
//! consumes:
//!   eliminates: cmd:naturallogarithm
//!   touches: null
//! produces: cmd:ln
//! rewrite_patterns:
//!   - {from: \naturallogarithm, to: \ln}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NATURALLOGARITHM_TO_LN: NaturallogarithmToLnRule {
        key: Physics / "naturallogarithm-to-ln",
        level: Standard,
        summary: "Collapse the long natural logarithm helper to the standard ln operator.",
        fidelity: Full,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::NATURALLOGARITHM],
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
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: NATURALLOGARITHM_TO_LN,
        level: Standard,
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
