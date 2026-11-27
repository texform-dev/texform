//! Collapse the long physics xmatrix name to xmat.
//!
//! ```yaml
//! proposal: xmatrix-to-xmat
//! triggers:
//!   - cmd:xmatrix
//! consumes:
//!   eliminates: cmd:xmatrix
//!   touches: null
//! produces: cmd:xmat
//! rewrite_patterns:
//!   - {label: plain, from: '\xmatrix{#1}{#2}{#3}', to: '\xmat{#1}{#2}{#3}'}
//!   - {label: star, from: '\xmatrix*{#1}{#2}{#3}', to: '\xmat*{#1}{#2}{#3}'}
//! ```

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static XMATRIX_TO_XMAT: XmatrixToXmatRule {
        key: Physics / "xmatrix-to-xmat",
        level: Authoring,
        summary: "Collapse the long physics xmatrix name to xmat.",
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::XMAT,
        aliases: [&physics::cmd::XMATRIX],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: XMATRIX_TO_XMAT,
        level: Authoring,
        examples: [
        {
            label: plain,
            packages: ["base", "physics"],
            input: r"\mqty{\xmatrix{x}{2}{2}}",
            expected: r"\mqty{\xmat{x}{2}{2}}",
        },
        {
            label: star,
            packages: ["base", "physics"],
            input: r"\mqty{\xmatrix*{x}{2}{2}}",
            expected: r"\mqty{\xmat*{x}{2}{2}}",
        },
        ]
    }
    // END: Generated examples
}
