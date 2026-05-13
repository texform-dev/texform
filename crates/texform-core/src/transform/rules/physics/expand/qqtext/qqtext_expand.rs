//! Expand quick-quad prose helpers, including the star branch, to explicit text and quad spacing.
//!
//! ```yaml
//! proposal: qqtext-expand
//! triggers:
//!   - cmd:qqtext
//!   - cmd:qq
//! consumes:
//!   eliminates: [cmd:qqtext, cmd:qq]
//!   touches: null
//! produces:
//!   - cmd:text
//!   - cmd:quad
//! rewrite_patterns:
//!   - {label: qqtext, from: '\qqtext{#1}', to: '\quad\text{#1}\quad'}
//!   - {label: qqtext-star, from: '\qqtext*{#1}', to: '\text{#1}\quad'}
//!   - {label: qq, from: '\qq{#1}', to: '\quad\text{#1}\quad'}
//!   - {label: qq-star, from: '\qq*{#1}', to: '\text{#1}\quad'}
//! ```

use texform_specs::builtin::base;
use texform_specs::builtin::physics;

use crate::ast::{Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Node};
use crate::transform::engine::TransformError;
use crate::transform::helpers::prefix_command_node;
use crate::transform::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::transform::rule_context::RuleContext;
use crate::transform::{cmd_targets, define_rule};

define_rule! {
    pub static QQTEXT_EXPAND: QqtextExpandRule {
        key: Physics / "qqtext-expand",
        class: Expand,
        summary: "Expand quick-quad prose helpers, including the star branch, to explicit text and quad spacing.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Physics],
        triggers: cmd_targets![&physics::cmd::QQTEXT, &physics::cmd::QQ],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&physics::cmd::QQTEXT, &physics::cmd::QQ],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::TEXT, &base::cmd::QUAD],
        },
        apply(rule, cx, node_id) {
            expand_qqtext_like(rule, cx, node_id)
        }
    }
}

fn expand_qqtext_like(
    _rule: &QqtextExpandRule,
    cx: &mut RuleContext<'_>,
    node_id: crate::ast::NodeId,
) -> Result<RuleEffect, TransformError> {
    let (subject, args) = match cx.node(node_id) {
        Node::Command { name, args, .. }
            if name == physics::cmd::QQTEXT.name || name == physics::cmd::QQ.name =>
        {
            (format!("\\{name}"), args.clone())
        }
        _ => return Ok(RuleEffect::Skipped),
    };

    cx.for_rule(QqtextExpandRule::KEY).expect_arg_len(&args, 2, &subject)?;
    let starred = cx.for_rule(QqtextExpandRule::KEY).star_arg_value(&args[0], &subject)?;
    let text_arg = text_argument(cx, &args[1], &subject)?;

    replace_with_text_sequence(cx, node_id, starred, text_arg);

    Ok(RuleEffect::Applied)
}

fn text_argument(
    cx: &mut RuleContext<'_>,
    slot: &ArgumentSlot,
    subject: &str,
) -> Result<ArgumentSlot, TransformError> {
    match slot {
        Some(arg) if arg.kind == ArgumentKind::Mandatory => match arg.value {
            ArgumentValue::TextContent(node_id) => Ok(Some(Argument {
                kind: arg.kind.clone(),
                value: ArgumentValue::TextContent(cx.ast.clone_subtree(node_id)),
            })),
            _ => Err(cx.for_rule(QqtextExpandRule::KEY).invalid_shape(format!("{subject} should carry a mandatory text argument"))),
        },
        _ => {
            Err(cx.for_rule(QqtextExpandRule::KEY).invalid_shape(format!("{subject} should carry a mandatory text argument")))
        }
    }
}

fn replace_with_text_sequence(
    cx: &mut RuleContext<'_>,
    node_id: crate::ast::NodeId,
    starred: bool,
    text_arg: ArgumentSlot,
) {
    let before = if starred {
        Vec::new()
    } else {
        vec![cx.ast.new_node(prefix_command_node(&base::cmd::QUAD, Vec::new()))]
    };
    let after = vec![cx.ast.new_node(prefix_command_node(&base::cmd::QUAD, Vec::new()))];
    let text_command = cx
        .ast
        .new_node(prefix_command_node(&base::cmd::TEXT, vec![text_arg]));
    cx.ast.replace_with_math_sequence(
        node_id,
        before,
        text_command,
        after,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: QQTEXT_EXPAND,
        class: Expand,
        examples: [
        {
            label: qqtext_inline_clause,
            packages: ["base", "physics"],
            input: r"E=mc^2 \qqtext{for} v \ll c",
            expected: r"E=mc^2 \quad\text{for}\quad v \ll c",
        },
        {
            label: qqtext_star_inline_clause,
            packages: ["base", "physics"],
            input: r"f(x)=x^2\qqtext*{if} x>0",
            expected: r"f(x)=x^2\text{if}\quad x>0",
        },
        {
            label: qq_nonstar_alias,
            packages: ["base", "physics"],
            input: r"\Delta S=0 \qq{at extrema}",
            expected: r"\Delta S=0 \quad\text{at extrema}\quad",
        },
        {
            label: qq_star_inline_clause,
            packages: ["base", "physics"],
            input: r"A=B\qq*{where} B>0",
            expected: r"A=B\text{where}\quad B>0",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn groups_qq_expansion_when_not_a_sibling_node() {
        use crate::parse::ParseContext;
        use crate::transform::TransformRule as _;
        use crate::transform::{transform_ast, RuleClass, TransformContextBuilder};

        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let transform_ctx = TransformContextBuilder::from_classes(&[RuleClass::Expand])
            .only(QQTEXT_EXPAND.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\qq{if}^2", true)
            .expect("parse should succeed");

        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("qqtext-expand transform should succeed");

        assert_eq!(output.applied.len(), 1);
        assert_eq!(output.applied[0].count, 1);
    }
}
