//! Collapse the amsmath capital accent \Ddot to its lower-case alias \ddot.
//!
//! ```yaml
//! proposal: Ddot-to-ddot
//! triggers:
//!   - cmd:Ddot
//! consumes:
//!   eliminates: cmd:Ddot
//!   touches: null
//! produces: cmd:ddot
//! rewrite_patterns:
//!   - {from: '\Ddot{#1}', to: '\ddot{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static DDOT_TO_DDOT: DdotToDdotRule {
        key: Ams / "Ddot-to-ddot",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Ddot to its lower-case alias \\ddot.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::DDOT,
        aliases: [&ams::cmd::DDOT],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DDOT_TO_DDOT,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Ddot{x}",
            expected: r"\ddot{x}",
        },
        ]
    }
    // END: Generated examples
}
