//! Collapse lowercase trace to the local tr anchor.
//!
//! ```yaml
//! proposal: trace-to-tr
//! consumes:
//!   eliminates: cmd:trace
//!   touches: null
//! produces: cmd:tr
//! rewrite_patterns:
//!   - {label: trace, from: \trace, to: \tr}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse lowercase trace to the local tr anchor.
    pub static TRACE_TO_TR: TraceToTrRule {
        key: Physics / "trace-to-tr",
        tier: Base,
        summary: "Collapse lowercase trace to the local tr anchor.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::TR,
        aliases: [
            &physics::cmd::TRACE,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: TRACE_TO_TR,
        tier: Base,
        examples: [
        {
            label: trace_density_operator,
            packages: ["base", "physics"],
            input: r"Z=\trace(\rho H)",
            expected: r"Z=\tr(\rho H)",
        },
        ]
    }
    // END: Generated examples
}
