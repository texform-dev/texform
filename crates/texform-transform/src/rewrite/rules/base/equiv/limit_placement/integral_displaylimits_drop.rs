//! Drop displaylimits after an integral sign, discarding the display-style limit placement.
//!
//! Integral signs default to `\nolimits`, so `\displaylimits` puts their
//! limits above and below in display style. Without math-style context the
//! rule cannot choose between `\limits` and `\nolimits`; it drops the modifier
//! and keeps the default placement, which is suitable only for equivalence
//! comparison.
//!
//! ```yaml
//! proposal: integral-displaylimits-drop
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

use crate::ast::NodeId;
use crate::rewrite::helpers::{
    active_command_is_builtin, active_command_is_builtin_character, drop_postfix_modifier,
    prefix_command_record,
};
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static INTEGRAL_DISPLAYLIMITS_DROP: IntegralDisplaylimitsDropRule {
        key: Base / "integral-displaylimits-drop",
        level: Equiv,
        summary: "Drop displaylimits after an integral sign, discarding the display-style limit placement.",
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
            Ok(drop_postfix_modifier(
                cx,
                node_id,
                &base::cmd::DISPLAYLIMITS,
                "base",
                is_integral_sign,
            ))
        }
    }
}

/// Audited integral signs whose default placement is `\nolimits`. Operators
/// that already default to `\displaylimits` belong to the Authoring
/// `displaylimits-drop` rule.
fn is_integral_sign(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    let Some(active) = prefix_command_record(cx, node_id) else {
        return false;
    };
    let (record, package) = match active.name {
        "idotsint" => return active_command_is_builtin(active, &ams::cmd::IDOTSINT, "ams", false),
        "int" => (&base::chars::INT, "base"),
        "iint" => (&base::chars::IINT, "base"),
        "iiint" => (&base::chars::IIINT, "base"),
        "oint" => (&base::chars::OINT, "base"),
        "oiint" => (&base::chars::OIINT, "base"),
        "oiiint" => (&base::chars::OIIINT, "base"),
        "iiiint" => (&ams::chars::IIIINT, "ams"),
        _ => return false,
    };
    active_command_is_builtin_character(cx, active, record, package)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: INTEGRAL_DISPLAYLIMITS_DROP,
        level: Equiv,
        examples: [
        {
            label: scripted,
            packages: ["base"],
            input: r"\int\displaylimits_{a}^{b} f",
            expected: r"\int_{a}^{b} f",
        },
        {
            label: bare,
            packages: ["base"],
            input: r"\oint\displaylimits",
            expected: r"\oint",
        },
        {
            label: ams_multiple_integral,
            packages: ["base", "ams"],
            input: r"\idotsint\displaylimits_{V} f",
            expected: r"\idotsint_{V} f",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: INTEGRAL_DISPLAYLIMITS_DROP,
        level: Equiv,
        examples: [
        {
            label: base_multiple_integrals,
            packages: ["base"],
            input: r"\iint\displaylimits_A+\iiint\displaylimits_V+\oiint\displaylimits_S+\oiiint\displaylimits_V",
            expected: r"\iint_A+\iiint_V+\oiint_S+\oiiint_V",
        },
        {
            label: ams_quadruple_integral,
            packages: ["base", "ams"],
            input: r"\iiiint\displaylimits_V",
            expected: r"\iiiint_V",
        },
        {
            label: preserves_default_displaylimits_operators,
            packages: ["base"],
            input: r"\sum\displaylimits_i+\intop\displaylimits_a",
            expected: r"\sum\displaylimits_i+\intop\displaylimits_a",
        },
        {
            label: preserves_nolimits_operator_names,
            packages: ["base"],
            input: r"\sin\displaylimits_x",
            expected: r"\sin\displaylimits_x",
        },
        {
            label: preserves_intervening_limit_modifier,
            packages: ["base"],
            input: r"\int\limits\displaylimits_a+\int\nolimits\displaylimits_b",
            expected: r"\int\limits\displaylimits_a+\int\nolimits\displaylimits_b",
        },
        {
            label: preserves_cross_group_operator,
            packages: ["base"],
            input: r"{\int}\displaylimits_a",
            expected: r"{\int}\displaylimits_a",
        },
        {
            label: preserves_already_scripted_operator,
            packages: ["base"],
            input: r"\int_a\displaylimits^b",
            expected: r"\int_a\displaylimits^b",
        },
        {
            label: preserves_isolated_modifier,
            packages: ["base"],
            input: r"\displaylimits_a",
            expected: r"\displaylimits_a",
        },
        ]
    }

    #[test]
    fn equiv_profile_drops_displaylimits_after_every_audited_operator() {
        use crate::parse::{ParseConfig, ParseContext};
        use crate::{BuildConfig, Profile, TransformContext};

        let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
        let engine =
            TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
                .unwrap();
        for (input, expected) in [
            (r"\int\displaylimits_a^b f", r"\int_a^b f"),
            (r"\sum\displaylimits_i a_i", r"\sum_i a_i"),
            (r"\int\limits\displaylimits_a", r"\int_a"),
            (r"\sin\displaylimits_x", r"\sin\displaylimits_x"),
        ] {
            let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
            engine.run(&mut ast, &parse_ctx).unwrap();
            ast.assert_invariants();
            let expected =
                crate::parse_to_ast_for_test(&parse_ctx, expected, &ParseConfig::STRICT);
            assert_eq!(
                crate::serialize::serialize(&ast),
                crate::serialize::serialize(&expected),
                "{input}"
            );
        }
    }
}
