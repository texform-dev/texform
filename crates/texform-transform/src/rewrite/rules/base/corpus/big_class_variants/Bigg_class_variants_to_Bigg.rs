//! Collapse \Biggl, \Biggr, \Biggm to \Bigg.
//!
//! ```yaml
//! proposal: Bigg-class-variants-to-Bigg
//! triggers:
//!   - cmd:Biggl
//!   - cmd:Biggr
//!   - cmd:Biggm
//! consumes:
//!   eliminates: [cmd:Biggl, cmd:Biggr, cmd:Biggm]
//!   touches: null
//! produces: cmd:Bigg
//! rewrite_patterns:
//!   - {label: left, from: '\Biggl #1', to: '\Bigg #1'}
//!   - {label: right, from: '\Biggr #1', to: '\Bigg #1'}
//!   - {label: middle, from: '\Biggm #1', to: '\Bigg #1'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static BIGG_CLASS_VARIANTS_TO_BIGG: BiggClassVariantsToBiggRule {
        key: Base / "Bigg-class-variants-to-Bigg",
        level: Corpus,
        summary: "Collapse \\Biggl, \\Biggr, \\Biggm to \\Bigg.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        canonical: &base::cmd::BIGG_2,
        aliases: [
            &base::cmd::BIGGL_2,
            &base::cmd::BIGGR_2,
            &base::cmd::BIGGM_2,
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
            input: r"\sin\Biggl(x",
            expected: r"\sin\Bigg(x",
        },
        {
            label: right,
            packages: ["base"],
            input: r"x+\Biggr)",
            expected: r"x+\Bigg)",
        },
        {
            label: middle,
            packages: ["base"],
            input: r"a\Biggm|b",
            expected: r"a\Bigg|b",
        },
        ]
    }
    // END: Generated examples
}
