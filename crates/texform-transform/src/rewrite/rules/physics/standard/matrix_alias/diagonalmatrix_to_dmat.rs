//! Collapse diagonalmatrix to the shorter dmat helper.
//!
//! ```yaml
//! proposal: diagonalmatrix-to-dmat
//! triggers:
//!   - cmd:diagonalmatrix
//! consumes:
//!   eliminates: cmd:diagonalmatrix
//!   touches: null
//! produces: cmd:dmat
//! rewrite_patterns:
//!   - {label: diagonalmatrix-braced, from: '\diagonalmatrix{#1}', to: '\dmat{#1}'}
//!   - {label: diagonalmatrix-bare, from: \diagonalmatrix, to: \dmat}
//! ```

use texform_specs::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static DIAGONALMATRIX_TO_DMAT: DiagonalmatrixToDmatRule {
        key: Physics / "diagonalmatrix-to-dmat",
        class: Standard,
        summary: "Collapse diagonalmatrix to the shorter dmat helper.",
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::DMAT,
        aliases: [&physics::cmd::DIAGONALMATRIX],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DIAGONALMATRIX_TO_DMAT,
        class: Standard,
        examples: [
        {
            label: dmat_diagonal_spectrum,
            packages: ["base", "physics"],
            input: r"D=\mqty(\diagonalmatrix{\lambda_1,\lambda_2,\lambda_3})",
            expected: r"D=\mqty(\dmat{\lambda_1,\lambda_2,\lambda_3})",
        },
        {
            label: dmat_bare_no_elements,
            packages: ["base", "physics"],
            input: r"D_0=\mqty(\diagonalmatrix)",
            expected: r"D_0=\mqty(\dmat)",
        },
        ]
    }
    // END: Generated examples
}
