//! Collapse hfilll to the canonical hfill command.
//!
//! MathJax renders these commands identically, while plain TeX assigns them
//! different stretch orders.
//!
//! ```yaml
//! proposal: hfilll-to-hfill
//! triggers:
//!   - cmd:hfilll
//! consumes:
//!   eliminates: cmd:hfilll
//!   touches: null
//! produces: cmd:hfill
//! rewrite_patterns:
//!   - {from: \hfilll, to: \hfill}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static HFILLL_TO_HFILL: HfilllToHfillRule {
        key: Base / "hfilll-to-hfill",
        level: Standard,
        summary: "Collapse hfilll to the canonical hfill command.",
        fidelity: Full,
        enabled_by_packages: [Base],
        canonical: &base::cmd::HFILL,
        aliases: [&base::cmd::HFILLL],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: HFILLL_TO_HFILL,
        level: Standard,
        examples: [
        {
            label: hfilll_inside_array_item,
            packages: ["base"],
            input: r"\begin{array}{l} x^2+1 \hfilll y^2+1 \end{array}",
            expected: r"\begin{array}{l} x^2+1 \hfill y^2+1 \end{array}",
        },
        ]
    }
    // END: Generated examples
}
