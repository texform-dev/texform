//! Rewrite plain operatorname arguments to equivalent base named-function or named-operator commands.
//!
//! ```yaml
//! proposal: operatorname-to-named-function
//! triggers:
//!   - cmd:operatorname
//! consumes:
//!   eliminates: null
//!   touches: [cmd:operatorname, cmd:limits]
//! produces:
//!   - cmd:arccos
//!   - cmd:arcsin
//!   - cmd:arctan
//!   - cmd:arg
//!   - cmd:cos
//!   - cmd:cosh
//!   - cmd:cot
//!   - cmd:coth
//!   - cmd:csc
//!   - cmd:deg
//!   - cmd:dim
//!   - cmd:exp
//!   - cmd:hom
//!   - cmd:ker
//!   - cmd:lg
//!   - cmd:ln
//!   - cmd:log
//!   - cmd:sec
//!   - cmd:sin
//!   - cmd:sinh
//!   - cmd:tan
//!   - cmd:tanh
//!   - cmd:det
//!   - cmd:gcd
//!   - cmd:inf
//!   - cmd:lim
//!   - cmd:max
//!   - cmd:min
//!   - cmd:Pr
//!   - cmd:sup
//! rewrite_patterns:
//!   - {label: unstarred-named-function, from: '\operatorname{#1}', to: \#1}
//!   - {label: starred-named-operator, from: '\operatorname*{#1}', to: \#1}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;
use texform_knowledge::specs::BuiltinCommandRecord;

use crate::ast::{ArgumentKind, ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId};
use crate::rewrite::helpers::prefix_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static OPERATORNAME_TO_NAMED_FUNCTION: OperatornameToNamedFunctionRule {
        key: Base / "operatorname-to-named-function",
        level: Authoring,
        summary: "Rewrite plain operatorname arguments to equivalent base named-function or named-operator commands.",
        fidelity: Render,
        enabled_by_packages: [Base, Ams],
        triggers: cmd_targets![&ams::cmd::OPERATORNAME],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&ams::cmd::OPERATORNAME, &base::cmd::LIMITS],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::ARCCOS, &base::cmd::ARCSIN, &base::cmd::ARCTAN, &base::cmd::ARG, &base::cmd::COS, &base::cmd::COSH, &base::cmd::COT, &base::cmd::COTH, &base::cmd::CSC, &base::cmd::DEG, &base::cmd::DIM, &base::cmd::EXP, &base::cmd::HOM, &base::cmd::KER, &base::cmd::LG, &base::cmd::LN, &base::cmd::LOG, &base::cmd::SEC, &base::cmd::SIN, &base::cmd::SINH, &base::cmd::TAN, &base::cmd::TANH, &base::cmd::DET, &base::cmd::GCD, &base::cmd::INF, &base::cmd::LIM, &base::cmd::MAX, &base::cmd::MIN, &base::cmd::PR, &base::cmd::SUP],
        },
        apply(_rule, cx, node_id) {
            let (starred, content) = {
                let Some(command) = cx.match_command(node_id, &ams::cmd::OPERATORNAME) else {
                    return Ok(RuleEffect::Skipped);
                };
                if !cx
                    .active_command(node_id)
                    .is_some_and(|record| record.from_packages == ["ams"])
                {
                    return Ok(RuleEffect::Skipped);
                }
                let subject = command.subject();
                let scoped = cx.for_rule(Self::KEY);
                scoped.expect_arg_len(command.args, 2, &subject)?;
                let starred = scoped.star_arg_value(&command.args[0], &subject)?;

                let Some(argument) = &command.args[1] else {
                    return Err(scoped.invalid_shape(format!(
                        "{subject} should carry a mandatory operator-name argument"
                    )));
                };
                if argument.kind != ArgumentKind::Mandatory {
                    return Err(scoped.invalid_shape(format!(
                        "{subject} operator name should be a mandatory argument"
                    )));
                }
                let ArgumentValue::OperatorNameContent(content) = argument.value else {
                    return Err(scoped.invalid_shape(format!(
                        "{subject} operator name should be operator-name content"
                    )));
                };
                (starred, content)
            };

            let Some(name) = plain_operator_name(cx.ast, content) else {
                return Ok(RuleEffect::Skipped);
            };
            let record = if starred {
                named_operator_record(name.as_str())
            } else {
                named_function_record(name.as_str())
            };
            let Some(record) = record else {
                return Ok(RuleEffect::Skipped);
            };
            if !cx
                .lookup_command(record.name, ContentMode::Math)
                .is_some_and(|active| active.from_packages == ["base"])
            {
                return Ok(RuleEffect::Skipped);
            }
            if !starred
                && cx
                .ast
                .next_sibling(node_id)
                .is_some_and(|next| is_limits_modifier(cx.ast, next))
            {
                return Ok(RuleEffect::Skipped);
            }

            cx.ast.replace_node_drop_detached_children(
                node_id,
                prefix_command_node(record, Vec::new()),
            );
            Ok(RuleEffect::Applied)
        }
    }
}

fn is_limits_modifier(ast: &Ast, node_id: NodeId) -> bool {
    match ast.node(node_id) {
        Node::Command { name, args, .. } => name == base::cmd::LIMITS.name && args.is_empty(),
        Node::Scripted { base, .. } => is_limits_modifier(ast, *base),
        _ => false,
    }
}

fn plain_operator_name(ast: &Ast, content: NodeId) -> Option<String> {
    let Node::Group {
        children,
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    } = ast.node(content)
    else {
        return None;
    };

    children
        .iter()
        .map(|child| match ast.node(*child) {
            Node::Char(ch) if ch.is_ascii() => Some(*ch),
            _ => None,
        })
        .collect()
}

fn named_function_record(name: &str) -> Option<&'static BuiltinCommandRecord> {
    match name {
        "arccos" => Some(&base::cmd::ARCCOS),
        "arcsin" => Some(&base::cmd::ARCSIN),
        "arctan" => Some(&base::cmd::ARCTAN),
        "arg" => Some(&base::cmd::ARG),
        "cos" => Some(&base::cmd::COS),
        "cosh" => Some(&base::cmd::COSH),
        "cot" => Some(&base::cmd::COT),
        "coth" => Some(&base::cmd::COTH),
        "csc" => Some(&base::cmd::CSC),
        "deg" => Some(&base::cmd::DEG),
        "dim" => Some(&base::cmd::DIM),
        "exp" => Some(&base::cmd::EXP),
        "hom" => Some(&base::cmd::HOM),
        "ker" => Some(&base::cmd::KER),
        "lg" => Some(&base::cmd::LG),
        "ln" => Some(&base::cmd::LN),
        "log" => Some(&base::cmd::LOG),
        "sec" => Some(&base::cmd::SEC),
        "sin" => Some(&base::cmd::SIN),
        "sinh" => Some(&base::cmd::SINH),
        "tan" => Some(&base::cmd::TAN),
        "tanh" => Some(&base::cmd::TANH),
        _ => None,
    }
}

fn named_operator_record(name: &str) -> Option<&'static BuiltinCommandRecord> {
    match name {
        "det" => Some(&base::cmd::DET),
        "gcd" => Some(&base::cmd::GCD),
        "inf" => Some(&base::cmd::INF),
        "lim" => Some(&base::cmd::LIM),
        "max" => Some(&base::cmd::MAX),
        "min" => Some(&base::cmd::MIN),
        "Pr" => Some(&base::cmd::PR),
        "sup" => Some(&base::cmd::SUP),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::rewrite::{RuleLevel, run_one_rule_for_test, transform_examples};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: OPERATORNAME_TO_NAMED_FUNCTION,
        level: Authoring,
        examples: [
        {
            label: unstarred_arccos,
            packages: ["base", "ams"],
            input: r"\operatorname{arccos} x",
            expected: r"\arccos x",
        },
        {
            label: unstarred_arcsin,
            packages: ["base", "ams"],
            input: r"\operatorname{arcsin} x",
            expected: r"\arcsin x",
        },
        {
            label: unstarred_arctan,
            packages: ["base", "ams"],
            input: r"\operatorname{arctan} x",
            expected: r"\arctan x",
        },
        {
            label: unstarred_arg,
            packages: ["base", "ams"],
            input: r"\operatorname{arg} x",
            expected: r"\arg x",
        },
        {
            label: unstarred_cos,
            packages: ["base", "ams"],
            input: r"\operatorname{cos} x",
            expected: r"\cos x",
        },
        {
            label: unstarred_cosh,
            packages: ["base", "ams"],
            input: r"\operatorname{cosh} x",
            expected: r"\cosh x",
        },
        {
            label: unstarred_cot,
            packages: ["base", "ams"],
            input: r"\operatorname{cot} x",
            expected: r"\cot x",
        },
        {
            label: unstarred_coth,
            packages: ["base", "ams"],
            input: r"\operatorname{coth} x",
            expected: r"\coth x",
        },
        {
            label: unstarred_csc,
            packages: ["base", "ams"],
            input: r"\operatorname{csc} x",
            expected: r"\csc x",
        },
        {
            label: unstarred_deg,
            packages: ["base", "ams"],
            input: r"\operatorname{deg} x",
            expected: r"\deg x",
        },
        {
            label: unstarred_dim,
            packages: ["base", "ams"],
            input: r"\operatorname{dim} x",
            expected: r"\dim x",
        },
        {
            label: unstarred_exp,
            packages: ["base", "ams"],
            input: r"\operatorname{exp} x",
            expected: r"\exp x",
        },
        {
            label: unstarred_hom,
            packages: ["base", "ams"],
            input: r"\operatorname{hom} x",
            expected: r"\hom x",
        },
        {
            label: unstarred_ker,
            packages: ["base", "ams"],
            input: r"\operatorname{ker} x",
            expected: r"\ker x",
        },
        {
            label: unstarred_lg,
            packages: ["base", "ams"],
            input: r"\operatorname{lg} x",
            expected: r"\lg x",
        },
        {
            label: unstarred_ln,
            packages: ["base", "ams"],
            input: r"\operatorname{ln} x",
            expected: r"\ln x",
        },
        {
            label: unstarred_log,
            packages: ["base", "ams"],
            input: r"\operatorname{log} x",
            expected: r"\log x",
        },
        {
            label: unstarred_sec,
            packages: ["base", "ams"],
            input: r"\operatorname{sec} x",
            expected: r"\sec x",
        },
        {
            label: unstarred_sin,
            packages: ["base", "ams"],
            input: r"\operatorname{sin} x",
            expected: r"\sin x",
        },
        {
            label: unstarred_sinh,
            packages: ["base", "ams"],
            input: r"\operatorname{sinh} x",
            expected: r"\sinh x",
        },
        {
            label: unstarred_tan,
            packages: ["base", "ams"],
            input: r"\operatorname{tan} x",
            expected: r"\tan x",
        },
        {
            label: unstarred_tanh,
            packages: ["base", "ams"],
            input: r"\operatorname{tanh} x",
            expected: r"\tanh x",
        },
        {
            label: starred_det,
            packages: ["base", "ams"],
            input: r"\operatorname*{det}_{x}",
            expected: r"\det_{x}",
        },
        {
            label: starred_gcd,
            packages: ["base", "ams"],
            input: r"\operatorname*{gcd}_{x}",
            expected: r"\gcd_{x}",
        },
        {
            label: starred_inf,
            packages: ["base", "ams"],
            input: r"\operatorname*{inf}_{x}",
            expected: r"\inf_{x}",
        },
        {
            label: starred_lim,
            packages: ["base", "ams"],
            input: r"\operatorname*{lim}_{x}",
            expected: r"\lim_{x}",
        },
        {
            label: starred_max,
            packages: ["base", "ams"],
            input: r"\operatorname*{max}_{x}",
            expected: r"\max_{x}",
        },
        {
            label: starred_min,
            packages: ["base", "ams"],
            input: r"\operatorname*{min}_{x}",
            expected: r"\min_{x}",
        },
        {
            label: starred_pr,
            packages: ["base", "ams"],
            input: r"\operatorname*{Pr}_{x}",
            expected: r"\Pr_{x}",
        },
        {
            label: starred_sup,
            packages: ["base", "ams"],
            input: r"\operatorname*{sup}_{x}",
            expected: r"\sup_{x}",
        },
        {
            label: starred_named_function_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname*{ln}_{n}",
            expected: r"\operatorname*{ln}_{n}",
        },
        {
            label: unstarred_named_operator_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname{lim}_{n}",
            expected: r"\operatorname{lim}_{n}",
        },
        {
            label: unknown_name_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname{rank} A",
            expected: r"\operatorname{rank} A",
        },
        {
            label: nested_group_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname{{ln}} x",
            expected: r"\operatorname{{ln}} x",
        },
        {
            label: explicit_spacing_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname{l\,n} x",
            expected: r"\operatorname{l\,n} x",
        },
        {
            label: non_ascii_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname{λ} x",
            expected: r"\operatorname{λ} x",
        },
        {
            label: bare_limits_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname{ln}\limits x",
            expected: r"\operatorname{ln}\limits x",
        },
        {
            label: scripted_limits_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname{ln}\limits_{n}",
            expected: r"\operatorname{ln}\limits_{n}",
        },
        {
            label: unstarred_nolimits_rewritten,
            packages: ["base", "ams"],
            input: r"\operatorname{ln}\nolimits_{n}",
            expected: r"\ln\nolimits_{n}",
        },
        {
            label: starred_limits_rewritten,
            packages: ["base", "ams"],
            input: r"\operatorname*{lim}\limits_{n}",
            expected: r"\lim\limits_{n}",
        },
        {
            label: starred_nolimits_rewritten,
            packages: ["base", "ams"],
            input: r"\operatorname*{max}\nolimits_{n}",
            expected: r"\max\nolimits_{n}",
        },
        {
            label: spaced_liminf_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname*{liminf}_{n}",
            expected: r"\operatorname*{liminf}_{n}",
        },
        {
            label: spaced_limsup_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname*{limsup}_{n}",
            expected: r"\operatorname*{limsup}_{n}",
        },
        {
            label: spaced_injlim_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname*{injlim}_{n}",
            expected: r"\operatorname*{injlim}_{n}",
        },
        {
            label: spaced_projlim_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname*{projlim}_{n}",
            expected: r"\operatorname*{projlim}_{n}",
        },
        {
            label: physics_optional_argument_collision_preserved,
            packages: ["base", "ams", "physics"],
            input: r"\operatorname{sin}[2]x",
            expected: r"\operatorname{sin}[2]x",
        },
        {
            label: physics_expression_collision_preserved,
            packages: ["base", "ams", "physics"],
            input: r"\operatorname{exp}(\frac12)",
            expected: r"\operatorname{exp}(\frac12)",
        },
        {
            label: physics_base_named_function_rewritten,
            packages: ["base", "ams", "physics"],
            input: r"\operatorname{arg}x",
            expected: r"\arg x",
        },
        {
            label: physics_det_collision_preserved,
            packages: ["base", "ams", "physics"],
            input: r"\operatorname*{det}_{n}",
            expected: r"\operatorname*{det}_{n}",
        },
        {
            label: physics_pr_collision_preserved,
            packages: ["base", "ams", "physics"],
            input: r"\operatorname*{Pr}_{n}",
            expected: r"\operatorname*{Pr}_{n}",
        },
        {
            label: physics_base_named_operator_rewritten,
            packages: ["base", "ams", "physics"],
            input: r"\operatorname*{lim}_{n}",
            expected: r"\lim_{n}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: OPERATORNAME_TO_NAMED_FUNCTION,
        level: Authoring,
        examples: [
        {
            label: rewrites_complete_whitelist,
            packages: ["base", "ams"],
            input: r"\operatorname{arccos}\operatorname{arcsin}\operatorname{arctan}\operatorname{arg}\operatorname{cos}\operatorname{cosh}\operatorname{cot}\operatorname{coth}\operatorname{csc}\operatorname{deg}\operatorname{dim}\operatorname{exp}\operatorname{hom}\operatorname{ker}\operatorname{lg}\operatorname{ln}\operatorname{log}\operatorname{sec}\operatorname{sin}\operatorname{sinh}\operatorname{tan}\operatorname{tanh}",
            expected: r"\arccos\arcsin\arctan\arg\cos\cosh\cot\coth\csc\deg\dim\exp\hom\ker\lg\ln\log\sec\sin\sinh\tan\tanh",
        },
        {
            label: preserves_unknown_name,
            packages: ["base", "ams"],
            input: r"\operatorname{rank} A",
            expected: r"\operatorname{rank} A",
        },
        {
            label: preserves_named_operator_with_limits_semantics,
            packages: ["base", "ams"],
            input: r"\operatorname{lim}_{n}",
            expected: r"\operatorname{lim}_{n}",
        },
        {
            label: preserves_nested_group,
            packages: ["base", "ams"],
            input: r"\operatorname{{ln}} x",
            expected: r"\operatorname{{ln}} x",
        },
        {
            label: preserves_styled_name,
            packages: ["base", "ams"],
            input: r"\operatorname{\mathrm{ln}} x",
            expected: r"\operatorname{\mathrm{ln}} x",
        },
        {
            label: preserves_explicit_spacing,
            packages: ["base", "ams"],
            input: r"\operatorname{l\,n} x",
            expected: r"\operatorname{l\,n} x",
        },
        {
            label: preserves_bare_limits_modifier,
            packages: ["base", "ams"],
            input: r"\operatorname{ln}\limits x",
            expected: r"\operatorname{ln}\limits x",
        },
        {
            label: preserves_scripted_limits_modifier,
            packages: ["base", "ams"],
            input: r"\operatorname{ln}\limits_{n}",
            expected: r"\operatorname{ln}\limits_{n}",
        },
        ]
    }

    #[test]
    fn removes_replaced_operator_name_content() {
        for input in [r"\operatorname{ln} x", r"\operatorname*{lim}_{n}"] {
            let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
            let mut ast = crate::parse_to_ast_for_test(
                &parse_ctx,
                input,
                &crate::parse::ParseConfig::STRICT,
            );
            let root_child = ast.children(ast.root())[0];
            let operatorname = match ast.node(root_child) {
                Node::Scripted { base, .. } => *base,
                _ => root_child,
            };
            let Some(argument) = &ast.arg_slots(operatorname)[1] else {
                panic!("operatorname should carry a mandatory argument");
            };
            let ArgumentValue::OperatorNameContent(content) = argument.value else {
                panic!("operatorname argument should be operator-name content");
            };

            run_one_rule_for_test(
                &mut ast,
                &parse_ctx,
                &OPERATORNAME_TO_NAMED_FUNCTION,
                RuleLevel::Authoring,
            )
            .expect("operatorname-to-named-function transform should succeed");

            assert!(!ast.contains(content));
        }
    }
}
