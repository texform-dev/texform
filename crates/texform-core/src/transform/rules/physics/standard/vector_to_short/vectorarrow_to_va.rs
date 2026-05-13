//! Collapse vectorarrow to the short va helper.
//!
//! ```yaml
//! proposal: vectorarrow-to-va
//! triggers:
//!   - cmd:vectorarrow
//! consumes:
//!   eliminates: cmd:vectorarrow
//!   touches: null
//! produces: cmd:va
//! rewrite_patterns:
//!   - {label: vectorarrow, from: '\vectorarrow{#1}', to: '\va{#1}'}
//!   - {label: vectorarrow-star, from: '\vectorarrow*{#1}', to: '\va*{#1}'}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    pub static VECTORARROW_TO_VA: VectorarrowToVaRule {
        key: Physics / "vectorarrow-to-va",
        class: Standard,
        summary: "Collapse vectorarrow to the short va helper.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::VA,
        aliases: [&physics::cmd::VECTORARROW],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: VECTORARROW_TO_VA,
        class: Standard,
        examples: [
        {
            label: vectorarrow_trajectory,
            packages: ["base", "physics"],
            input: r"\vectorarrow{r}(t)=\vb{r}_0+t\vb{v}",
            expected: r"\va{r}(t)=\vb{r}_0+t\vb{v}",
        },
        {
            label: vectorarrow_star_bold_italic,
            packages: ["base", "physics"],
            input: r"\vectorarrow*{\alpha}+\vectorarrow*{p}",
            expected: r"\va*{\alpha}+\va*{p}",
        },
        ]
    }
    // END: Generated examples

}
