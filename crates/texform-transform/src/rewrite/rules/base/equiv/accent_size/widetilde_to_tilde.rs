//! Drop stretchiness from \widetilde by rewriting it to \tilde.
//!
//! ```yaml
//! proposal: widetilde-to-tilde
//! triggers:
//!   - cmd:widetilde
//! consumes:
//!   eliminates: cmd:widetilde
//!   touches: null
//! produces: cmd:tilde
//! rewrite_patterns:
//!   - {from: '\widetilde{#1}', to: '\tilde{#1}'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static WIDETILDE_TO_TILDE: WidetildeToTildeRule {
        key: Base / "widetilde-to-tilde",
        level: Equiv,
        summary: "Drop stretchiness from \\widetilde by rewriting it to \\tilde.",
        fidelity: Math,
        enabled_by_packages: [Base],
        canonical: &base::cmd::TILDE,
        aliases: [
            &base::cmd::WIDETILDE,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: WIDETILDE_TO_TILDE,
        level: Equiv,
        examples: [
        {
            label: wide_accent_size_drop,
            packages: ["base"],
            input: r"\widetilde{AB}",
            expected: r"\tilde{AB}",
        },
        ]
    }
    // END: Generated examples
}
