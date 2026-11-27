//! Collapse the amsmath capital accent \Tilde to its lower-case alias \tilde.
//!
//! ```yaml
//! proposal: Tilde-to-tilde
//! triggers:
//!   - cmd:Tilde
//! consumes:
//!   eliminates: cmd:Tilde
//!   touches: null
//! produces: cmd:tilde
//! rewrite_patterns:
//!   - {from: '\Tilde{#1}', to: '\tilde{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static TILDE_TO_TILDE: TildeToTildeRule {
        key: Ams / "Tilde-to-tilde",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Tilde to its lower-case alias \\tilde.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::TILDE,
        aliases: [&ams::cmd::TILDE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: TILDE_TO_TILDE,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Tilde{x}",
            expected: r"\tilde{x}",
        },
        {
            label: nested_accent,
            packages: ["base", "ams"],
            input: r"\Tilde{\Tilde{A}}",
            expected: r"\tilde{\tilde{A}}",
        },
        ]
    }
    // END: Generated examples
}
