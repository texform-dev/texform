//! Collapse the long physics smallmatrixquantity name to smqty.
//!
//! ```yaml
//! proposal: smallmatrixquantity-to-smqty
//! triggers:
//!   - cmd:smallmatrixquantity
//! consumes:
//!   eliminates: cmd:smallmatrixquantity
//!   touches: null
//! produces: cmd:smqty
//! rewrite_patterns:
//!   - {label: absent, from: \smallmatrixquantity, to: \smqty}
//!   - {label: absent-star, from: \smallmatrixquantity*, to: \smqty*}
//!   - {label: brace, from: '\smallmatrixquantity{#1}', to: '\smqty{#1}'}
//!   - {label: brace-star, from: '\smallmatrixquantity*{#1}', to: '\smqty*{#1}'}
//!   - {label: paren, from: \smallmatrixquantity(#1), to: \smqty(#1)}
//!   - {label: paren-star, from: \smallmatrixquantity*(#1), to: \smqty*(#1)}
//!   - {label: bracket, from: '\smallmatrixquantity[#1]', to: '\smqty[#1]'}
//!   - {label: bracket-star, from: '\smallmatrixquantity*[#1]', to: '\smqty*[#1]'}
//!   - {label: bar, from: \smallmatrixquantity|#1|, to: \smqty|#1|}
//!   - {label: bar-star, from: \smallmatrixquantity*|#1|, to: \smqty*|#1|}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static SMALLMATRIXQUANTITY_TO_SMQTY: SmallmatrixquantityToSmqtyRule {
        key: Physics / "smallmatrixquantity-to-smqty",
        level: Authoring,
        summary: "Collapse the long physics smallmatrixquantity name to smqty.",
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::SMQTY,
        aliases: [&physics::cmd::SMALLMATRIXQUANTITY],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: SMALLMATRIXQUANTITY_TO_SMQTY,
        level: Authoring,
        examples: [
        {
            label: absent,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity",
            expected: r"A=\smqty",
        },
        {
            label: absent_star,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity*",
            expected: r"A=\smqty*",
        },
        {
            label: brace,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity{a&b\\c&d}",
            expected: r"A=\smqty{a&b\\c&d}",
        },
        {
            label: brace_star,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity*{a&b\\c&d}",
            expected: r"A=\smqty*{a&b\\c&d}",
        },
        {
            label: paren,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity(a&b\\c&d)",
            expected: r"A=\smqty(a&b\\c&d)",
        },
        {
            label: paren_star,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity*(a&b\\c&d)",
            expected: r"A=\smqty*(a&b\\c&d)",
        },
        {
            label: bracket,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity[a&b\\c&d]",
            expected: r"A=\smqty[a&b\\c&d]",
        },
        {
            label: bracket_star,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity*[a&b\\c&d]",
            expected: r"A=\smqty*[a&b\\c&d]",
        },
        {
            label: bar,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity|a&b\\c&d|",
            expected: r"A=\smqty|a&b\\c&d|",
        },
        {
            label: bar_star,
            packages: ["base", "physics"],
            input: r"A=\smallmatrixquantity*|a&b\\c&d|",
            expected: r"A=\smqty*|a&b\\c&d|",
        },
        ]
    }
    // END: Generated examples
}
