//! Collapse a bare natural logarithm helper to the standard ln operator.
//!
//! ```yaml
//! proposal: naturallogarithm-to-ln
//! triggers:
//!   - cmd:naturallogarithm
//! consumes:
//!   eliminates: null
//!   touches: cmd:naturallogarithm
//! produces: cmd:ln
//! rewrite_patterns:
//!   - {label: bare, from: \naturallogarithm, to: \ln}
//! ```
//!
//! The produced command uses the physics record, which overrides the base
//! definition whenever this rule is enabled.
//!
//! In LaTeX physics.sty, `\naturallogarithm` is the original `\ln` saved with
//! `\let`, a plain operator that grabs nothing, while the redefined `\ln` grabs
//! a following `[...]` power and `(...)` argument. The rule renames only a bare
//! `\naturallogarithm`, one not followed by `(`, `[`, or a brace group; scripts
//! on the command keep it bare. The other branches are left to `physics/named-
//! fn-to-operatorname`.

use texform_knowledge::builtin::physics;

use super::helpers::rename_bare_named_function;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static NATURALLOGARITHM_TO_LN: NaturallogarithmToLnRule {
        key: Physics / "naturallogarithm-to-ln",
        level: Authoring,
        summary: "Collapse a bare natural logarithm helper to the standard ln operator.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::NATURALLOGARITHM],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&physics::cmd::NATURALLOGARITHM],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::LN],
        },
        apply(_rule, cx, node_id) {
            rename_bare_named_function(
                cx,
                Self::KEY,
                node_id,
                &physics::cmd::NATURALLOGARITHM,
                &physics::cmd::LN,
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
        rule: NATURALLOGARITHM_TO_LN,
        level: Authoring,
        examples: [
        {
            label: naturallogarithm_entropy,
            packages: ["base", "physics"],
            input: r"S=k_B\naturallogarithm W",
            expected: r"S=k_B\ln W",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &NATURALLOGARITHM_TO_LN, RuleLevel::Authoring)
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
        // `\ln` would grab a following argument that `\naturallogarithm` leaves literal.
        assert_eq!(rewrite(r"\naturallogarithm(\frac{x}{y})"), canonical(r"\naturallogarithm(\frac{x}{y})"));
        assert_eq!(rewrite(r"\naturallogarithm[2](x)"), canonical(r"\naturallogarithm[2](x)"));
        assert_eq!(rewrite(r"\naturallogarithm{x}"), canonical(r"\naturallogarithm{x}"));
    }
}
