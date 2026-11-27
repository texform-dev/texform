//! Collapse the amsmath capital accent \Hat to its lower-case alias \hat.
//!
//! ```yaml
//! proposal: Hat-to-hat
//! triggers:
//!   - cmd:Hat
//! consumes:
//!   eliminates: cmd:Hat
//!   touches: null
//! produces: cmd:hat
//! rewrite_patterns:
//!   - {from: '\Hat{#1}', to: '\hat{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static HAT_TO_HAT: HatToHatRule {
        key: Ams / "Hat-to-hat",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Hat to its lower-case alias \\hat.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::HAT,
        aliases: [&ams::cmd::HAT],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: HAT_TO_HAT,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Hat{x}",
            expected: r"\hat{x}",
        },
        ]
    }
    // END: Generated examples
}
