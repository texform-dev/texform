//! Collapse the amsmath capital accent \Check to its lower-case alias \check.
//!
//! ```yaml
//! proposal: Check-to-check
//! triggers:
//!   - cmd:Check
//! consumes:
//!   eliminates: cmd:Check
//!   touches: null
//! produces: cmd:check
//! rewrite_patterns:
//!   - {from: '\Check{#1}', to: '\check{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static CHECK_TO_CHECK: CheckToCheckRule {
        key: Ams / "Check-to-check",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Check to its lower-case alias \\check.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::CHECK,
        aliases: [&ams::cmd::CHECK],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: CHECK_TO_CHECK,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Check{x}",
            expected: r"\check{x}",
        },
        ]
    }
    // END: Generated examples
}
