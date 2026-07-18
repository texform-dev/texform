//! Rewrite the AMS text-style fraction command to frac.
//!
//! ```yaml
//! proposal: tfrac-to-frac
//! triggers:
//!   - cmd:tfrac
//! consumes:
//!   eliminates: cmd:tfrac
//!   touches: null
//! produces: cmd:frac
//! rewrite_patterns:
//!   - {from: '\tfrac{#1}{#2}', to: '\frac{#1}{#2}'}
//! ```

use texform_knowledge::builtin::{ams, base};

use crate::rewrite::alias_rule;

alias_rule! {
    pub static TFRAC_TO_FRAC: TfracToFracRule {
        key: Ams / "tfrac-to-frac",
        level: Corpus,
        summary: "Rewrite the AMS text-style fraction command to frac.",
        fidelity: Reading,
        enabled_by_packages: [Ams],
        canonical: &base::cmd::FRAC,
        aliases: [
            &ams::cmd::TFRAC,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: TFRAC_TO_FRAC,
        level: Corpus,
        examples: [
        {
            label: plain,
            packages: ["base", "ams"],
            input: r"x+\tfrac{1}{2}",
            expected: r"x+\frac{1}{2}",
        },
        {
            label: display_style,
            packages: ["base", "ams"],
            input: r"\displaystyle\tfrac{1}{2}",
            expected: r"\displaystyle\frac{1}{2}",
        },
        {
            label: script,
            packages: ["base", "ams"],
            input: r"x_{\tfrac{1}{2}}",
            expected: r"x_{\frac{1}{2}}",
        },
        ]
    }
    // END: Generated examples
}
