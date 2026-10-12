//! Collapse a bare Trace with no following argument group to the local Tr anchor.
//!
//! ```yaml
//! proposal: trace-capital-to-Tr
//! triggers:
//!   - cmd:Trace
//! consumes:
//!   eliminates: null
//!   touches: cmd:Trace
//! produces: cmd:Tr
//! rewrite_patterns:
//!   - {label: bare, from: \Trace, to: \Tr}
//! ```
//!
//! In LaTeX physics.sty, `\Trace` is a plain `\DeclareMathOperator`, while
//! `\Tr` is `\opbraces{\Trace}` and turns a following `{...}`, `[...]`, or
//! `(...)` into an auto-sized fence. Only the bare form is equivalent; the
//! other branches are left to `physics/named-fn-to-operatorname`. MathJax
//! treats both names as the same `Expression`, so it renders every branch
//! alike.

use texform_knowledge::builtin::physics;

use crate::rewrite::helpers::{followed_by_paren_bracket_or_group, prefix_command_node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static TRACE_CAPITAL_TO_TR: TraceCapitalToTrRule {
        key: Physics / "trace-capital-to-Tr",
        level: Authoring,
        summary: "Collapse a bare Trace with no following argument group to the local Tr anchor.",
        fidelity: Render,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::TRACE_2],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&physics::cmd::TRACE_2],
        },
        produces: RuleProduces {
            targets: cmd_targets![&physics::cmd::TR_2],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &physics::cmd::TRACE_2) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\Trace")?;
            if followed_by_paren_bracket_or_group(cx.ast, node_id) {
                return Ok(RuleEffect::Skipped);
            }

            cx.ast.replace_node_drop_detached_children(
                node_id,
                prefix_command_node(&physics::cmd::TR_2, Vec::new()),
            );
            Ok(RuleEffect::Applied)
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
        rule: TRACE_CAPITAL_TO_TR,
        level: Authoring,
        examples: [
        {
            label: bare,
            packages: ["base", "physics"],
            input: r"\Trace\rho=1",
            expected: r"\Tr\rho=1",
        },
        {
            label: scripted_bare,
            packages: ["base", "physics"],
            input: r"\Trace_A\rho",
            expected: r"\Tr_A\rho",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &TRACE_CAPITAL_TO_TR, RuleLevel::Authoring)
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
        // `\Tr` would turn each of these into an auto-sized fence.
        assert_eq!(rewrite(r"\Trace(\frac{x}{y})"), canonical(r"\Trace(\frac{x}{y})"));
        assert_eq!(rewrite(r"\Trace{\frac{x}{y}}"), canonical(r"\Trace{\frac{x}{y}}"));
        assert_eq!(rewrite(r"\Trace[\frac{x}{y}]"), canonical(r"\Trace[\frac{x}{y}]"));
    }
}
