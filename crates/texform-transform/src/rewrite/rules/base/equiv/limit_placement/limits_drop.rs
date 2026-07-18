//! Drop explicit limits from an audited operator while preserving attached scripts.
//!
//! ```yaml
//! proposal: limits-drop
//! triggers:
//!   - cmd:limits
//! consumes:
//!   eliminates: null
//!   touches: cmd:limits
//! produces: null
//! rewrite_patterns:
//!   - {label: bare, from: '#1\limits', to: '#1'}
//!   - {label: scripted, from: '#1\limits_{#2}^{#3}', to: '#1_{#2}^{#3}'}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_limit_modifier;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static LIMITS_DROP: LimitsDropRule {
        key: Base / "limits-drop",
        level: Equiv,
        summary: "Drop explicit limits from an audited operator while preserving attached scripts.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::LIMITS],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::LIMITS],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(_rule, cx, node_id) {
            Ok(drop_limit_modifier(cx, node_id, &base::cmd::LIMITS))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: LIMITS_DROP,
        level: Equiv,
        examples: [
        {
            label: base_bare,
            packages: ["base"],
            input: r"\sin\limits",
            expected: r"\sin",
        },
        {
            label: base_scripted,
            packages: ["base"],
            input: r"\sum\limits_{i=1}^{n}",
            expected: r"\sum_{i=1}^{n}",
        },
        {
            label: base_collision,
            packages: ["base"],
            input: r"\det\limits",
            expected: r"\det",
        },
        {
            label: physics_owner_preserved,
            packages: ["base", "physics"],
            input: r"\det\limits_x",
            expected: r"\det\limits_x",
        },
        {
            label: ams_scripted,
            packages: ["base", "ams"],
            input: r"\varlimsup\limits_x^y",
            expected: r"\varlimsup_x^y",
        },
        {
            label: starred_operatorname,
            packages: ["base", "ams"],
            input: r"\operatorname*{foo}\limits_x",
            expected: r"\operatorname*{foo}_x",
        },
        {
            label: ams_character,
            packages: ["base", "ams"],
            input: r"\iiiint\limits^1",
            expected: r"\iiiint^1",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: LIMITS_DROP,
        level: Equiv,
        examples: [
        {
            label: preserves_isolated_modifier,
            packages: ["base"],
            input: r"\limits_x",
            expected: r"\limits_x",
        },
        {
            label: preserves_unknown_predecessor,
            packages: ["base"],
            input: r"x\limits_x",
            expected: r"x\limits_x",
        },
        {
            label: preserves_cross_group_operator,
            packages: ["base"],
            input: r"{\sum}\limits_x",
            expected: r"{\sum}\limits_x",
        },
        {
            label: preserves_already_scripted_operator,
            packages: ["base"],
            input: r"\sum_i\limits_j",
            expected: r"\sum_i\limits_j",
        },
        {
            label: preserves_unstarred_operatorname,
            packages: ["base", "ams"],
            input: r"\operatorname{foo}\limits_x",
            expected: r"\operatorname{foo}\limits_x",
        },
        {
            label: preserves_physics_function_override,
            packages: ["base", "physics"],
            input: r"\sin\limits_x",
            expected: r"\sin\limits_x",
        },
        {
            label: preserves_physics_collision_family,
            packages: ["base", "physics"],
            input: r"\exp\limits_x+\Pr\limits_y",
            expected: r"\exp\limits_x+\Pr\limits_y",
        },
        ]
    }

}
