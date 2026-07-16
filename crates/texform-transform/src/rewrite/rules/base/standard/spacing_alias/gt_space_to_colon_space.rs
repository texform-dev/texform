//! Collapse \> to the canonical medium math space command \:.
//!
//! ```yaml
//! proposal: gt-space-to-colon-space
//! triggers:
//!   - cmd:>
//! consumes:
//!   eliminates: cmd:>
//!   touches: null
//! produces: 'cmd::'
//! rewrite_patterns:
//!   - {from: \>, to: '\:'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static GT_SPACE_TO_COLON_SPACE: GtSpaceToColonSpaceRule {
        key: Base / "gt-space-to-colon-space",
        level: Standard,
        summary: "Collapse \\> to the canonical medium math space command \\:",
        fidelity: Full,
        enabled_by_packages: [Base],
        canonical: &base::cmd::_COLON,
        aliases: [&base::cmd::_GREATER_THAN],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: GT_SPACE_TO_COLON_SPACE,
        level: Standard,
        examples: [
        {
            label: math_gt_space,
            packages: ["base"],
            input: r"a\>b",
            expected: r"a\:b",
        },
        {
            label: text_gt_space,
            packages: ["base", "textmacros"],
            input: r"\text{A\>B}",
            expected: r"\text{A\:B}",
        },
        ]
    }
    // END: Generated examples
}
