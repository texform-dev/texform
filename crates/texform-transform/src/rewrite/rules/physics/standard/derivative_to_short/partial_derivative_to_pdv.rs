//! Collapse long partial derivative names to the pdv command.
//!
//! ```yaml
//! proposal: partial-derivative-to-pdv
//! triggers:
//!   - cmd:partialderivative
//!   - cmd:pderivative
//! consumes:
//!   eliminates: [cmd:partialderivative, cmd:pderivative]
//!   touches: null
//! produces: cmd:pdv
//! rewrite_patterns:
//!   - {label: partialderivative-two-args, from: '\partialderivative{#1}{#2}', to: '\pdv{#1}{#2}'}
//!   - {label: partialderivative-star, from: '\partialderivative*{#1}{#2}', to: '\pdv*{#1}{#2}'}
//!   - {label: partialderivative-order, from: '\partialderivative[#1]{#2}{#3}', to: '\pdv[#1]{#2}{#3}'}
//!   - {label: partialderivative-single-arg, from: '\partialderivative{#1}', to: '\pdv{#1}'}
//!   - {label: partialderivative-three-args, from: '\partialderivative{#1}{#2}{#3}', to: '\pdv{#1}{#2}{#3}'}
//!   - {label: pderivative-two-args, from: '\pderivative{#1}{#2}', to: '\pdv{#1}{#2}'}
//!   - {label: pderivative-star, from: '\pderivative*{#1}{#2}', to: '\pdv*{#1}{#2}'}
//!   - {label: pderivative-order, from: '\pderivative[#1]{#2}{#3}', to: '\pdv[#1]{#2}{#3}'}
//!   - {label: pderivative-single-arg, from: '\pderivative{#1}', to: '\pdv{#1}'}
//!   - {label: pderivative-three-args, from: '\pderivative{#1}{#2}{#3}', to: '\pdv{#1}{#2}{#3}'}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static PARTIAL_DERIVATIVE_TO_PDV: PartialDerivativeToPdvRule {
        key: Physics / "partial-derivative-to-pdv",
        class: Standard,
        summary: "Collapse long partial derivative names to the pdv command.",
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::PDV,
        aliases: [
            &physics::cmd::PARTIALDERIVATIVE,
            &physics::cmd::PDERIVATIVE,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PARTIAL_DERIVATIVE_TO_PDV,
        class: Standard,
        examples: [
        {
            label: pdv_alias_jacobian_entry,
            packages: ["base", "physics"],
            input: r"J=\pderivative{f}{x}+\pderivative{g}{y}",
            expected: r"J=\pdv{f}{x}+\pdv{g}{y}",
        },
        {
            label: partialderivative_long_alias,
            packages: ["base", "physics"],
            input: r"\partialderivative{T}{z}=0",
            expected: r"\pdv{T}{z}=0",
        },
        {
            label: partialderivative_star_alias,
            packages: ["base", "physics"],
            input: r"\partialderivative*{f}{x}",
            expected: r"\pdv*{f}{x}",
        },
        {
            label: partialderivative_order_alias,
            packages: ["base", "physics"],
            input: r"\partialderivative[n]{f}{x}",
            expected: r"\pdv[n]{f}{x}",
        },
        {
            label: partialderivative_single_arg_alias,
            packages: ["base", "physics"],
            input: r"\partialderivative{x} f(x,y)",
            expected: r"\pdv{x} f(x,y)",
        },
        {
            label: partialderivative_mixed_alias,
            packages: ["base", "physics"],
            input: r"\partialderivative{f}{x}{y}",
            expected: r"\pdv{f}{x}{y}",
        },
        {
            label: pderivative_star_alias,
            packages: ["base", "physics"],
            input: r"\pderivative*{f}{x}",
            expected: r"\pdv*{f}{x}",
        },
        {
            label: pderivative_order_alias,
            packages: ["base", "physics"],
            input: r"\pderivative[n]{f}{x}",
            expected: r"\pdv[n]{f}{x}",
        },
        {
            label: pderivative_single_arg_alias,
            packages: ["base", "physics"],
            input: r"\pderivative{x} f(x,y)",
            expected: r"\pdv{x} f(x,y)",
        },
        {
            label: pderivative_mixed_alias,
            packages: ["base", "physics"],
            input: r"\pderivative{f}{x}{y}",
            expected: r"\pdv{f}{x}{y}",
        },
        ]
    }
    // END: Generated examples

}
