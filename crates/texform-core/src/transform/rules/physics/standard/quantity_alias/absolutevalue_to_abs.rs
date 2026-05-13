//! Collapse absolutevalue to the shorter abs helper before fence expansion.
//!
//! ```yaml
//! proposal: absolutevalue-to-abs
//! triggers:
//!   - cmd:absolutevalue
//! consumes:
//!   eliminates: cmd:absolutevalue
//!   touches: null
//! produces: cmd:abs
//! rewrite_patterns:
//!   - {label: nonstar, from: '\absolutevalue{#1}', to: '\abs{#1}'}
//!   - {label: star, from: '\absolutevalue*{#1}', to: '\abs*{#1}'}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    pub static ABSOLUTEVALUE_TO_ABS: AbsolutevalueToAbsRule {
        key: Physics / "absolutevalue-to-abs",
        class: Standard,
        summary: "Collapse absolutevalue to the shorter abs helper before fence expansion.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::ABS,
        aliases: [&physics::cmd::ABSOLUTEVALUE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ABSOLUTEVALUE_TO_ABS,
        class: Standard,
        examples: [
        {
            label: abs_alias_power_context,
            packages: ["base", "physics"],
            input: r"\absolutevalue{z}^2=x^2+y^2",
            expected: r"\abs{z}^2=x^2+y^2",
        },
        {
            label: abs_alias_star_power_context,
            packages: ["base", "physics"],
            input: r"\absolutevalue*{z}^2=x^2+y^2",
            expected: r"\abs*{z}^2=x^2+y^2",
        },
        ]
    }
    // END: Generated examples
}
