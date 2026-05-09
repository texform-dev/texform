//! Collapse the long Probability helper to the standard Pr operator form.
//!
//! ```yaml
//! proposal: probability-to-pr
//! consumes:
//!   eliminates: cmd:Probability
//!   touches: null
//! produces: cmd:Pr
//! rewrite_patterns:
//!   - {label: probability, from: \Probability, to: \Pr}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse `\Probability` into `\Pr`.
    pub static PROBABILITY_TO_PR: ProbabilityToPrRule {
        key: Physics / "probability-to-pr",
        class: Standard,
        summary: "Collapse the long Probability helper to the standard Pr operator form.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &base::cmd::PR,
        aliases: [&physics::cmd::PROBABILITY],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PROBABILITY_TO_PR,
        class: Standard,
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
