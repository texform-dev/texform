//! Collapse the long cosine helper to the standard cos operator.
//!
//! ```yaml
//! proposal: cosine-to-cos
//! triggers:
//!   - cmd:cosine
//! consumes:
//!   eliminates: cmd:cosine
//!   touches: null
//! produces: cmd:cos
//! rewrite_patterns:
//!   - {from: \cosine, to: \cos}
//! ```

use texform_knowledge::builtin::base;
use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static COSINE_TO_COS: CosineToCosRule {
        key: Physics / "cosine-to-cos",
        level: Standard,
        summary: "Collapse the long cosine helper to the standard cos operator.",
        fidelity: Full,
        enabled_by_packages: [Physics],
        canonical: &base::cmd::COS,
        aliases: [&physics::cmd::COSINE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: COSINE_TO_COS,
        level: Standard,
        examples: [
        {
            label: cosine_oscillation,
            packages: ["base", "physics"],
            input: r"E(t)=E_0\cosine(\omega t)",
            expected: r"E(t)=E_0\cos(\omega t)",
        },
        {
            label: cosine_power_identity,
            packages: ["base", "physics"],
            input: r"\cosine^2\theta+\sin^2\theta=1",
            expected: r"\cos^2\theta+\sin^2\theta=1",
        },
        ]
    }
    // END: Generated examples
}
