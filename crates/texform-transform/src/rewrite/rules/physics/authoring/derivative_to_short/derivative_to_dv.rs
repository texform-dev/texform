//! Collapse derivative to the compact dv command.
//!
//! ```yaml
//! proposal: derivative-to-dv
//! triggers:
//!   - cmd:derivative
//! consumes:
//!   eliminates: cmd:derivative
//!   touches: null
//! produces: cmd:dv
//! rewrite_patterns:
//!   - {label: derivative-two-args, from: '\derivative{#1}{#2}', to: '\dv{#1}{#2}'}
//!   - {label: derivative-star, from: '\derivative*{#1}{#2}', to: '\dv*{#1}{#2}'}
//!   - {label: derivative-order, from: '\derivative[#1]{#2}{#3}', to: '\dv[#1]{#2}{#3}'}
//!   - {label: derivative-single-arg, from: '\derivative{#1}', to: '\dv{#1}'}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static DERIVATIVE_TO_DV: DerivativeToDvRule {
        key: Physics / "derivative-to-dv",
        level: Authoring,
        summary: "Collapse derivative to the compact dv command.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::DV,
        aliases: [&physics::cmd::DERIVATIVE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DERIVATIVE_TO_DV,
        level: Authoring,
        examples: [
        {
            label: dv_alias_embedded,
            packages: ["base", "physics"],
            input: r"F=m\derivative{x}{t}",
            expected: r"F=m\dv{x}{t}",
        },
        {
            label: derivative_field_alias,
            packages: ["base", "physics"],
            input: r"\derivative{\phi}{x}=0",
            expected: r"\dv{\phi}{x}=0",
        },
        {
            label: derivative_star_alias,
            packages: ["base", "physics"],
            input: r"\derivative*{x}{t}",
            expected: r"\dv*{x}{t}",
        },
        {
            label: derivative_order_alias,
            packages: ["base", "physics"],
            input: r"\derivative[n]{x}{t}",
            expected: r"\dv[n]{x}{t}",
        },
        {
            label: derivative_single_arg_alias,
            packages: ["base", "physics"],
            input: r"\derivative{x} f(x)",
            expected: r"\dv{x} f(x)",
        },
        ]
    }
    // END: Generated examples
}
