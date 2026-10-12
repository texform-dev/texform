//! Rewrite physics-only named functions to operatorname and expand their auto-sized arguments.
//!
//! ```yaml
//! proposal: named-fn-to-operatorname
//! triggers:
//!   - cmd:asin
//!   - cmd:acos
//!   - cmd:atan
//!   - cmd:csch
//!   - cmd:arccsc
//!   - cmd:acsc
//!   - cmd:sech
//!   - cmd:arcsec
//!   - cmd:asec
//!   - cmd:arccot
//!   - cmd:acot
//!   - cmd:tr
//!   - cmd:Tr
//!   - cmd:erf
//!   - cmd:trace
//!   - cmd:Trace
//!   - cmd:sine
//!   - cmd:cosine
//!   - cmd:naturallogarithm
//!   - cmd:Probability
//! consumes:
//!   eliminates: [cmd:asin, cmd:acos, cmd:atan, cmd:csch, cmd:arccsc, cmd:acsc, cmd:sech, cmd:arcsec, cmd:asec, cmd:arccot, cmd:acot, cmd:tr, cmd:Tr, cmd:erf, cmd:trace, cmd:Trace]
//!   touches: [cmd:sine, cmd:cosine, cmd:naturallogarithm, cmd:Probability]
//! produces:
//!   - cmd:operatorname
//!   - cmd:left
//!   - cmd:right
//! rewrite_patterns:
//!   - {label: bare, from: \asin, to: '\operatorname{asin}'}
//!   - {label: trig-paren, from: \asin(#1), to: '\operatorname{asin}\left(#1\right)'}
//!   - {label: trig-power-paren, from: '\asin[#1](#2)', to: '\operatorname{asin}^{#1}\left(#2\right)'}
//!   - {label: trig-literal-power, from: '\asin[#1]', to: '\operatorname{asin}[#1]'}
//!   - {label: operator-brace, from: '\tr{#1}', to: '\operatorname{tr}\left\{#1\right\}'}
//!   - {label: operator-bracket, from: '\tr[#1]', to: '\operatorname{tr}\left[#1\right]'}
//!   - {label: operator-paren, from: \tr(#1), to: '\operatorname{tr}\left(#1\right)'}
//!   - {label: plain-operator, from: \erf, to: '\operatorname{erf}'}
//!   - {label: trace-alias, from: \trace, to: '\operatorname{tr}'}
//!   - {label: long-name-argument, from: \sine(#1), to: '\operatorname{sin}(#1)'}
//! ```
//!
//! Semantics follow LaTeX physics.sty. None of these names has a base
//! command, so each becomes `\operatorname` with its printed name. Trig-family
//! names use `\trigbraces` (`o d()`), `\tr` and `\Tr` use `\opbraces`
//! (`g o d()`, recorded as one paired `d<(,)><[,]><{,}>` slot because only
//! the first group is fenced), and `\erf`, `\trace`, and `\Trace` are plain
//! `\DeclareMathOperator` commands whose following delimiters stay literal.
//! The long names `\sine`, `\cosine`, `\naturallogarithm`, and `\Probability`
//! are the original operators saved with `\let`; the rule rewrites them only
//! when `(`, `[`, or a brace group follows, where the short command would grab
//! it, and leaves bare occurrences to the authoring aliases.
//!
//! A grabbed argument becomes an explicit `\left...\right` fence.
//! Physics closes the fence inside `\mathopen{}\mathclose\bgroup...\egroup`,
//! so it acts as a closing atom, while an explicit `\left...\right` fence is
//! an inner atom. The explicit fence therefore adds about 1/6 em before the
//! opening delimiter and, when an ordinary atom or an opening delimiter
//! follows the fence, about another 1/6 em after it: about 1/3 em in the worst
//! case. Script styles show no difference. No compensating space is inserted.
//!
//! MathJax differs: it reads `\asin[2]x` as a power, auto-sizes `\erf(...)`
//! and `\trace(...)`, hides the braces of `\tr{...}`, and prints `\tr[...]`
//! literally.

use texform_knowledge::builtin::{ams, base, physics};
use texform_knowledge::specs::BuiltinCommandRecord;

use super::helpers::{
    ArgumentBraces, fence_argument, replace_with_fenced_operator, with_power,
};
use crate::ast::{Ast, ContentMode, GroupKind, Node};
use crate::rewrite::helpers::{
    followed_by_paren_bracket_or_group, mandatory_operator_name_slot, prefix_command_node,
    star_slot,
};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

/// Physics-only named functions, their printed operator names, and argument handling.
const NAMED_FUNCTIONS: [(&BuiltinCommandRecord, &str, ArgumentBraces); 16] = [
    (&physics::cmd::ASIN, "asin", ArgumentBraces::Trig),
    (&physics::cmd::ACOS, "acos", ArgumentBraces::Trig),
    (&physics::cmd::ATAN, "atan", ArgumentBraces::Trig),
    (&physics::cmd::CSCH, "csch", ArgumentBraces::Trig),
    (&physics::cmd::ARCCSC, "arccsc", ArgumentBraces::Trig),
    (&physics::cmd::ACSC, "acsc", ArgumentBraces::Trig),
    (&physics::cmd::SECH, "sech", ArgumentBraces::Trig),
    (&physics::cmd::ARCSEC, "arcsec", ArgumentBraces::Trig),
    (&physics::cmd::ASEC, "asec", ArgumentBraces::Trig),
    (&physics::cmd::ARCCOT, "arccot", ArgumentBraces::Trig),
    (&physics::cmd::ACOT, "acot", ArgumentBraces::Trig),
    (&physics::cmd::TR, "tr", ArgumentBraces::Operator),
    (&physics::cmd::TR_2, "Tr", ArgumentBraces::Operator),
    (&physics::cmd::ERF, "erf", ArgumentBraces::Plain),
    (&physics::cmd::TRACE, "tr", ArgumentBraces::Plain),
    (&physics::cmd::TRACE_2, "Tr", ArgumentBraces::Plain),
];

/// Long physics names of standard operators and the names they print.
///
/// They are the original LaTeX operators saved with `\let`, so a following
/// delimiter stays literal. The bare forms are renamed to the short command by
/// the authoring aliases; this rule rewrites only an occurrence followed by
/// `(`, `[`, or a brace group, where the short command would grab it.
const LONG_NAMES: [(&BuiltinCommandRecord, &str); 4] = [
    (&physics::cmd::SINE, "sin"),
    (&physics::cmd::COSINE, "cos"),
    (&physics::cmd::NATURALLOGARITHM, "ln"),
    (&physics::cmd::PROBABILITY, "Pr"),
];

define_rule! {
    pub static NAMED_FN_TO_OPERATORNAME: NamedFnToOperatornameRule {
        key: Physics / "named-fn-to-operatorname",
        level: Faithful,
        summary: "Rewrite physics-only named functions to operatorname and expand their auto-sized arguments.",
        fidelity: Reading,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::ASIN, &physics::cmd::ACOS, &physics::cmd::ATAN, &physics::cmd::CSCH, &physics::cmd::ARCCSC, &physics::cmd::ACSC, &physics::cmd::SECH, &physics::cmd::ARCSEC, &physics::cmd::ASEC, &physics::cmd::ARCCOT, &physics::cmd::ACOT, &physics::cmd::TR, &physics::cmd::TR_2, &physics::cmd::ERF, &physics::cmd::TRACE, &physics::cmd::TRACE_2, &physics::cmd::SINE, &physics::cmd::COSINE, &physics::cmd::NATURALLOGARITHM, &physics::cmd::PROBABILITY],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::ASIN, &physics::cmd::ACOS, &physics::cmd::ATAN, &physics::cmd::CSCH, &physics::cmd::ARCCSC, &physics::cmd::ACSC, &physics::cmd::SECH, &physics::cmd::ARCSEC, &physics::cmd::ASEC, &physics::cmd::ARCCOT, &physics::cmd::ACOT, &physics::cmd::TR, &physics::cmd::TR_2, &physics::cmd::ERF, &physics::cmd::TRACE, &physics::cmd::TRACE_2],
            touches: cmd_targets![&physics::cmd::SINE, &physics::cmd::COSINE, &physics::cmd::NATURALLOGARITHM, &physics::cmd::PROBABILITY],
        },
        produces: RuleProduces {
            targets: cmd_targets![&ams::cmd::OPERATORNAME, &base::cmd::LEFT, &base::cmd::RIGHT],
        },
        apply(_rule, cx, node_id) {
            if let Some((record, printed)) = LONG_NAMES
                .into_iter()
                .find(|(record, _)| cx.match_command(node_id, record).is_some())
            {
                let command = cx
                    .match_command(node_id, record)
                    .expect("matched command should still match");
                cx.for_rule(Self::KEY).expect_no_args(command.args, &command.subject())?;
                if !followed_by_paren_bracket_or_group(cx.ast, node_id) {
                    return Ok(RuleEffect::Skipped);
                }
                let operator = operatorname_node(cx.ast, printed);
                cx.ast.replace_node_drop_detached_children(node_id, operator);
                return Ok(RuleEffect::Applied);
            }
            let Some((record, printed, braces)) = NAMED_FUNCTIONS
                .into_iter()
                .find(|(record, _, _)| cx.match_command(node_id, record).is_some())
            else {
                return Ok(RuleEffect::Skipped);
            };
            let (power, argument) = {
                let command = cx
                    .match_command(node_id, record)
                    .expect("matched command should still match");
                let subject = command.subject();
                let scoped = cx.for_rule(Self::KEY);
                match braces {
                    ArgumentBraces::Trig => {
                        scoped.expect_arg_len(command.args, 2, &subject)?;
                        (
                            scoped.optional_math_content(&command.args[0], &subject, "power")?,
                            fence_argument(&scoped, &command.args[1], &subject)?,
                        )
                    }
                    ArgumentBraces::Operator => {
                        scoped.expect_arg_len(command.args, 1, &subject)?;
                        (None, fence_argument(&scoped, &command.args[0], &subject)?)
                    }
                    ArgumentBraces::Plain => {
                        scoped.expect_no_args(command.args, &subject)?;
                        (None, None)
                    }
                }
            };

            let operator = operatorname_node(cx.ast, printed);
            match (argument, power) {
                (Some(argument), power) => {
                    let operator = cx.ast.new_node(operator);
                    let operator = with_power(cx.ast, operator, power);
                    replace_with_fenced_operator(cx.ast, node_id, operator, argument);
                }
                // Without parentheses, physics prints the power as literal `[...]`.
                (None, Some(power)) => {
                    let operator = cx.ast.new_node(operator);
                    let mut after = vec![cx.ast.new_node(Node::Char('['))];
                    cx.ast.append_cloned_math_content(&mut after, power);
                    after.push(cx.ast.new_node(Node::Char(']')));
                    cx.ast.replace_with_math_sequence_preserving_scripts(
                        node_id,
                        Vec::new(),
                        operator,
                        after,
                    );
                }
                (None, None) => cx.ast.replace_node_drop_detached_children(node_id, operator),
            }
            Ok(RuleEffect::Applied)
        }
    }
}

fn operatorname_node(ast: &mut Ast, name: &str) -> Node {
    let children = name.chars().map(|ch| ast.new_node(Node::Char(ch))).collect();
    let content = ast.new_node(Node::Group {
        children,
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    });
    prefix_command_node(
        &ams::cmd::OPERATORNAME,
        vec![star_slot(false), mandatory_operator_name_slot(content)],
    )
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
        rule: NAMED_FN_TO_OPERATORNAME,
        level: Faithful,
        examples: [
        {
            label: trig_power_paren_nested,
            packages: ["base", "ams", "physics"],
            input: r"\asin[2](x(y+z)\frac{a}{b})",
            expected: r"\operatorname{asin}^{2}\left(x(y+z)\frac{a}{b}\right)",
        },
        {
            label: trig_paren,
            packages: ["base", "ams", "physics"],
            input: r"\sech(\frac{x}{y})",
            expected: r"\operatorname{sech}\left(\frac{x}{y}\right)",
        },
        {
            label: trig_paren_ordinary_after,
            packages: ["base", "ams", "physics"],
            input: r"\csch(x)y",
            expected: r"\operatorname{csch}\left(x\right)y",
        },
        {
            label: trig_literal_power,
            packages: ["base", "ams", "physics"],
            input: r"\asin[2]x",
            expected: r"\operatorname{asin}[2]x",
        },
        {
            label: trig_bare,
            packages: ["base", "ams", "physics"],
            input: r"\csch x",
            expected: r"\operatorname{csch} x",
        },
        {
            label: operator_brace,
            packages: ["base", "ams", "physics"],
            input: r"\tr{\frac{A}{2}}",
            expected: r"\operatorname{tr}\left\{\frac{A}{2}\right\}",
        },
        {
            label: operator_bracket_literal_paren,
            packages: ["base", "ams", "physics"],
            input: r"\Tr[\frac{A}{2}](t)",
            expected: r"\operatorname{Tr}\left[\frac{A}{2}\right](t)",
        },
        {
            label: operator_paren,
            packages: ["base", "ams", "physics"],
            input: r"\tr(\frac{A}{2})",
            expected: r"\operatorname{tr}\left(\frac{A}{2}\right)",
        },
        {
            label: operator_subscript_literal_paren,
            packages: ["base", "ams", "physics"],
            input: r"\tr_A(\rho)",
            expected: r"\operatorname{tr}_A(\rho)",
        },
        {
            label: erf_literal_paren,
            packages: ["base", "ams", "physics"],
            input: r"\erf(\frac{x}{y})",
            expected: r"\operatorname{erf}(\frac{x}{y})",
        },
        {
            label: trace_literal_paren,
            packages: ["base", "ams", "physics"],
            input: r"\trace(\frac{x}{y})",
            expected: r"\operatorname{tr}(\frac{x}{y})",
        },
        {
            label: trace_literal_brace,
            packages: ["base", "ams", "physics"],
            input: r"\trace{\frac{x}{y}}",
            expected: r"\operatorname{tr}{\frac{x}{y}}",
        },
        {
            label: trace_capital_bare,
            packages: ["base", "ams", "physics"],
            input: r"\Trace\rho",
            expected: r"\operatorname{Tr}\rho",
        },
        {
            label: long_name_literal_paren,
            packages: ["base", "ams", "physics"],
            input: r"\sine(\frac{x}{y})",
            expected: r"\operatorname{sin}(\frac{x}{y})",
        },
        {
            label: long_name_literal_brace,
            packages: ["base", "ams", "physics"],
            input: r"\Probability{\frac{A}{2}}",
            expected: r"\operatorname{Pr}{\frac{A}{2}}",
        },
        ]
    }
    // END: Generated examples

    fn rewrite(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &NAMED_FN_TO_OPERATORNAME, RuleLevel::Faithful)
            .expect("rule should run");
        ast.assert_invariants();
        serialize(&ast)
    }

    fn canonical(input: &str) -> String {
        let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
        serialize(&crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT))
    }

    #[test]
    fn script_after_opener_stays_in_fence() {
        assert_eq!(rewrite(r"\tr(^2x)"), canonical(r"\operatorname{tr}\left(^2x\right)"));
    }

    #[test]
    fn scripted_literal_power_keeps_scripts_on_bracket() {
        assert_eq!(rewrite(r"\asin[2]_a x"), canonical(r"\operatorname{asin}[2]_a x"));
    }

    #[test]
    fn operator_only_first_group_is_fenced() {
        assert_eq!(rewrite(r"\Tr{A}[B](C)"), canonical(r"\operatorname{Tr}\left\{A\right\}[B](C)"));
    }

    #[test]
    fn trig_braced_argument_stays_literal() {
        assert_eq!(rewrite(r"\acot{x}"), canonical(r"\operatorname{acot}{x}"));
    }
}
