//! Rewrite AMS display-style and text-style binomial commands to binom.
//!
//! ```yaml
//! proposal: dbinom-and-tbinom-to-binom
//! triggers:
//!   - cmd:dbinom
//!   - cmd:tbinom
//! consumes:
//!   eliminates: [cmd:dbinom, cmd:tbinom]
//!   touches: null
//! produces: cmd:binom
//! rewrite_patterns:
//!   - {label: display-style, from: '\dbinom{#1}{#2}', to: '\binom{#1}{#2}'}
//!   - {label: text-style, from: '\tbinom{#1}{#2}', to: '\binom{#1}{#2}'}
//! ```

use texform_knowledge::builtin::ams;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static DBINOM_AND_TBINOM_TO_BINOM: DbinomAndTbinomToBinomRule {
        key: Ams / "dbinom-and-tbinom-to-binom",
        level: Equiv,
        summary: "Rewrite AMS display-style and text-style binomial commands to binom.",
        fidelity: Reading,
        enabled_by_packages: [Ams],
        canonical: &ams::cmd::BINOM,
        aliases: [
            &ams::cmd::DBINOM,
            &ams::cmd::TBINOM,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DBINOM_AND_TBINOM_TO_BINOM,
        level: Equiv,
        examples: [
        {
            label: dbinom_plain,
            packages: ["base", "ams"],
            input: r"x+\dbinom{n}{k}",
            expected: r"x+\binom{n}{k}",
        },
        {
            label: dbinom_script,
            packages: ["base", "ams"],
            input: r"x_{\dbinom{n}{k}}",
            expected: r"x_{\binom{n}{k}}",
        },
        {
            label: tbinom_plain,
            packages: ["base", "ams"],
            input: r"x+\tbinom{n}{k}",
            expected: r"x+\binom{n}{k}",
        },
        {
            label: tbinom_display,
            packages: ["base", "ams"],
            input: r"\displaystyle\tbinom{n}{k}",
            expected: r"\displaystyle\binom{n}{k}",
        },
        {
            label: tbinom_script,
            packages: ["base", "ams"],
            input: r"x_{\tbinom{n}{k}}",
            expected: r"x_{\binom{n}{k}}",
        },
        ]
    }
    // END: Generated examples
}
