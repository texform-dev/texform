//! Collapse matrixdeterminant to the shorter mdet determinant helper.
//!
//! ```yaml
//! proposal: matrixdeterminant-to-mdet
//! triggers:
//!   - cmd:matrixdeterminant
//! consumes:
//!   eliminates: cmd:matrixdeterminant
//!   touches: null
//! produces: cmd:mdet
//! rewrite_patterns:
//!   - {from: '\matrixdeterminant{#1}', to: '\mdet{#1}'}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static MATRIXDETERMINANT_TO_MDET: MatrixdeterminantToMdetRule {
        key: Physics / "matrixdeterminant-to-mdet",
        level: Standard,
        summary: "Collapse matrixdeterminant to the shorter mdet determinant helper.",
        fidelity: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::MDET,
        aliases: [&physics::cmd::MATRIXDETERMINANT],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: MATRIXDETERMINANT_TO_MDET,
        level: Standard,
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
