//! Drop stretchiness from \widehat by rewriting it to \hat.
//!
//! ```yaml
//! proposal: widehat-to-hat
//! triggers:
//!   - cmd:widehat
//! consumes:
//!   eliminates: cmd:widehat
//!   touches: null
//! produces: cmd:hat
//! rewrite_patterns:
//!   - {from: '\widehat{#1}', to: '\hat{#1}'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static WIDEHAT_TO_HAT: WidehatToHatRule {
        key: Base / "widehat-to-hat",
        level: Equiv,
        summary: "Drop stretchiness from \\widehat by rewriting it to \\hat.",
        fidelity: Math,
        enabled_by_packages: [Base],
        canonical: &base::cmd::HAT,
        aliases: [
            &base::cmd::WIDEHAT,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: WIDEHAT_TO_HAT,
        level: Equiv,
        examples: [
        {
            label: wide_accent_size_drop,
            packages: ["base"],
            input: r"\widehat{AB}",
            expected: r"\hat{AB}",
        },
        ]
    }
    // END: Generated examples
}
