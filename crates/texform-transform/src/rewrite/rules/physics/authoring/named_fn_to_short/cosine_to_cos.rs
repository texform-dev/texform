//! Collapse a bare cosine helper to the standard cos operator.
//!
//! ```yaml
//! proposal: cosine-to-cos
//! triggers:
//!   - cmd:cosine
//! consumes:
//!   eliminates: null
//!   touches: cmd:cosine
//! produces: cmd:cos
//! rewrite_patterns:
//!   - {label: bare, from: \cosine, to: \cos}
//! ```
//!
//! The produced command uses the physics record, which overrides the base
//! definition whenever this rule is enabled.
//!
//! In LaTeX physics.sty, `\cosine` is the original `\cos` saved with `\let`, a
//! plain operator that grabs nothing, while the redefined `\cos` grabs a
//! following `[...]` power and `(...)` argument. The rule renames only a bare
//! `\cosine`, one not followed by `(`, `[`, or a brace group; scripts on the
//! command keep it bare. The other branches are left to
//! `physics/named-fn-to-operatorname`.

use texform_knowledge::builtin::physics;

use super::helpers::rename_bare_named_function;
use crate::rewrite::rule::{RuleConsumes, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static COSINE_TO_COS: CosineToCosRule {
        key: Physics / "cosine-to-cos",
        level: Authoring,
        summary: "Collapse a bare cosine helper to the standard cos operator.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::COSINE],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&physics::cmd::COSINE],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::COS],
        },
        apply(_rule, cx, node_id) {
            rename_bare_named_function(
                cx,
                Self::KEY,
                node_id,
                &physics::cmd::COSINE,
                &physics::cmd::COS,
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
        rule: COSINE_TO_COS,
        level: Authoring,
        examples: [
        {
            label: cosine_oscillation,
            packages: ["base", "physics"],
            input: r"E(t)=E_0\cosine\omega t",
            expected: r"E(t)=E_0\cos\omega t",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &COSINE_TO_COS, RuleLevel::Authoring)
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
        // `\cos` would grab a following argument that `\cosine` leaves literal.
        assert_eq!(rewrite(r"\cosine(\frac{x}{y})"), canonical(r"\cosine(\frac{x}{y})"));
        assert_eq!(rewrite(r"\cosine[2](x)"), canonical(r"\cosine[2](x)"));
        assert_eq!(rewrite(r"\cosine{x}"), canonical(r"\cosine{x}"));
    }
}
