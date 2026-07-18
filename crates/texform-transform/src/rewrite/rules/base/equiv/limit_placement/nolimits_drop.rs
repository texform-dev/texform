//! Drop explicit nolimits from an audited operator while preserving attached scripts.
//!
//! ```yaml
//! proposal: nolimits-drop
//! triggers:
//!   - cmd:nolimits
//! consumes:
//!   eliminates: null
//!   touches: cmd:nolimits
//! produces: null
//! rewrite_patterns:
//!   - {label: bare, from: '#1\nolimits', to: '#1'}
//!   - {label: scripted, from: '#1\nolimits_{#2}^{#3}', to: '#1_{#2}^{#3}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_limit_modifier;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NOLIMITS_DROP: NolimitsDropRule {
        key: Base / "nolimits-drop",
        level: Equiv,
        summary: "Drop explicit nolimits from an audited operator while preserving attached scripts.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::NOLIMITS],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::NOLIMITS],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(_rule, cx, node_id) {
            Ok(drop_limit_modifier(cx, node_id, &base::cmd::NOLIMITS))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: NOLIMITS_DROP,
        level: Equiv,
        examples: [
        {
            label: base_bare,
            packages: ["base"],
            input: r"\mathop{f}\nolimits",
            expected: r"\mathop{f}",
        },
        {
            label: base_scripted,
            packages: ["base"],
            input: r"\int\nolimits_0^1",
            expected: r"\int_0^1",
        },
        {
            label: base_collision,
            packages: ["base"],
            input: r"\det\nolimits_x",
            expected: r"\det_x",
        },
        {
            label: physics_owner_preserved,
            packages: ["base", "physics"],
            input: r"\det\nolimits_x",
            expected: r"\det\nolimits_x",
        },
        {
            label: ams_scripted,
            packages: ["base", "ams"],
            input: r"\iiiint\nolimits_0^1",
            expected: r"\iiiint_0^1",
        },
        {
            label: starred_operatorname,
            packages: ["base", "ams"],
            input: r"\operatorname*{foo}\nolimits_x",
            expected: r"\operatorname*{foo}_x",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: NOLIMITS_DROP,
        level: Equiv,
        examples: [
        {
            label: preserves_isolated_modifier,
            packages: ["base"],
            input: r"\nolimits_x",
            expected: r"\nolimits_x",
        },
        {
            label: preserves_unknown_predecessor,
            packages: ["base"],
            input: r"x\nolimits_x",
            expected: r"x\nolimits_x",
        },
        {
            label: preserves_cross_group_operator,
            packages: ["base"],
            input: r"{\int}\nolimits_x",
            expected: r"{\int}\nolimits_x",
        },
        {
            label: preserves_already_scripted_operator,
            packages: ["base"],
            input: r"\int_0\nolimits^1",
            expected: r"\int_0\nolimits^1",
        },
        {
            label: preserves_unstarred_operatorname,
            packages: ["base", "ams"],
            input: r"\operatorname{foo}\nolimits_x",
            expected: r"\operatorname{foo}\nolimits_x",
        },
        {
            label: preserves_physics_function_override,
            packages: ["base", "physics"],
            input: r"\cos\nolimits_x",
            expected: r"\cos\nolimits_x",
        },
        {
            label: preserves_physics_collision_family,
            packages: ["base", "physics"],
            input: r"\exp\nolimits_x+\Pr\nolimits_y",
            expected: r"\exp\nolimits_x+\Pr\nolimits_y",
        },
        ]
    }

}
