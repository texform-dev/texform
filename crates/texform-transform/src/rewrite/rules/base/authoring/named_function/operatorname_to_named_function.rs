//! Rewrite unstarred plain-text operatorname arguments to a closed whitelist of equivalent base named-function commands.
//!
//! ```yaml
//! proposal: operatorname-to-named-function
//! triggers:
//!   - cmd:operatorname
//! consumes:
//!   eliminates: null
//!   touches:
//!     - cmd:operatorname
//!     - cmd:limits
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
//! rewrite_patterns:
//!   - {from: '\operatorname{#1}', to: \#1}
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
        summary: "Rewrite unstarred plain-text operatorname arguments to a closed whitelist of equivalent base named-function commands.",
        fidelity: Render,
        enabled_by_packages: [Base, Ams],
        triggers: cmd_targets![&ams::cmd::OPERATORNAME],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&ams::cmd::OPERATORNAME, &base::cmd::LIMITS],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::ARCCOS, &base::cmd::ARCSIN, &base::cmd::ARCTAN, &base::cmd::ARG, &base::cmd::COS, &base::cmd::COSH, &base::cmd::COT, &base::cmd::COTH, &base::cmd::CSC, &base::cmd::DEG, &base::cmd::DIM, &base::cmd::EXP, &base::cmd::HOM, &base::cmd::KER, &base::cmd::LG, &base::cmd::LN, &base::cmd::LOG, &base::cmd::SEC, &base::cmd::SIN, &base::cmd::SINH, &base::cmd::TAN, &base::cmd::TANH],
        },
        apply(_rule, cx, node_id) {
            let content = {
                let Some(command) = cx.match_command(node_id, &ams::cmd::OPERATORNAME) else {
                    return Ok(RuleEffect::Skipped);
                };
                let subject = command.subject();
                let scoped = cx.for_rule(Self::KEY);
                scoped.expect_arg_len(command.args, 2, &subject)?;
                if scoped.star_arg_value(&command.args[0], &subject)? {
                    return Ok(RuleEffect::Skipped);
                }

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
                content
            };

            let Some(name) = plain_operator_name(cx.ast, content) else {
                return Ok(RuleEffect::Skipped);
            };
            let Some(record) = named_function_record(name.as_str()) else {
                return Ok(RuleEffect::Skipped);
            };
            if cx
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
            label: whitelist_match,
            packages: ["base", "ams"],
            input: r"\operatorname{ln} x",
            expected: r"\ln x",
        },
        {
            label: starred_preserved,
            packages: ["base", "ams"],
            input: r"\operatorname*{ln}_{n}",
            expected: r"\operatorname*{ln}_{n}",
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
        let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
        let mut ast = crate::parse_to_ast_for_test(
            &parse_ctx,
            r"\operatorname{ln} x",
            &crate::parse::ParseConfig::STRICT,
        );
        let operatorname = ast.children(ast.root())[0];
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
