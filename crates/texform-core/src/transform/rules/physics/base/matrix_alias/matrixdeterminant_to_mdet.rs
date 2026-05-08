//! Collapse matrixdeterminant to the shorter mdet determinant helper.
//!
//! ```yaml
//! proposal: matrixdeterminant-to-mdet
//! consumes:
//!   eliminates: cmd:matrixdeterminant
//!   touches: null
//! produces: cmd:mdet
//! rewrite_patterns:
//!   - {from: '\matrixdeterminant{#1}', to: '\mdet{#1}'}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse matrixdeterminant to the shorter mdet determinant helper.
    pub static MATRIXDETERMINANT_TO_MDET: MatrixdeterminantToMdetRule {
        key: Physics / "matrixdeterminant-to-mdet",
        tier: Base,
        summary: "Collapse matrixdeterminant to the shorter mdet determinant helper.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::MDET,
        aliases: [&physics::cmd::MATRIXDETERMINANT],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: MATRIXDETERMINANT_TO_MDET,
        tier: Base,
        examples: [
        {
            label: matrixdeterminant_characteristic_polynomial,
            packages: ["base", "physics"],
            input: r"p(\lambda)=\matrixdeterminant{\lambda I-A}",
            expected: r"p(\lambda)=\mdet{\lambda I-A}",
        },
        ]
    }
    // END: Generated examples

}
