//! Expand physics power and auto-sized parenthesis arguments of standard named functions.
//!
//! ```yaml
//! proposal: named-fn-paren-expand
//! triggers:
//!   - cmd:sin
//!   - cmd:sinh
//!   - cmd:arcsin
//!   - cmd:cos
//!   - cmd:cosh
//!   - cmd:arccos
//!   - cmd:tan
//!   - cmd:tanh
//!   - cmd:arctan
//!   - cmd:csc
//!   - cmd:sec
//!   - cmd:cot
//!   - cmd:coth
//!   - cmd:log
//!   - cmd:ln
//! consumes:
//!   eliminates: null
//!   touches: [cmd:sin, cmd:sinh, cmd:arcsin, cmd:cos, cmd:cosh, cmd:arccos, cmd:tan, cmd:tanh, cmd:arctan, cmd:csc, cmd:sec, cmd:cot, cmd:coth, cmd:log, cmd:ln]
//! produces:
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: trig-paren, from: \sin(#1), to: \sin\left(#1\right)}
//!   - {label: trig-power-paren, from: '\sin[#1](#2)', to: '\sin^{#1}\left(#2\right)'}
//! ```
//!
//! Semantics follow LaTeX physics.sty, where these names use `\trigbraces`
//! (`o d()`): `\sin[n](...)` prints a power and auto-sized parentheses, while
//! `\sin[n]` without parentheses prints `[n]` literally. MathJax reads that
//! last branch as a power, so the rule leaves it unchanged. The physics
//! records hold the parentheses as a `d()` slot, so the rule rewrites only an
//! argument the parser attached; parentheses separated from the command, as
//! in `{\sin}(x)` or `\sin{}(x)`, stay literal.
//!
//! Physics closes the fence inside `\mathopen{}\mathclose\bgroup...\egroup`,
//! so it acts as a closing atom, while an explicit `\left...\right` fence is
//! an inner atom. The explicit fence therefore adds about 1/6 em before the
//! opening delimiter and, when an ordinary atom or an opening delimiter
//! follows the fence, about another 1/6 em after it: about 1/3 em in the worst
//! case. Script styles show no difference. No compensating space is inserted.

use texform_knowledge::builtin::{base, physics};
use texform_knowledge::specs::BuiltinCommandRecord;

use super::helpers::{fence_argument, replace_with_fenced_operator, with_power};
use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

/// Names that physics.sty redefines through `\trigbraces`.
const TRIG_FUNCTIONS: [&BuiltinCommandRecord; 15] = [
    &physics::cmd::SIN,
    &physics::cmd::SINH,
    &physics::cmd::ARCSIN,
    &physics::cmd::COS,
    &physics::cmd::COSH,
    &physics::cmd::ARCCOS,
    &physics::cmd::TAN,
    &physics::cmd::TANH,
    &physics::cmd::ARCTAN,
    &physics::cmd::CSC,
    &physics::cmd::SEC,
    &physics::cmd::COT,
    &physics::cmd::COTH,
    &physics::cmd::LOG,
    &physics::cmd::LN,
];

define_rule! {
    pub static NAMED_FN_PAREN_EXPAND: NamedFnParenExpandRule {
        key: Physics / "named-fn-paren-expand",
        level: Faithful,
        summary: "Expand physics power and auto-sized parenthesis arguments of standard named functions.",
        fidelity: Reading,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::SIN, &physics::cmd::SINH, &physics::cmd::ARCSIN, &physics::cmd::COS, &physics::cmd::COSH, &physics::cmd::ARCCOS, &physics::cmd::TAN, &physics::cmd::TANH, &physics::cmd::ARCTAN, &physics::cmd::CSC, &physics::cmd::SEC, &physics::cmd::COT, &physics::cmd::COTH, &physics::cmd::LOG, &physics::cmd::LN],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&physics::cmd::SIN, &physics::cmd::SINH, &physics::cmd::ARCSIN, &physics::cmd::COS, &physics::cmd::COSH, &physics::cmd::ARCCOS, &physics::cmd::TAN, &physics::cmd::TANH, &physics::cmd::ARCTAN, &physics::cmd::CSC, &physics::cmd::SEC, &physics::cmd::COT, &physics::cmd::COTH, &physics::cmd::LOG, &physics::cmd::LN],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(_rule, cx, node_id) {
            let Some(record) = TRIG_FUNCTIONS
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
            let (power, argument) = {
                let command = cx
                    .match_command(node_id, record)
                    .expect("matched command should still match");
                let subject = command.subject();
                let scoped = cx.for_rule(Self::KEY);
                scoped.expect_arg_len(command.args, 2, &subject)?;
                (
                    scoped.optional_math_content(&command.args[0], &subject, "power")?,
                    fence_argument(&scoped, &command.args[1], &subject)?,
                )
            };
            // Without parentheses, MathJax reads `[n]` as a power while LaTeX
            // prints it literally, so the command is left unchanged.
            let Some(argument) = argument else {
                return Ok(RuleEffect::Skipped);
            };

            let operator = cx.ast.new_node(Node::Command {
                name: record.name.to_string(),
                args: vec![None; record.argspec.args.len()],
                known: true,
            });
            let operator = with_power(cx.ast, operator, power);
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
        rule: NAMED_FN_PAREN_EXPAND,
        level: Faithful,
        examples: [
        {
            label: trig_paren_nested,
            packages: ["base", "ams", "physics"],
            input: r"\sin(x+(y-z)\frac{a}{b})",
            expected: r"\sin\left(x+(y-z)\frac{a}{b}\right)",
        },
        {
            label: trig_power_paren,
            packages: ["base", "ams", "physics"],
            input: r"\sin[2](\frac{x}{y})",
            expected: r"\sin^{2}\left(\frac{x}{y}\right)",
        },
        {
            label: trig_paren_scripted_close,
            packages: ["base", "ams", "physics"],
            input: r"\cos(\frac{x}{y})^2",
            expected: r"\cos\left(\frac{x}{y}\right)^2",
        },
        {
            label: trig_paren_ordinary_after,
            packages: ["base", "ams", "physics"],
            input: r"\ln(x)y",
            expected: r"\ln\left(x\right)y",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &NAMED_FN_PAREN_EXPAND, RuleLevel::Faithful)
            .expect("rule should run");
        ast.assert_invariants();
        serialize(&ast)
    }

    fn canonical(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        serialize(&crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT))
    }

    #[test]
    fn parenthesis_inside_group_is_not_a_closer() {
        assert_eq!(rewrite(r"\sin(x{y)}z)"), canonical(r"\sin\left(x{y)}z\right)"));
    }

    #[test]
    fn script_after_opener_stays_in_fence() {
        assert_eq!(rewrite(r"\sin(^2x)"), canonical(r"\sin\left(^2x\right)"));
    }

    #[test]
    fn braced_argument_preserved() {
        assert_eq!(rewrite(r"\sin{x}"), canonical(r"\sin{x}"));
    }

    #[test]
    fn bracket_after_power_is_not_an_argument() {
        assert_eq!(rewrite(r"\log[2][x]"), canonical(r"\log[2][x]"));
    }

    #[test]
    fn literal_power_without_parentheses_preserved() {
        assert_eq!(rewrite(r"\sin[2]x"), canonical(r"\sin[2]x"));
    }

    #[test]
    fn scripted_command_preserved() {
        assert_eq!(rewrite(r"\sin^2(x)"), canonical(r"\sin^2(x)"));
    }

    #[test]
    fn bare_name_preserved() {
        assert_eq!(rewrite(r"\ln x"), canonical(r"\ln x"));
    }
}
