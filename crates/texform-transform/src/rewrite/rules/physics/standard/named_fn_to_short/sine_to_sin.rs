//! Collapse the long sine helper to the standard sin operator.
//!
//! ```yaml
//! proposal: sine-to-sin
//! triggers:
//!   - cmd:sine
//! consumes:
//!   eliminates: cmd:sine
//!   touches: null
//! produces: cmd:sin
//! rewrite_patterns:
//!   - {from: \sine, to: \sin}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static SINE_TO_SIN: SineToSinRule {
        key: Physics / "sine-to-sin",
        level: Standard,
        summary: "Collapse the long sine helper to the standard sin operator.",
        fidelity: Lossless,
        enabled_by_packages: [Physics],
        canonical: &base::cmd::SIN,
        aliases: [&physics::cmd::SINE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SINE_TO_SIN,
        level: Standard,
        examples: [
        {
            label: sine_in_trig_sum,
            packages: ["base", "physics"],
            input: r"y=\sine x+\cos x",
            expected: r"y=\sin x+\cos x",
        },
        ]
    }
    // END: Generated examples
}
