//! Collapse lowercase trace to the local tr anchor.
//!
//! ```yaml
//! proposal: trace-to-tr
//! triggers:
//!   - cmd:trace
//! consumes:
//!   eliminates: cmd:trace
//!   touches: null
//! produces: cmd:tr
//! rewrite_patterns:
//!   - {label: trace, from: \trace, to: \tr}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static TRACE_TO_TR: TraceToTrRule {
        key: Physics / "trace-to-tr",
        class: Standard,
        summary: "Collapse lowercase trace to the local tr anchor.",
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
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: TRACE_TO_TR,
        class: Standard,
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
