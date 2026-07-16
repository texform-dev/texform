//! Collapse hfil to the canonical hfill command under MathJax spacing semantics.
//!
//! MathJax renders these commands identically, while plain TeX assigns them
//! different stretch orders.
//!
//! ```yaml
//! proposal: hfil-to-hfill
//! triggers:
//!   - cmd:hfil
//! consumes:
//!   eliminates: cmd:hfil
//!   touches: null
//! produces: cmd:hfill
//! rewrite_patterns:
//!   - {from: \hfil, to: \hfill}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static HFIL_TO_HFILL: HfilToHfillRule {
        key: Base / "hfil-to-hfill",
        level: Standard,
        summary: "Collapse hfil to the canonical hfill command under MathJax spacing semantics.",
        fidelity: Full,
        enabled_by_packages: [Base],
        canonical: &base::cmd::HFILL,
        aliases: [&base::cmd::HFIL],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: HFIL_TO_HFILL,
        level: Standard,
        examples: [
        {
            label: hfil_inside_array_item,
            packages: ["base"],
            input: r"\begin{array}{l}A\hfil B\end{array}",
            expected: r"\begin{array}{l}A\hfill B\end{array}",
        },
        ]
    }
    // END: Generated examples
}
