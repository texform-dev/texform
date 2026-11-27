//! Drop displaylimits after an operator whose default limit placement is already displaylimits.
//!
//! `\displaylimits` restores TeX's default placement for an Op atom, so it is
//! a no-op after an operator whose definition already uses that placement.
//! The rule only accepts an audited whitelist of such operators directly
//! before the modifier in the same sequence; integral signs, which default to
//! `\nolimits`, belong to the Equiv `integral-displaylimits-drop` rule.
//!
//! ```yaml
//! proposal: displaylimits-drop
//! triggers:
//!   - cmd:displaylimits
//! consumes:
//!   eliminates: null
//!   touches: cmd:displaylimits
//! produces: null
//! rewrite_patterns:
//!   - {label: bare, from: '#1\displaylimits', to: '#1'}
//!   - {label: scripted, from: '#1\displaylimits_{#2}^{#3}', to: '#1_{#2}^{#3}'}
//! ```

use texform_knowledge::builtin::{ams, base};

use crate::ast::{ArgumentKind, ArgumentValue, Node, NodeId};
use crate::rewrite::helpers::{
    active_command_is_builtin, active_command_is_builtin_character, drop_postfix_modifier,
    prefix_command_record,
};
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static DISPLAYLIMITS_DROP: DisplaylimitsDropRule {
        key: Base / "displaylimits-drop",
        level: Authoring,
        summary: "Drop displaylimits after an operator whose default limit placement is already displaylimits.",
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::DISPLAYLIMITS],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::DISPLAYLIMITS],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(_rule, cx, node_id) {
            // A directly preceding `\limits` or `\nolimits` is not an operator,
            // so a modifier that overrides it is kept.
            Ok(drop_postfix_modifier(
                cx,
                node_id,
                &base::cmd::DISPLAYLIMITS,
                "base",
                has_default_displaylimits,
            ))
        }
    }
}

/// Audited operators whose LaTeX definition is a bare `\mathop` with the
/// default `\displaylimits` placement. Records redefined by another package,
/// such as the physics `\det` and `\Pr`, fail closed.
fn has_default_displaylimits(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    let Some(active) = prefix_command_record(cx, node_id) else {
        return false;
    };
    let character = match active.name {
        "sum" => Some(&base::chars::SUM),
        "prod" => Some(&base::chars::PROD),
        "coprod" => Some(&base::chars::COPROD),
        "bigcup" => Some(&base::chars::BIGCUP),
        "bigcap" => Some(&base::chars::BIGCAP),
        "bigvee" => Some(&base::chars::BIGVEE),
        "bigwedge" => Some(&base::chars::BIGWEDGE),
        "bigoplus" => Some(&base::chars::BIGOPLUS),
        "bigotimes" => Some(&base::chars::BIGOTIMES),
        "bigodot" => Some(&base::chars::BIGODOT),
        "biguplus" => Some(&base::chars::BIGUPLUS),
        "bigsqcup" => Some(&base::chars::BIGSQCUP),
        "smallint" => Some(&base::chars::SMALLINT),
        "intop" => Some(&base::chars::INTOP),
        "ointop" => Some(&base::chars::OINTOP),
        _ => None,
    };
    if let Some(record) = character {
        return active_command_is_builtin_character(cx, active, record, "base");
    }
    let (record, package, exclusive) = match active.name {
        // Only the starred form uses limits placement; plain operatorname is nolimits.
        "operatorname" => {
            let Node::Command { args, .. } = cx.ast.node(node_id) else {
                return false;
            };
            return active_command_is_builtin(active, &ams::cmd::OPERATORNAME, "ams", true)
                && matches!(
                    args.first(),
                    Some(Some(argument))
                        if argument.kind == ArgumentKind::Star
                            && argument.value == ArgumentValue::Boolean(true)
                );
        }
        "mathop" => (&base::cmd::MATHOP, "base", false),
        "lim" => (&base::cmd::LIM, "base", false),
        "liminf" => (&base::cmd::LIMINF, "base", false),
        "limsup" => (&base::cmd::LIMSUP, "base", false),
        "max" => (&base::cmd::MAX, "base", false),
        "min" => (&base::cmd::MIN, "base", false),
        "sup" => (&base::cmd::SUP, "base", false),
        "inf" => (&base::cmd::INF, "base", false),
        "gcd" => (&base::cmd::GCD, "base", false),
        "det" => (&base::cmd::DET, "base", true),
        "Pr" => (&base::cmd::PR, "base", true),
        "injlim" => (&ams::cmd::INJLIM, "ams", false),
        "projlim" => (&ams::cmd::PROJLIM, "ams", false),
        "varliminf" => (&ams::cmd::VARLIMINF, "ams", false),
        "varlimsup" => (&ams::cmd::VARLIMSUP, "ams", false),
        "varinjlim" => (&ams::cmd::VARINJLIM, "ams", false),
        "varprojlim" => (&ams::cmd::VARPROJLIM, "ams", false),
        _ => return false,
    };
    active_command_is_builtin(active, record, package, exclusive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DISPLAYLIMITS_DROP,
        level: Authoring,
        examples: [
        {
            label: bare,
            packages: ["base"],
            input: r"\sum\displaylimits a",
            expected: r"\sum a",
        },
        {
            label: scripted,
            packages: ["base"],
            input: r"\sum\displaylimits_{i=1}^{n} a_i",
            expected: r"\sum_{i=1}^{n} a_i",
        },
        {
            label: operator_name,
            packages: ["base"],
            input: r"\lim\displaylimits_{x\to 0} f",
            expected: r"\lim_{x\to 0} f",
        },
        {
            label: mathop,
            packages: ["base"],
            input: r"\mathop{E}\displaylimits_{x} f",
            expected: r"\mathop{E}_{x} f",
        },
        {
            label: starred_operatorname,
            packages: ["base", "ams"],
            input: r"\operatorname*{argmax}\displaylimits_{x} f",
            expected: r"\operatorname*{argmax}_{x} f",
        },
        {
            label: raw_intop,
            packages: ["base"],
            input: r"\intop\displaylimits_{a}^{b} f",
            expected: r"\intop_{a}^{b} f",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: DISPLAYLIMITS_DROP,
        level: Authoring,
        examples: [
        {
            label: big_operator_characters,
            packages: ["base"],
            input: r"\prod\displaylimits_i\coprod\displaylimits_i\bigcup\displaylimits_i\bigsqcup\displaylimits_i\smallint\displaylimits_a\ointop\displaylimits_a",
            expected: r"\prod_i\coprod_i\bigcup_i\bigsqcup_i\smallint_a\ointop_a",
        },
        {
            label: base_operator_names,
            packages: ["base"],
            input: r"\limsup\displaylimits_n\max\displaylimits_x\det\displaylimits_A\Pr\displaylimits_x\gcd\displaylimits_i",
            expected: r"\limsup_n\max_x\det_A\Pr_x\gcd_i",
        },
        {
            label: ams_operator_names,
            packages: ["base", "ams"],
            input: r"\varlimsup\displaylimits_n\injlim\displaylimits_i",
            expected: r"\varlimsup_n\injlim_i",
        },
        {
            label: repeated_modifiers_drop_in_turn,
            packages: ["base"],
            input: r"\sum\displaylimits\displaylimits_i",
            expected: r"\sum_i",
        },
        {
            label: overriding_modifier_after_displaylimits_remains,
            packages: ["base"],
            input: r"\sum\displaylimits\limits_i",
            expected: r"\sum\limits_i",
        },
        {
            label: preserves_intervening_limit_modifier,
            packages: ["base"],
            input: r"\sum\limits\displaylimits_i+\sum\nolimits\displaylimits_i",
            expected: r"\sum\limits\displaylimits_i+\sum\nolimits\displaylimits_i",
        },
        {
            label: preserves_integral_signs,
            packages: ["base", "ams"],
            input: r"\int\displaylimits_a+\oint\displaylimits_a+\iiiint\displaylimits_a+\idotsint\displaylimits_a",
            expected: r"\int\displaylimits_a+\oint\displaylimits_a+\iiiint\displaylimits_a+\idotsint\displaylimits_a",
        },
        {
            label: preserves_nolimits_operator_names,
            packages: ["base", "ams"],
            input: r"\sin\displaylimits_x+\log\displaylimits_x+\operatorname{foo}\displaylimits_x",
            expected: r"\sin\displaylimits_x+\log\displaylimits_x+\operatorname{foo}\displaylimits_x",
        },
        {
            label: preserves_physics_owned_operators,
            packages: ["base", "physics"],
            input: r"\det\displaylimits_A+\Pr\displaylimits_x",
            expected: r"\det\displaylimits_A+\Pr\displaylimits_x",
        },
        {
            label: preserves_isolated_modifier,
            packages: ["base"],
            input: r"\displaylimits_x+{\displaylimits}",
            expected: r"\displaylimits_x+{\displaylimits}",
        },
        {
            label: preserves_unknown_predecessor,
            packages: ["base"],
            input: r"x\displaylimits_i",
            expected: r"x\displaylimits_i",
        },
        {
            label: preserves_cross_group_operator,
            packages: ["base"],
            input: r"{\sum}\displaylimits_i",
            expected: r"{\sum}\displaylimits_i",
        },
        {
            label: preserves_already_scripted_operator,
            packages: ["base"],
            input: r"\sum_i\displaylimits^n",
            expected: r"\sum_i\displaylimits^n",
        },
        {
            label: preserves_modifier_in_single_child_slot,
            packages: ["base"],
            input: r"\sum^\displaylimits",
            expected: r"\sum^\displaylimits",
        },
        ]
    }
}
