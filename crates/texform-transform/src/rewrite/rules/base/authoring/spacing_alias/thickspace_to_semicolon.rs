//! Collapse thickspace to the explicit thick math space command.
//!
//! ```yaml
//! proposal: thickspace-to-semicolon
//! triggers:
//!   - cmd:thickspace
//! consumes:
//!   eliminates: cmd:thickspace
//!   touches: null
//! produces: cmd:;
//! rewrite_patterns:
//!   - {from: \thickspace, to: \;}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static THICKSPACE_TO_SEMICOLON: ThickspaceToSemicolonRule {
        key: Base / "thickspace-to-semicolon",
        level: Authoring,
        summary: "Collapse thickspace to the explicit thick math space command.",
        enabled_by_packages: [Base],
        canonical: &base::cmd::_SEMICOLON,
        aliases: [&base::cmd::THICKSPACE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: THICKSPACE_TO_SEMICOLON,
        level: Authoring,
        examples: [
        {
            label: thickspace_between_atoms,
            packages: ["base"],
            input: r"a \thickspace b",
            expected: r"a \; b",
        },
        ]
    }
    // END: Generated examples
}
