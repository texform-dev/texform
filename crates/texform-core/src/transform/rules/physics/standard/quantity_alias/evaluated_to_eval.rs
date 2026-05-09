//! Collapse evaluated to the shorter eval helper before eval expansion.
//!
//! ```yaml
//! proposal: evaluated-to-eval
//! triggers:
//!   - cmd:evaluated
//! consumes:
//!   eliminates: cmd:evaluated
//!   touches: null
//! produces: cmd:eval
//! rewrite_patterns:
//!   - {label: braced, from: '\evaluated{#1}#2', to: '\eval{#1}#2'}
//!   - {label: braced-star, from: '\evaluated*{#1}#2', to: '\eval*{#1}#2'}
//!   - {label: paren, from: \evaluated(#1|#2, to: \eval(#1|#2}
//!   - {label: bracket, from: '\evaluated[#1|#2', to: '\eval[#1|#2'}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse evaluated to the shorter eval helper before eval expansion.
    pub static EVALUATED_TO_EVAL: EvaluatedToEvalRule {
        key: Physics / "evaluated-to-eval",
        class: Standard,
        summary: "Collapse evaluated to the shorter eval helper before eval expansion.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::EVAL,
        aliases: [&physics::cmd::EVALUATED],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: EVALUATED_TO_EVAL,
        class: Standard,
        examples: [
        {
            label: eval_alias_named_bounds,
            packages: ["base", "physics"],
            input: r"I=\evaluated{F(x)}_{a}^{b}",
            expected: r"I=\eval{F(x)}_{a}^{b}",
        },
        {
            label: eval_alias_star,
            packages: ["base", "physics"],
            input: r"\evaluated*{F(x)}_a^b",
            expected: r"\eval*{F(x)}_a^b",
        },
        {
            label: eval_alias_paren,
            packages: ["base", "physics"],
            input: r"\evaluated(\sin x|_0^{\pi}=0",
            expected: r"\eval(\sin x|_0^{\pi}=0",
        },
        {
            label: eval_alias_bracket,
            packages: ["base", "physics"],
            input: r"\evaluated[f(x)|_{x=0}=1",
            expected: r"\eval[f(x)|_{x=0}=1",
        },
        ]
    }
    // END: Generated examples
}
