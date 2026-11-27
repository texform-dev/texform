//! Collapse the amsmath capital accent \Dot to its lower-case alias \dot.
//!
//! ```yaml
//! proposal: Dot-to-dot
//! triggers:
//!   - cmd:Dot
//! consumes:
//!   eliminates: cmd:Dot
//!   touches: null
//! produces: cmd:dot
//! rewrite_patterns:
//!   - {from: '\Dot{#1}', to: '\dot{#1}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static DOT_TO_DOT: DotToDotRule {
        key: Ams / "Dot-to-dot",
        level: Authoring,
        summary: "Collapse the amsmath capital accent \\Dot to its lower-case alias \\dot.",
        enabled_by_packages: [Ams],
        canonical: &base::cmd::DOT,
        aliases: [&ams::cmd::DOT],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DOT_TO_DOT,
        level: Authoring,
        examples: [
        {
            label: single_accent,
            packages: ["base", "ams"],
            input: r"\Dot{x}",
            expected: r"\dot{x}",
        },
        ]
    }
    // END: Generated examples
}
