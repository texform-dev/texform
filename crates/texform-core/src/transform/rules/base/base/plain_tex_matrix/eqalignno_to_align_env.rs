//! Rewrite eqalignno to the standard align environment.
//!
//! ```yaml
//! proposal: eqalignno-to-align-env
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
    cr_rows, is_char, linebreak_command, mandatory_math_body, replace_with_environment, tag_command,
};
use crate::ast::{Node, NodeId};
use crate::transform::engine::TransformError;
use crate::transform::rule::{RuleConsumes, RuleProduces, RuleTarget};
use crate::transform::rule_context::RuleContext;
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    /// Rewrite eqalignno to the standard align environment.
    pub static EQALIGNNO_TO_ALIGN_ENV: EqalignnoToAlignEnvRule {
        key: Base / "eqalignno-to-align-env",
        tier: Base,
        summary: "Rewrite eqalignno to the standard align environment.",
        phase: Normalize,
        safety: Semantic,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::EQALIGNNO, &base::cmd::CR],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::ALIGN), RuleTarget::Command(&ams::cmd::TAG)],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::EQALIGNNO) else {
                return Ok(crate::transform::rule::RuleEffect::Skipped);
            };
            cx.expect_arg_len(rule.meta().key, command.args, 1, r"\eqalignno")?;
            let body = mandatory_math_body(
                rule.meta().key,
                cx,
                &command.args[0],
                base::cmd::EQALIGNNO.name,
            )?;
            let rows = cr_rows(cx, body);
            let row_count = rows.len();
            let mut children = Vec::new();

            for (index, row) in rows.into_iter().enumerate() {
                let (row, tag) = split_eqalignno_row(rule.meta().key, cx, row)?;
                children.extend(row);
                children.push(cx.ast.new_node(tag_command(tag)));
                if index + 1 < row_count {
                    children.push(cx.ast.new_node(linebreak_command()));
                }
            }

            replace_with_environment(cx, node_id, &ams::env::ALIGN, Vec::new(), children);
            Ok(crate::transform::rule::RuleEffect::Applied)
        }
    }
}

fn split_eqalignno_row(
    rule: crate::transform::rule::RuleKey,
    cx: &mut RuleContext<'_>,
    mut row: Vec<NodeId>,
) -> Result<(Vec<NodeId>, NodeId), TransformError> {
    let Some(last) = row.last().copied() else {
        return Err(cx.invalid_shape(rule, r"\eqalignno row should not be empty"));
    };
    if !matches!(cx.ast.node(last), Node::Char(')')) {
        return Err(cx.invalid_shape(rule, r"\eqalignno row should end with a parenthesized tag"));
    }

    let Some(amp_index) = row
        .windows(2)
        .rposition(|pair| is_char(cx, pair[0], '&') && is_char(cx, pair[1], '('))
    else {
        return Err(cx.invalid_shape(
            rule,
            r"\eqalignno row should contain a final &(...) tag",
        ));
    };

    let tag_tail = row.split_off(amp_index);
    let tag = text_node_from_math_nodes(rule, cx, &tag_tail[2..tag_tail.len() - 1])?;
    for node in tag_tail {
        cx.ast.remove_detached(node);
    }

    Ok((row, tag))
}

fn text_node_from_math_nodes(
    rule: crate::transform::rule::RuleKey,
    cx: &mut RuleContext<'_>,
    nodes: &[NodeId],
) -> Result<NodeId, TransformError> {
    let mut text = String::new();
    for node in nodes {
        match cx.ast.node(*node) {
            Node::Char(ch) => text.push(*ch),
            Node::Text(value) => text.push_str(value),
            _ => {
                return Err(cx.invalid_shape(
                    rule,
                    r"\eqalignno tags should contain text-like content",
                ));
            }
        }
    }
    Ok(cx.ast.new_node(Node::Text(text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::engine::{TransformEngineError, TransformError};
    use crate::transform::transform_examples;
    use crate::transform::{TransformProfile, TransformRule as _, transform_ast};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: EQALIGNNO_TO_ALIGN_ENV,
        tier: Base,
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
        let transform_ctx = TransformProfile::AUTHORING
            .builder()
            .only(EQALIGNNO_TO_ALIGN_ENV.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\eqalignno{x&=y&(n_i)}", true)
            .expect("parse should succeed");

        let err = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect_err("scripted equation tags are not valid text-like tag content");

        assert!(matches!(
            err,
            TransformEngineError::Rule(TransformError::InvalidNodeShape { message, .. })
                if message.contains("text-like content")
        ));
    }
}
