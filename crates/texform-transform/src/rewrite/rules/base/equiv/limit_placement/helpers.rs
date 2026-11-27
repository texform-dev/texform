//! Shared helpers for dropping explicit limit-placement modifiers.

use texform_knowledge::builtin::{ams, base};
use texform_knowledge::specs::BuiltinCommandRecord;

use crate::ast::{ArgumentKind, ArgumentValue, Node, NodeId};
use crate::rewrite::helpers::{
    active_command_is_builtin, active_command_is_builtin_character, drop_postfix_modifier,
    prefix_command_record,
};
use crate::rewrite::rule::RuleEffect;
use crate::rewrite::rule_context::RuleContext;

pub(super) fn drop_limit_modifier(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    modifier: &'static BuiltinCommandRecord,
) -> RuleEffect {
    drop_postfix_modifier(cx, node_id, modifier, "base", is_eligible_operator)
}

fn is_eligible_operator(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    if is_starred_operatorname(cx, node_id) {
        return true;
    }

    prefix_command_record(cx, node_id).is_some_and(|active| {
        let record = match active.name {
            "mathop" => &base::cmd::MATHOP,
            "arccos" => &base::cmd::ARCCOS,
            "arcsin" => &base::cmd::ARCSIN,
            "arctan" => &base::cmd::ARCTAN,
            "arg" => &base::cmd::ARG,
            "cos" => &base::cmd::COS,
            "cosh" => &base::cmd::COSH,
            "cot" => &base::cmd::COT,
            "coth" => &base::cmd::COTH,
            "csc" => &base::cmd::CSC,
            "deg" => &base::cmd::DEG,
            "dim" => &base::cmd::DIM,
            "gcd" => &base::cmd::GCD,
            "hom" => &base::cmd::HOM,
            "inf" => &base::cmd::INF,
            "ker" => &base::cmd::KER,
            "lg" => &base::cmd::LG,
            "lim" => &base::cmd::LIM,
            "liminf" => &base::cmd::LIMINF,
            "limsup" => &base::cmd::LIMSUP,
            "ln" => &base::cmd::LN,
            "log" => &base::cmd::LOG,
            "max" => &base::cmd::MAX,
            "min" => &base::cmd::MIN,
            "sec" => &base::cmd::SEC,
            "sin" => &base::cmd::SIN,
            "sinh" => &base::cmd::SINH,
            "sup" => &base::cmd::SUP,
            "tan" => &base::cmd::TAN,
            "tanh" => &base::cmd::TANH,
            "det" => return active_command_is_builtin(active, &base::cmd::DET, "base", true),
            "exp" => return active_command_is_builtin(active, &base::cmd::EXP, "base", true),
            "Pr" => return active_command_is_builtin(active, &base::cmd::PR, "base", true),
            "injlim" => return active_command_is_builtin(active, &ams::cmd::INJLIM, "ams", false),
            "projlim" => {
                return active_command_is_builtin(active, &ams::cmd::PROJLIM, "ams", false);
            }
            "varliminf" => {
                return active_command_is_builtin(active, &ams::cmd::VARLIMINF, "ams", false);
            }
            "varlimsup" => {
                return active_command_is_builtin(active, &ams::cmd::VARLIMSUP, "ams", false);
            }
            "varinjlim" => {
                return active_command_is_builtin(active, &ams::cmd::VARINJLIM, "ams", false);
            }
            "varprojlim" => {
                return active_command_is_builtin(active, &ams::cmd::VARPROJLIM, "ams", false);
            }
            _ => return is_eligible_character(cx, node_id),
        };
        active_command_is_builtin(active, record, "base", false)
    })
}

fn is_starred_operatorname(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    let Some(active) = prefix_command_record(cx, node_id) else {
        return false;
    };
    if !active_command_is_builtin(active, &ams::cmd::OPERATORNAME, "ams", true) {
        return false;
    }
    let Node::Command { args, .. } = cx.ast.node(node_id) else {
        return false;
    };
    matches!(
        args.first(),
        Some(Some(argument))
            if argument.kind == ArgumentKind::Star
                && matches!(argument.value, ArgumentValue::Boolean(true))
    )
}

fn is_eligible_character(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    let Some(active) = prefix_command_record(cx, node_id) else {
        return false;
    };
    let (record, package) = match active.name {
        "coprod" => (&base::chars::COPROD, "base"),
        "bigvee" => (&base::chars::BIGVEE, "base"),
        "bigwedge" => (&base::chars::BIGWEDGE, "base"),
        "biguplus" => (&base::chars::BIGUPLUS, "base"),
        "bigcap" => (&base::chars::BIGCAP, "base"),
        "bigcup" => (&base::chars::BIGCUP, "base"),
        "int" => (&base::chars::INT, "base"),
        "intop" => (&base::chars::INTOP, "base"),
        "iint" => (&base::chars::IINT, "base"),
        "iiint" => (&base::chars::IIINT, "base"),
        "prod" => (&base::chars::PROD, "base"),
        "sum" => (&base::chars::SUM, "base"),
        "bigotimes" => (&base::chars::BIGOTIMES, "base"),
        "bigoplus" => (&base::chars::BIGOPLUS, "base"),
        "bigodot" => (&base::chars::BIGODOT, "base"),
        "oint" => (&base::chars::OINT, "base"),
        "ointop" => (&base::chars::OINTOP, "base"),
        "oiint" => (&base::chars::OIINT, "base"),
        "oiiint" => (&base::chars::OIIINT, "base"),
        "bigsqcup" => (&base::chars::BIGSQCUP, "base"),
        "smallint" => (&base::chars::SMALLINT, "base"),
        "iiiint" => (&ams::chars::IIIINT, "ams"),
        _ => return false,
    };
    active_command_is_builtin_character(cx, active, record, package)
}
