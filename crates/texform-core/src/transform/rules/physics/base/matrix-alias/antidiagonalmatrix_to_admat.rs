//! Collapse antidiagonalmatrix to the shorter admat helper.
//!
//! ```yaml
//! proposal: antidiagonalmatrix-to-admat
//! consumes:
//!   eliminates: cmd:antidiagonalmatrix
//!   touches: null
//! produces: cmd:admat
//! rewrite_patterns:
//!   - {label: antidiagonalmatrix-braced, from: '\antidiagonalmatrix{#1}', to: '\admat{#1}'}
//!   - {label: antidiagonalmatrix-bare, from: \antidiagonalmatrix, to: \admat}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse antidiagonalmatrix to the shorter admat helper.
    pub static ANTIDIAGONALMATRIX_TO_ADMAT: AntidiagonalmatrixToAdmatRule {
        key: Physics / "antidiagonalmatrix-to-admat",
        tier: Base,
        summary: "Collapse antidiagonalmatrix to the shorter admat helper.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::ADMAT,
        aliases: [&physics::cmd::ANTIDIAGONALMATRIX],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ANTIDIAGONALMATRIX_TO_ADMAT,
        tier: Base,
        examples: [
        {
            label: admat_sign_flip,
            packages: ["base", "physics"],
            input: r"J=\mqty(\antidiagonalmatrix{1,-1})",
            expected: r"J=\mqty(\admat{1,-1})",
        },
        {
            label: admat_bare_no_elements,
            packages: ["base", "physics"],
            input: r"J_0=\mqty(\antidiagonalmatrix)",
            expected: r"J_0=\mqty(\admat)",
        },
        ]
    }
    // END: Generated examples
}
