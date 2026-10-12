//! Collapse a bare sine helper to the standard sin operator.
//!
//! ```yaml
//! proposal: sine-to-sin
//! triggers:
//!   - cmd:sine
//! consumes:
//!   eliminates: null
//!   touches: cmd:sine
//! produces: cmd:sin
//! rewrite_patterns:
//!   - {label: bare, from: \sine, to: \sin}
//! ```
//!
//! In LaTeX physics.sty, `\sine` is the original `\sin` saved with `\let`, a
//! plain operator that grabs nothing, while the redefined `\sin` grabs a
//! following `[...]` power and `(...)` argument. The rule renames only a bare
//! `\sine`, one not followed by `(`, `[`, or a brace group; scripts on the
//! command keep it bare. The other branches are left to
//! `physics/named-fn-to-operatorname`.

use texform_knowledge::builtin::{base, physics};

use super::helpers::rename_bare_named_function;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static SINE_TO_SIN: SineToSinRule {
        key: Physics / "sine-to-sin",
        level: Authoring,
        summary: "Collapse a bare sine helper to the standard sin operator.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::SINE],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&physics::cmd::SINE],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::SIN],
        },
        apply(_rule, cx, node_id) {
            rename_bare_named_function(
                cx,
                Self::KEY,
                node_id,
                &physics::cmd::SINE,
                &base::cmd::SIN,
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
        rule: SINE_TO_SIN,
        level: Authoring,
        examples: [
        {
            label: sine_in_trig_sum,
            packages: ["base", "physics"],
            input: r"y=\sine x+\cos x",
            expected: r"y=\sin x+\cos x",
        },
        {
            label: sine_scripted_bare,
            packages: ["base", "physics"],
            input: r"\sine^2\theta",
            expected: r"\sin^2\theta",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &SINE_TO_SIN, RuleLevel::Authoring)
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
        // `\sin` would grab a following argument that `\sine` leaves literal.
        assert_eq!(rewrite(r"\sine(\frac{x}{y})"), canonical(r"\sine(\frac{x}{y})"));
        assert_eq!(rewrite(r"\sine[2](x)"), canonical(r"\sine[2](x)"));
        assert_eq!(rewrite(r"\sine{x}"), canonical(r"\sine{x}"));
    }
}
