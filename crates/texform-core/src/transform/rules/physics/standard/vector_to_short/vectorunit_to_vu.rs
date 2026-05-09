//! Collapse vectorunit to the short vu helper.
//!
//! ```yaml
//! proposal: vectorunit-to-vu
//! consumes:
//!   eliminates: cmd:vectorunit
//!   touches: null
//! produces: cmd:vu
//! rewrite_patterns:
//!   - {label: vectorunit, from: '\vectorunit{#1}', to: '\vu{#1}'}
//!   - {label: vectorunit-star, from: '\vectorunit*{#1}', to: '\vu*{#1}'}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse vectorunit to the short vu helper.
    pub static VECTORUNIT_TO_VU: VectorunitToVuRule {
        key: Physics / "vectorunit-to-vu",
        class: Standard,
        summary: "Collapse vectorunit to the short vu helper.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::VU,
        aliases: [&physics::cmd::VECTORUNIT],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: VECTORUNIT_TO_VU,
        class: Standard,
        examples: [
        {
            label: vectorunit_radial_basis,
            packages: ["base", "physics"],
            input: r"\vectorunit{e}_r\cdot\vb{r}=r",
            expected: r"\vu{e}_r\cdot\vb{r}=r",
        },
        {
            label: vectorunit_star_bold_italic,
            packages: ["base", "physics"],
            input: r"\vectorunit*{\alpha}\cdot\vectorunit*{p}",
            expected: r"\vu*{\alpha}\cdot\vu*{p}",
        },
        ]
    }
    // END: Generated examples

}
