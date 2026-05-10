//! Collapse vectorbold to the short vb helper.
//!
//! ```yaml
//! proposal: vectorbold-to-vb
//! triggers:
//!   - cmd:vectorbold
//! consumes:
//!   eliminates: cmd:vectorbold
//!   touches: null
//! produces: cmd:vb
//! rewrite_patterns:
//!   - {label: vectorbold, from: '\vectorbold{#1}', to: '\vb{#1}'}
//!   - {label: vectorbold-star, from: '\vectorbold*{#1}', to: '\vb*{#1}'}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    pub static VECTORBOLD_TO_VB: VectorboldToVbRule {
        key: Physics / "vectorbold-to-vb",
        class: Standard,
        summary: "Collapse vectorbold to the short vb helper.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::VB,
        aliases: [&physics::cmd::VECTORBOLD],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: VECTORBOLD_TO_VB,
        class: Standard,
        examples: [
        {
            label: vectorbold_newton_law,
            packages: ["base", "physics"],
            input: r"\vectorbold{F}=m\vb{a}",
            expected: r"\vb{F}=m\vb{a}",
        },
        {
            label: vectorbold_star_bold_italic,
            packages: ["base", "physics"],
            input: r"\vectorbold*{\alpha}+\vectorbold*{p}",
            expected: r"\vb*{\alpha}+\vb*{p}",
        },
        ]
    }
    // END: Generated examples

}
