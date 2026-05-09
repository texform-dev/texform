//! Collapse the long sine helper to the standard sin operator.
//!
//! ```yaml
//! proposal: sine-to-sin
//! consumes:
//!   eliminates: cmd:sine
//!   touches: null
//! produces: cmd:sin
//! rewrite_patterns:
//!   - {label: sine, from: \sine, to: \sin}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse the long sine helper to the standard sin operator.
    pub static SINE_TO_SIN: SineToSinRule {
        key: Physics / "sine-to-sin",
        class: Standard,
        summary: "Collapse the long sine helper to the standard sin operator.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &base::cmd::SIN,
        aliases: [&physics::cmd::SINE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SINE_TO_SIN,
        class: Standard,
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
