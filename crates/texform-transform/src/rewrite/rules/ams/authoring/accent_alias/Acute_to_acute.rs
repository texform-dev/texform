//! Collapse the amsmath capital accent \Acute to its lower-case alias \acute.
//!
//! ```yaml
//! proposal: Acute-to-acute
//! triggers:
//!   - cmd:Acute
//! consumes:
//!   eliminates: cmd:Acute
//!   touches: null
//! produces: cmd:acute
//! rewrite_patterns:
//!   - {from: '\Acute{#1}', to: '\acute{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static ACUTE_TO_ACUTE: AcuteToAcuteRule {
        key: Ams / "Acute-to-acute",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Acute to its lower-case alias \\acute.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::ACUTE,
        aliases: [&ams::cmd::ACUTE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ACUTE_TO_ACUTE,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Acute{x}",
            expected: r"\acute{x}",
        },
        ]
    }
    // END: Generated examples
}
