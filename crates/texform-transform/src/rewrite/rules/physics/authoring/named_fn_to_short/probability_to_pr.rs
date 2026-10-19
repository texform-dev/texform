//! Collapse a bare Probability helper to the standard Pr operator.
//!
//! ```yaml
//! proposal: probability-to-pr
//! triggers:
//!   - cmd:Probability
//! consumes:
//!   eliminates: null
//!   touches: cmd:Probability
//! produces: cmd:Pr
//! rewrite_patterns:
//!   - {label: bare, from: \Probability, to: \Pr}
//! ```
//!
//! The produced command uses the physics record, which overrides the base
//! definition whenever this rule is enabled.
//!
//! In LaTeX physics.sty, `\Probability` is the original `\Pr` saved with
//! `\let`, a plain operator that grabs nothing, while the redefined `\Pr` grabs
//! a following `{...}`, `[...]`, or `(...)` argument as an auto-sized fence.
//! The rule renames only a bare `\Probability`, one not followed by `(`, `[`,
//! or a brace group; scripts on the command keep it bare. The other branches
//! are left to `physics/named-fn-to-operatorname`.

use texform_knowledge::builtin::physics;

use super::helpers::rename_bare_named_function;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static PROBABILITY_TO_PR: ProbabilityToPrRule {
        key: Physics / "probability-to-pr",
        level: Authoring,
        summary: "Collapse a bare Probability helper to the standard Pr operator.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::PROBABILITY],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&physics::cmd::PROBABILITY],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::PR],
        },
        apply(_rule, cx, node_id) {
            rename_bare_named_function(
                cx,
                Self::KEY,
                node_id,
                &physics::cmd::PROBABILITY,
                &physics::cmd::PR,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{RuleLevel, run_one_rule_for_test};
    use crate::serialize::serialize;
    use texform_core::parse::ParseConfig;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: PROBABILITY_TO_PR,
        level: Authoring,
        examples: [
        {
            label: probability_event,
            packages: ["base", "physics"],
            input: r"\Probability A=\frac{1}{2}",
            expected: r"\Pr A=\frac{1}{2}",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &PROBABILITY_TO_PR, RuleLevel::Authoring)
            .expect("rule should run");
        ast.assert_invariants();
        serialize(&ast)
    }

    fn canonical(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        serialize(&crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT))
    }

    #[test]
    fn argument_followers_preserved() {
        // `\Pr` would grab a following argument that `\Probability` leaves literal.
        assert_eq!(rewrite(r"\Probability(\frac{x}{y})"), canonical(r"\Probability(\frac{x}{y})"));
        assert_eq!(rewrite(r"\Probability[2](x)"), canonical(r"\Probability[2](x)"));
        assert_eq!(rewrite(r"\Probability{x}"), canonical(r"\Probability{x}"));
    }
}
