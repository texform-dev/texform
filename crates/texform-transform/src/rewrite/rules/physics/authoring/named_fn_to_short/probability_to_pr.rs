//! Collapse the long Probability helper to the standard Pr operator form.
//!
//! ```yaml
//! proposal: probability-to-pr
//! triggers:
//!   - cmd:Probability
//! consumes:
//!   eliminates: cmd:Probability
//!   touches: null
//! produces: cmd:Pr
//! rewrite_patterns:
//!   - {from: \Probability, to: \Pr}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static PROBABILITY_TO_PR: ProbabilityToPrRule {
        key: Physics / "probability-to-pr",
        level: Authoring,
        summary: "Collapse the long Probability helper to the standard Pr operator form.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        canonical: &base::cmd::PR,
        aliases: [&physics::cmd::PROBABILITY],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PROBABILITY_TO_PR,
        level: Authoring,
        examples: [
        {
            label: probability_conditional,
            packages: ["base", "physics"],
            input: r"\Probability(A \mid B)=\frac{1}{2}",
            expected: r"\Pr(A \mid B)=\frac{1}{2}",
        },
        ]
    }
    // END: Generated examples
}
