//! Collapse the amsmath capital accent \Vec to its lower-case alias \vec.
//!
//! ```yaml
//! proposal: Vec-to-vec
//! triggers:
//!   - cmd:Vec
//! consumes:
//!   eliminates: cmd:Vec
//!   touches: null
//! produces: cmd:vec
//! rewrite_patterns:
//!   - {from: '\Vec{#1}', to: '\vec{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static VEC_TO_VEC: VecToVecRule {
        key: Ams / "Vec-to-vec",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Vec to its lower-case alias \\vec.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::VEC,
        aliases: [&ams::cmd::VEC],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: VEC_TO_VEC,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Vec{x}",
            expected: r"\vec{x}",
        },
        ]
    }
    // END: Generated examples
}
