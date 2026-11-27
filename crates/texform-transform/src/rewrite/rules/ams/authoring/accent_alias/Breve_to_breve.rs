//! Collapse the amsmath capital accent \Breve to its lower-case alias \breve.
//!
//! ```yaml
//! proposal: Breve-to-breve
//! triggers:
//!   - cmd:Breve
//! consumes:
//!   eliminates: cmd:Breve
//!   touches: null
//! produces: cmd:breve
//! rewrite_patterns:
//!   - {from: '\Breve{#1}', to: '\breve{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static BREVE_TO_BREVE: BreveToBreveRule {
        key: Ams / "Breve-to-breve",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Breve to its lower-case alias \\breve.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::BREVE,
        aliases: [&ams::cmd::BREVE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BREVE_TO_BREVE,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Breve{x}",
            expected: r"\breve{x}",
        },
        ]
    }
    // END: Generated examples
}
