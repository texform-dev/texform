//! Rewrite the AMS display-style fraction command to frac.
//!
//! ```yaml
//! proposal: dfrac-to-frac
//! triggers:
//!   - cmd:dfrac
//! consumes:
//!   eliminates: cmd:dfrac
//!   touches: null
//! produces: cmd:frac
//! rewrite_patterns:
//!   - {from: '\dfrac{#1}{#2}', to: '\frac{#1}{#2}'}
//! ```

use texform_knowledge::builtin::{ams, base};

use crate::rewrite::alias_rule;

alias_rule! {
    pub static DFRAC_TO_FRAC: DfracToFracRule {
        key: Ams / "dfrac-to-frac",
        level: Corpus,
        summary: "Rewrite the AMS display-style fraction command to frac.",
        fidelity: Reading,
        enabled_by_packages: [Ams],
        canonical: &base::cmd::FRAC,
        aliases: [
            &ams::cmd::DFRAC,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DFRAC_TO_FRAC,
        level: Corpus,
        examples: [
        {
            label: plain,
            packages: ["base", "ams"],
            input: r"x+\dfrac{1}{2}",
            expected: r"x+\frac{1}{2}",
        },
        {
            label: script,
            packages: ["base", "ams"],
            input: r"x_{\dfrac{1}{2}}",
            expected: r"x_{\frac{1}{2}}",
        },
        ]
    }
    // END: Generated examples
}
