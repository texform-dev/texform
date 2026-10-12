//! Expand the physics auto-sized argument of exp, det, and Pr into an explicit fence.
//!
//! ```yaml
//! proposal: physics-operator-fence-expand
//! triggers:
//!   - cmd:exp
//!   - cmd:det
//!   - cmd:Pr
//! consumes:
//!   eliminates: null
//!   touches: [cmd:exp, cmd:det, cmd:Pr]
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: operator-brace, from: '\exp{#1}', to: '\exp\left\{#1\right\}'}
//!   - {label: operator-bracket, from: '\exp[#1]', to: '\exp\left[#1\right]'}
//!   - {label: operator-paren, from: \exp(#1), to: \exp\left(#1\right)}
//! ```
//!
//! Semantics follow LaTeX physics.sty, where these names use `\opbraces`
//! (`g o d()`): only the first `{...}`, `[...]`, or `(...)` becomes an
//! auto-sized fence, and later bracket or parenthesis groups stay literal.
//! The physics records hold that first group as one paired
//! `d<(,)><[,]><{,}>` slot, so a later group is an ordinary sibling. A command
//! that carries scripts grabs nothing and is left unchanged.
//!
//! Physics closes the fence inside `\mathopen{}\mathclose\bgroup...\egroup`,
//! so it acts as a closing atom, while an explicit `\left...\right` fence is
//! an inner atom. The explicit fence therefore adds about 1/6 em before the
//! opening delimiter and, when an ordinary atom or an opening delimiter
//! follows the fence, about another 1/6 em after it: about 1/3 em in the worst
//! case. Script styles show no difference. No compensating space is inserted.
//!
//! MathJax differs: it hides the braces of `\exp{...}` and prints `\exp[...]`
//! literally. In display style, LaTeX places the scripts of `\det_A` and
//! `\Pr_X` below the operator, while MathJax places them beside it.

use texform_knowledge::builtin::{base, physics};
use texform_knowledge::specs::BuiltinCommandRecord;

use super::helpers::{fence_argument, replace_with_fenced_operator};
use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

/// Names that physics.sty redefines through `\opbraces`.
const OPERATORS: [&BuiltinCommandRecord; 3] =
    [&physics::cmd::EXP, &physics::cmd::DET, &physics::cmd::PR];

define_rule! {
    pub static PHYSICS_OPERATOR_FENCE_EXPAND: PhysicsOperatorFenceExpandRule {
        key: Physics / "physics-operator-fence-expand",
        level: Faithful,
        summary: "Expand the physics auto-sized argument of exp, det, and Pr into an explicit fence.",
        fidelity: Reading,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::EXP, &physics::cmd::DET, &physics::cmd::PR],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&physics::cmd::EXP, &physics::cmd::DET, &physics::cmd::PR],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(_rule, cx, node_id) {
            let Some(record) = OPERATORS
                .into_iter()
                .find(|record| cx.match_command(node_id, record).is_some())
            else {
                return Ok(RuleEffect::Skipped);
            };
            if !cx
                .active_command(node_id)
                .is_some_and(|active| active.from_packages.contains(&"physics"))
            {
                return Ok(RuleEffect::Skipped);
            }
            let argument = {
                let command = cx
                    .match_command(node_id, record)
                    .expect("matched command should still match");
                let subject = command.subject();
                let scoped = cx.for_rule(Self::KEY);
                scoped.expect_arg_len(command.args, 1, &subject)?;
                fence_argument(&scoped, &command.args[0], &subject)?
            };
            let Some(argument) = argument else {
                return Ok(RuleEffect::Skipped);
            };

            let operator = cx.ast.new_node(Node::Command {
                name: record.name.to_string(),
                args: vec![None],
                known: true,
            });
            replace_with_fenced_operator(cx.ast, node_id, operator, argument);
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
        rule: PHYSICS_OPERATOR_FENCE_EXPAND,
        level: Faithful,
        examples: [
        {
            label: operator_brace,
            packages: ["base", "ams", "physics"],
            input: r"\exp{\frac{x}{y}}",
            expected: r"\exp\left\{\frac{x}{y}\right\}",
        },
        {
            label: operator_bracket_literal_paren,
            packages: ["base", "ams", "physics"],
            input: r"\exp[\frac{x}{y}](t)",
            expected: r"\exp\left[\frac{x}{y}\right](t)",
        },
        {
            label: operator_paren,
            packages: ["base", "ams", "physics"],
            input: r"\det(\frac{A}{2})",
            expected: r"\det\left(\frac{A}{2}\right)",
        },
        {
            label: operator_paren_ordinary_after,
            packages: ["base", "ams", "physics"],
            input: r"\Pr(A)B",
            expected: r"\Pr\left(A\right)B",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &PHYSICS_OPERATOR_FENCE_EXPAND, RuleLevel::Faithful)
            .expect("rule should run");
        ast.assert_invariants();
        serialize(&ast)
    }

    fn canonical(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        serialize(&crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT))
    }

    #[test]
    fn nested_brackets_matched() {
        assert_eq!(rewrite(r"\exp[a[b]\frac{c}{d}]"), canonical(r"\exp\left[a[b]\frac{c}{d}\right]"));
    }

    #[test]
    fn braced_argument_scripts_stay_on_fence() {
        assert_eq!(rewrite(r"\Pr{X}^2"), canonical(r"\Pr\left\{X\right\}^2"));
    }

    #[test]
    fn scripted_command_preserved() {
        assert_eq!(rewrite(r"\det_A(x)"), canonical(r"\det_A(x)"));
    }

    #[test]
    fn bare_name_preserved() {
        assert_eq!(rewrite(r"\Pr X"), canonical(r"\Pr X"));
    }
}
