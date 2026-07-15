//! Collapse \bigl, \bigr, \bigm to \big.
//!
//! ```yaml
//! proposal: big-class-variants-to-big
//! triggers:
//!   - cmd:bigl
//!   - cmd:bigr
//!   - cmd:bigm
//! consumes:
//!   eliminates: [cmd:bigl, cmd:bigr, cmd:bigm]
//!   touches: null
//! produces: cmd:big
//! rewrite_patterns:
//!   - {label: left, from: '\bigl #1', to: '\big #1'}
//!   - {label: right, from: '\bigr #1', to: '\big #1'}
//!   - {label: middle, from: '\bigm #1', to: '\big #1'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static BIG_CLASS_VARIANTS_TO_BIG: BigClassVariantsToBigRule {
        key: Base / "big-class-variants-to-big",
        level: Equiv,
        summary: "Collapse \\bigl, \\bigr, \\bigm to \\big.",
        fidelity: Approximate,
        enabled_by_packages: [Base],
        canonical: &base::cmd::BIG,
        aliases: [
            &base::cmd::BIGL,
            &base::cmd::BIGR,
            &base::cmd::BIGM,
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
            input: r"\sin\bigl(x",
            expected: r"\sin\big(x",
        },
        {
            label: right,
            packages: ["base"],
            input: r"x+\bigr)",
            expected: r"x+\big)",
        },
        {
            label: middle,
            packages: ["base"],
            input: r"a\bigm|b",
            expected: r"a\big|b",
        },
        ]
    }
    // END: Generated examples
}
