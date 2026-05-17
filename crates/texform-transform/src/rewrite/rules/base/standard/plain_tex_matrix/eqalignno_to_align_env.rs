//! Rewrite eqalignno to the standard align environment.
//!
//! ```yaml
//! proposal: eqalignno-to-align-env
//! triggers:
//!   - cmd:eqalignno
//! consumes:
//!   eliminates: [cmd:eqalignno, cmd:cr]
//!   touches: null
//! produces:
//!   - env:align
//!   - cmd:tag
//! rewrite_patterns:
//!   - {from: '\eqalignno{#1&#2&(#3) \cr #4&#5&(#6)}', to: '\begin{align} #1&#2 \tag{#3}\\ #4&#5 \tag{#6} \end{align}'}
//! ```

use texform_specs::builtin::ams;
use texform_specs::builtin::base;

use super::helpers::{
    cr_rows, linebreak_command, mandatory_math_body, replace_with_environment, tag_command,
};
use crate::ast::{Node, NodeId};
use crate::rewrite::RuleError;
use crate::rewrite::rule::{RuleConsumes, RuleProduces, RuleTarget};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static EQALIGNNO_TO_ALIGN_ENV: EqalignnoToAlignEnvRule {
        key: Base / "eqalignno-to-align-env",
        class: Standard,
        summary: "Rewrite eqalignno to the standard align environment.",
        safety: Semantic,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::EQALIGNNO],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::EQALIGNNO, &base::cmd::CR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::ALIGN), RuleTarget::Command(&ams::cmd::TAG)],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::EQALIGNNO) else {
                return Ok(crate::rewrite::rule::RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_arg_len(command.args, 1, r"\eqalignno")?;
            let body = mandatory_math_body(
                Self::KEY,
                cx,
                &command.args[0],
                base::cmd::EQALIGNNO.name,
            )?;
            let rows = cr_rows(cx, body);
            let row_count = rows.len();
            let mut children = Vec::new();

            for (index, row) in rows.into_iter().enumerate() {
                let (row, tag) = split_eqalignno_row(Self::KEY, cx, row)?;
                children.extend(row);
                children.push(cx.ast.new_node(tag_command(tag)));
                if index + 1 < row_count {
                    children.push(cx.ast.new_node(linebreak_command()));
                }
            }

            replace_with_environment(cx, node_id, &ams::env::ALIGN, Vec::new(), children);
            Ok(crate::rewrite::rule::RuleEffect::Applied)
        }
    }
}

fn split_eqalignno_row(
    rule: crate::rewrite::rule::RuleKey,
    cx: &mut RuleContext<'_>,
    mut row: Vec<NodeId>,
) -> Result<(Vec<NodeId>, NodeId), RuleError> {
    let Some(last) = row.last().copied() else {
        return Err(cx.for_rule(rule).invalid_shape(r"\eqalignno row should not be empty"));
    };
    if !matches!(cx.ast.node(last), Node::Char(')')) {
        return Err(cx.for_rule(rule).invalid_shape(r"\eqalignno row should end with a parenthesized tag"));
    }

    let Some(amp_index) = row
        .windows(2)
        .rposition(|pair| cx.ast.is_char(pair[0], '&') && cx.ast.is_char(pair[1], '('))
    else {
        return Err(cx.for_rule(rule).invalid_shape(r"\eqalignno row should contain a final &(...) tag"));
    };

    let tag_tail = row.split_off(amp_index);
    let tag = text_node_from_math_nodes(rule, cx, &tag_tail[2..tag_tail.len() - 1])?;
    for node in tag_tail {
        cx.ast.remove_detached(node);
    }

    Ok((row, tag))
}

fn text_node_from_math_nodes(
    rule: crate::rewrite::rule::RuleKey,
    cx: &mut RuleContext<'_>,
    nodes: &[NodeId],
) -> Result<NodeId, RuleError> {
    let mut text = String::new();
    for node in nodes {
        match cx.ast.node(*node) {
            Node::Char(ch) => text.push(*ch),
            Node::Text(value) => text.push_str(value),
            _ => {
                return Err(cx.for_rule(rule).invalid_shape(r"\eqalignno tags should contain text-like content"));
            }
        }
    }
    Ok(cx.ast.new_node(Node::Text(text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RewriteError, RewriteRule as _, RuleClass, RuleError};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: EQALIGNNO_TO_ALIGN_ENV,
        class: Standard,
        examples: [
        {
            label: eqalignno_right_tag_branch,
            packages: ["base", "ams"],
            input: r"\eqalignno{F(x)&=\int_0^x f(t)\,dt&(1)\cr F'(x)&=f(x)&(2)\cr F''(x)&=f'(x)&(3)}",
            expected: r"\begin{align} F(x)&=\int_0^x f(t)\,dt \tag{1}\\ F'(x)&=f(x) \tag{2}\\ F''(x)&=f'(x) \tag{3} \end{align}",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn rejects_non_text_like_tag_content() {
        let parse_ctx = crate::parse::ParseContext::from_packages(&["base", "ams"]);
        let mut ast = parse_ctx
            .parse_to_ast(r"\eqalignno{x&=y&(n_i)}", true)
            .expect("parse should succeed");

        let err = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &EQALIGNNO_TO_ALIGN_ENV,
            RuleClass::Standard,
        )
            .expect_err("scripted equation tags are not valid text-like tag content");

        assert!(matches!(
            err,
            crate::TransformError::Rewrite(RewriteError::Rule {
                kind: RuleError::InvalidNodeShape { message },
                ..
            })
                if message.contains("text-like content")
        ));
    }
}
