//! Collapse the amsmath capital accent \Grave to its lower-case alias \grave.
//!
//! ```yaml
//! proposal: Grave-to-grave
//! triggers:
//!   - cmd:Grave
//! consumes:
//!   eliminates: cmd:Grave
//!   touches: null
//! produces: cmd:grave
//! rewrite_patterns:
//!   - {from: '\Grave{#1}', to: '\grave{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static GRAVE_TO_GRAVE: GraveToGraveRule {
        key: Ams / "Grave-to-grave",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Grave to its lower-case alias \\grave.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::GRAVE,
        aliases: [&ams::cmd::GRAVE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: GRAVE_TO_GRAVE,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Grave{x}",
            expected: r"\grave{x}",
        },
        ]
    }
    // END: Generated examples
}
