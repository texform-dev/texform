//! Collapse \biggl, \biggr, \biggm to \bigg.
//!
//! ```yaml
//! proposal: bigg-class-variants-to-bigg
//! triggers:
//!   - cmd:biggl
//!   - cmd:biggr
//!   - cmd:biggm
//! consumes:
//!   eliminates: [cmd:biggl, cmd:biggr, cmd:biggm]
//!   touches: null
//! produces: cmd:bigg
//! rewrite_patterns:
//!   - {label: left, from: '\biggl #1', to: '\bigg #1'}
//!   - {label: right, from: '\biggr #1', to: '\bigg #1'}
//!   - {label: middle, from: '\biggm #1', to: '\bigg #1'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static BIGG_CLASS_VARIANTS_TO_BIGG: BiggClassVariantsToBiggRule {
        key: Base / "bigg-class-variants-to-bigg",
        level: Corpus,
        summary: "Collapse \\biggl, \\biggr, \\biggm to \\bigg.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        canonical: &base::cmd::BIGG,
        aliases: [
            &base::cmd::BIGGL,
            &base::cmd::BIGGR,
            &base::cmd::BIGGM,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BIGG_CLASS_VARIANTS_TO_BIGG,
        level: Corpus,
        examples: [
        {
            label: left,
            packages: ["base"],
            input: r"\sin\biggl(x",
            expected: r"\sin\bigg(x",
        },
        {
            label: right,
            packages: ["base"],
            input: r"x+\biggr)",
            expected: r"x+\bigg)",
        },
        {
            label: middle,
            packages: ["base"],
            input: r"a\biggm|b",
            expected: r"a\bigg|b",
        },
        ]
    }
    // END: Generated examples
}
