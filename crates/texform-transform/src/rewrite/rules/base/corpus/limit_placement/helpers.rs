//! Shared helpers for dropping explicit limit-placement modifiers.

use texform_knowledge::builtin::{ams, base};
use texform_knowledge::specs::{BuiltinCharacterRecord, BuiltinCommandRecord};

use crate::ast::{ArgumentKind, ArgumentValue, ContentMode, Node, NodeId, Slot};
use crate::rewrite::rule::RuleEffect;
use crate::rewrite::rule_context::RuleContext;

pub(super) fn drop_limit_modifier(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    modifier: &'static BuiltinCommandRecord,
) -> RuleEffect {
    if !is_builtin_command(cx, node_id, modifier, "base") {
        return RuleEffect::Skipped;
    }
    let Node::Command { args, .. } = cx.ast.node(node_id) else {
        return RuleEffect::Skipped;
    };
    if !args.is_empty() {
        return RuleEffect::Skipped;
    }

    let (modifier_root, scripts) = match cx.ast.parent(node_id) {
        Some(crate::ast::ParentLink {
            slot: Slot::GroupChild(_),
            ..
        }) => (node_id, None),
        Some(crate::ast::ParentLink {
            parent,
            slot: Slot::ScriptBase,
        }) => {
            let Node::Scripted {
                base,
                subscript,
                superscript,
            } = cx.ast.node(parent)
            else {
                return RuleEffect::Skipped;
            };
            if *base != node_id
                || !matches!(cx.ast.slot(parent), Some(Slot::GroupChild(_)))
            {
                return RuleEffect::Skipped;
            }
            (parent, Some((*subscript, *superscript)))
        }
        _ => return RuleEffect::Skipped,
    };

    let Some(operator) = cx.ast.prev_sibling(modifier_root) else {
        return RuleEffect::Skipped;
    };
    if !is_eligible_operator(cx, operator) {
        return RuleEffect::Skipped;
    }

    if let Some((subscript, superscript)) = scripts {
        let base = cx.ast.clone_subtree(operator);
        let subscript = subscript.map(|id| cx.ast.clone_subtree(id));
        let superscript = superscript.map(|id| cx.ast.clone_subtree(id));
        cx.ast.replace_node_drop_detached_children(
            operator,
            Node::Scripted {
                base,
                subscript,
                superscript,
            },
        );
    }
    cx.ast.remove_node(modifier_root);
    RuleEffect::Applied
}

fn is_eligible_operator(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    if is_starred_operatorname(cx, node_id) {
        return true;
    }

    command_record(cx, node_id).is_some_and(|active| {
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
            "det" => return matches_builtin_command(active, &base::cmd::DET, "base", true),
            "exp" => return matches_builtin_command(active, &base::cmd::EXP, "base", true),
            "Pr" => return matches_builtin_command(active, &base::cmd::PR, "base", true),
            "injlim" => return matches_builtin_command(active, &ams::cmd::INJLIM, "ams", false),
            "projlim" => {
                return matches_builtin_command(active, &ams::cmd::PROJLIM, "ams", false);
            }
            "varliminf" => {
                return matches_builtin_command(active, &ams::cmd::VARLIMINF, "ams", false);
            }
            "varlimsup" => {
                return matches_builtin_command(active, &ams::cmd::VARLIMSUP, "ams", false);
            }
            "varinjlim" => {
                return matches_builtin_command(active, &ams::cmd::VARINJLIM, "ams", false);
            }
            "varprojlim" => {
                return matches_builtin_command(active, &ams::cmd::VARPROJLIM, "ams", false);
            }
            _ => return is_eligible_character(cx, node_id),
        };
        matches_builtin_command(active, record, "base", false)
    })
}

fn command_record<'a>(
    cx: &'a RuleContext<'_>,
    node_id: NodeId,
) -> Option<&'a texform_knowledge::specs::ActiveCommandRecord> {
    let Node::Command { .. } = cx.ast.node(node_id) else {
        return None;
    };
    cx.active_command(node_id)
}

fn is_builtin_command(
    cx: &RuleContext<'_>,
    node_id: NodeId,
    record: &'static BuiltinCommandRecord,
    package: &str,
) -> bool {
    command_record(cx, node_id)
        .is_some_and(|active| matches_builtin_command(active, record, package, true))
}

fn matches_builtin_command(
    active: &texform_knowledge::specs::ActiveCommandRecord,
    record: &'static BuiltinCommandRecord,
    package: &str,
    exact_owner: bool,
) -> bool {
    active.name == record.name
        && active.kind == record.kind
        && active.allowed_mode == record.allowed_mode
        && active.argspec.source == record.argspec.source
        && if exact_owner {
            active.from_packages == [package]
        } else {
            active.from_packages.contains(&package)
        }
}

fn is_starred_operatorname(cx: &RuleContext<'_>, node_id: NodeId) -> bool {
    let Some(active) = command_record(cx, node_id) else {
        return false;
    };
    if !matches_builtin_command(active, &ams::cmd::OPERATORNAME, "ams", true) {
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
    let Some(active) = command_record(cx, node_id) else {
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
    matches_character(cx, active, record, package)
}

fn matches_character(
    cx: &RuleContext<'_>,
    active: &texform_knowledge::specs::ActiveCommandRecord,
    record: &'static BuiltinCharacterRecord,
    package: &str,
) -> bool {
    active.name == record.name
        && active.from_packages == [package]
        && cx
            .lookup_character(record.name, ContentMode::Math)
            .is_some_and(|character| character.package == package)
}
