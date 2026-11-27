//! Collapse the amsmath capital accent \Bar to its lower-case alias \bar.
//!
//! ```yaml
//! proposal: Bar-to-bar
//! triggers:
//!   - cmd:Bar
//! consumes:
//!   eliminates: cmd:Bar
//!   touches: null
//! produces: cmd:bar
//! rewrite_patterns:
//!   - {from: '\Bar{#1}', to: '\bar{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static BAR_TO_BAR: BarToBarRule {
        key: Ams / "Bar-to-bar",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Bar to its lower-case alias \\bar.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::BAR,
        aliases: [&ams::cmd::BAR],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BAR_TO_BAR,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Bar{x}",
            expected: r"\bar{x}",
        },
        ]
    }
    // END: Generated examples
}
