//! Collapse long functional derivative names to the fdv command.
//!
//! ```yaml
//! proposal: functional-derivative-to-fdv
//! triggers:
//!   - cmd:fderivative
//!   - cmd:functionalderivative
//! consumes:
//!   eliminates: [cmd:fderivative, cmd:functionalderivative]
//!   touches: null
//! produces: cmd:fdv
//! rewrite_patterns:
//!   - {label: fderivative-two-args, from: '\fderivative{#1}{#2}', to: '\fdv{#1}{#2}'}
//!   - {label: fderivative-star, from: '\fderivative*{#1}{#2}', to: '\fdv*{#1}{#2}'}
//!   - {label: fderivative-order, from: '\fderivative[#1]{#2}{#3}', to: '\fdv[#1]{#2}{#3}'}
//!   - {label: fderivative-single-arg, from: '\fderivative{#1}', to: '\fdv{#1}'}
//!   - {label: functionalderivative-two-args, from: '\functionalderivative{#1}{#2}', to: '\fdv{#1}{#2}'}
//!   - {label: functionalderivative-star, from: '\functionalderivative*{#1}{#2}', to: '\fdv*{#1}{#2}'}
//!   - {label: functionalderivative-order, from: '\functionalderivative[#1]{#2}{#3}', to: '\fdv[#1]{#2}{#3}'}
//!   - {label: functionalderivative-single-arg, from: '\functionalderivative{#1}', to: '\fdv{#1}'}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static FUNCTIONAL_DERIVATIVE_TO_FDV: FunctionalDerivativeToFdvRule {
        key: Physics / "functional-derivative-to-fdv",
        level: Standard,
        summary: "Collapse long functional derivative names to the fdv command.",
        fidelity: Full,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::FDV,
        aliases: [
            &physics::cmd::FDERIVATIVE,
            &physics::cmd::FUNCTIONALDERIVATIVE,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: FUNCTIONAL_DERIVATIVE_TO_FDV,
        level: Standard,
        examples: [
        {
            label: fdv_alias_stationary_action,
            packages: ["base", "physics"],
            input: r"\functionalderivative{S[\phi]}{\phi(x)}=0",
            expected: r"\fdv{S[\phi]}{\phi(x)}=0",
        },
        {
            label: fderivative_short_alias,
            packages: ["base", "physics"],
            input: r"\fderivative{\Gamma[J]}{J(y)}=\phi(y)",
            expected: r"\fdv{\Gamma[J]}{J(y)}=\phi(y)",
        },
        {
            label: fderivative_star_alias,
            packages: ["base", "physics"],
            input: r"\fderivative*{S}{\phi}",
            expected: r"\fdv*{S}{\phi}",
        },
        {
            label: fderivative_order_alias,
            packages: ["base", "physics"],
            input: r"\fderivative[n]{S}{\phi}",
            expected: r"\fdv[n]{S}{\phi}",
        },
        {
            label: fderivative_single_arg_alias,
            packages: ["base", "physics"],
            input: r"\fderivative{\phi} S[\phi]",
            expected: r"\fdv{\phi} S[\phi]",
        },
        {
            label: functionalderivative_star_alias,
            packages: ["base", "physics"],
            input: r"\functionalderivative*{S}{\phi}",
            expected: r"\fdv*{S}{\phi}",
        },
        {
            label: functionalderivative_order_alias,
            packages: ["base", "physics"],
            input: r"\functionalderivative[n]{S}{\phi}",
            expected: r"\fdv[n]{S}{\phi}",
        },
        {
            label: functionalderivative_single_arg_alias,
            packages: ["base", "physics"],
            input: r"\functionalderivative{\phi} S[\phi]",
            expected: r"\fdv{\phi} S[\phi]",
        },
        ]
    }
    // END: Generated examples
}
