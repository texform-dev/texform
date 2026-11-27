//! Collapse the long physics matrixquantity name to mqty.
//!
//! ```yaml
//! proposal: matrixquantity-to-mqty
//! triggers:
//!   - cmd:matrixquantity
//! consumes:
//!   eliminates: cmd:matrixquantity
//!   touches: null
//! produces: cmd:mqty
//! rewrite_patterns:
//!   - {label: absent, from: \matrixquantity, to: \mqty}
//!   - {label: absent-star, from: \matrixquantity*, to: \mqty*}
//!   - {label: brace, from: '\matrixquantity{#1}', to: '\mqty{#1}'}
//!   - {label: brace-star, from: '\matrixquantity*{#1}', to: '\mqty*{#1}'}
//!   - {label: paren, from: \matrixquantity(#1), to: \mqty(#1)}
//!   - {label: paren-star, from: \matrixquantity*(#1), to: \mqty*(#1)}
//!   - {label: bracket, from: '\matrixquantity[#1]', to: '\mqty[#1]'}
//!   - {label: bracket-star, from: '\matrixquantity*[#1]', to: '\mqty*[#1]'}
//!   - {label: bar, from: \matrixquantity|#1|, to: \mqty|#1|}
//!   - {label: bar-star, from: \matrixquantity*|#1|, to: \mqty*|#1|}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static MATRIXQUANTITY_TO_MQTY: MatrixquantityToMqtyRule {
        key: Physics / "matrixquantity-to-mqty",
        level: Authoring,
        summary: "Collapse the long physics matrixquantity name to mqty.",
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::MQTY,
        aliases: [&physics::cmd::MATRIXQUANTITY],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: MATRIXQUANTITY_TO_MQTY,
        level: Authoring,
        examples: [
        {
            label: absent,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity",
            expected: r"A=\mqty",
        },
        {
            label: absent_star,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity*",
            expected: r"A=\mqty*",
        },
        {
            label: brace,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity{a&b\\c&d}",
            expected: r"A=\mqty{a&b\\c&d}",
        },
        {
            label: brace_star,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity*{a&b\\c&d}",
            expected: r"A=\mqty*{a&b\\c&d}",
        },
        {
            label: paren,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity(a&b\\c&d)",
            expected: r"A=\mqty(a&b\\c&d)",
        },
        {
            label: paren_star,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity*(a&b\\c&d)",
            expected: r"A=\mqty*(a&b\\c&d)",
        },
        {
            label: bracket,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity[a&b\\c&d]",
            expected: r"A=\mqty[a&b\\c&d]",
        },
        {
            label: bracket_star,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity*[a&b\\c&d]",
            expected: r"A=\mqty*[a&b\\c&d]",
        },
        {
            label: bar,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity|a&b\\c&d|",
            expected: r"A=\mqty|a&b\\c&d|",
        },
        {
            label: bar_star,
            packages: ["base", "physics"],
            input: r"A=\matrixquantity*|a&b\\c&d|",
            expected: r"A=\mqty*|a&b\\c&d|",
        },
        ]
    }
    // END: Generated examples
}
