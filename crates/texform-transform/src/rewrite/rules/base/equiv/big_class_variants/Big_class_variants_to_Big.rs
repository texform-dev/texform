//! Collapse \Bigl, \Bigr, \Bigm to \Big.
//!
//! ```yaml
//! proposal: Big-class-variants-to-Big
//! triggers:
//!   - cmd:Bigl
//!   - cmd:Bigr
//!   - cmd:Bigm
//! consumes:
//!   eliminates: [cmd:Bigl, cmd:Bigr, cmd:Bigm]
//!   touches: null
//! produces: cmd:Big
//! rewrite_patterns:
//!   - {label: left, from: '\Bigl #1', to: '\Big #1'}
//!   - {label: right, from: '\Bigr #1', to: '\Big #1'}
//!   - {label: middle, from: '\Bigm #1', to: '\Big #1'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static BIG_CLASS_VARIANTS_TO_BIG: BigClassVariantsToBigRule {
        key: Base / "Big-class-variants-to-Big",
        level: Equiv,
        summary: "Collapse \\Bigl, \\Bigr, \\Bigm to \\Big.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        canonical: &base::cmd::BIG_2,
        aliases: [
            &base::cmd::BIGL_2,
            &base::cmd::BIGR_2,
            &base::cmd::BIGM_2,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BIG_CLASS_VARIANTS_TO_BIG,
        level: Equiv,
        examples: [
        {
            label: left,
            packages: ["base"],
            input: r"\sin\Bigl(x",
            expected: r"\sin\Big(x",
        },
        {
            label: right,
            packages: ["base"],
            input: r"x+\Bigr)",
            expected: r"x+\Big)",
        },
        {
            label: middle,
            packages: ["base"],
            input: r"a\Bigm|b",
            expected: r"a\Big|b",
        },
        ]
    }
    // END: Generated examples
}
