//! Rewrite eqalignno to the standard align* environment.
//!
//! The third alignment cell is mathematical content, not a text-only equation number. A complete outer parenthesis pair becomes \tag; other nonempty cells use \tag* so no parentheses are added. Rows without a numbering cell get no tag, matching \eqalignno, which never numbers rows automatically.
//!
//! ```yaml
//! proposal: eqalignno-to-align-env
//! triggers:
//!   - cmd:eqalignno
//! consumes:
//!   eliminates: cmd:eqalignno
//!   touches: [cmd:cr, cmd:\, cmd:newline]
//! produces:
//!   - env:align*
//!   - cmd:tag
//! rewrite_patterns:
//!   - {label: parenthesized-numeric-number, from: '\eqalignno{#1&#2&(#3)}', to: '\begin{align*} #1&#2 \tag{#3} \end{align*}'}
//!   - {label: raw-numeric-number, from: '\eqalignno{#1&#2&#3}', to: '\begin{align*} #1&#2 \tag*{#3} \end{align*}'}
//!   - {label: parenthesized-math-number, from: '\eqalignno{#1&#2&(#3)}', to: '\begin{align*} #1&#2 \tag{$#3$} \end{align*}'}
//!   - {label: raw-math-number, from: '\eqalignno{#1&#2&#3}', to: '\begin{align*} #1&#2 \tag*{$#3$} \end{align*}'}
//!   - {label: empty-number-cell, from: '\eqalignno{#1&#2&}', to: '\begin{align*} #1&#2 \end{align*}'}
//!   - {label: missing-number-cell, from: '\eqalignno{#1&#2}', to: '\begin{align*} #1&#2 \end{align*}'}
//!   - {label: final-row-terminator, from: '\eqalignno{#1&#2&(#3)\cr}', to: '\begin{align*} #1&#2 \tag{#3} \end{align*}'}
//!   - {label: internal-empty-row, from: '\eqalignno{#1&#2&(#3)\cr\cr#4&#5&(#6)}', to: '\begin{align*} #1&#2 \tag{#3}\\\\#4&#5 \tag{#6} \end{align*}'}
//!   - {label: linebreak-arguments, from: '\eqalignno{#1&#2&(#3)\\*[#4]}', to: '\begin{align*} #1&#2 \tag{#3}\\*[#4] \end{align*}'}
//!   - {label: newline, from: '\eqalignno{#1&#2&(#3)\newline#4&#5&(#6)}', to: '\begin{align*} #1&#2 \tag{#3}\\#4&#5 \tag{#6} \end{align*}'}
//!   - {label: leading-row-scripts, from: '\eqalignno{#1&#2&(#3)\cr_{#4}^{#5}#6&#7&(#8)}', to: '\begin{align*} #1&#2 \tag{#3}\\{}_{#4}^{#5}#6&#7 \tag{#8} \end{align*}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::base;

use super::helpers::{
    alignment_rows, mandatory_math_body, replace_with_environment, tag_command,
};
use crate::ast::{ContentMode, GroupKind, Node, NodeId};
use crate::rewrite::RuleError;
use crate::rewrite::rule::{RuleConsumes, RuleProduces, RuleTarget};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static EQALIGNNO_TO_ALIGN_ENV: EqalignnoToAlignEnvRule {
        key: Base / "eqalignno-to-align-env",
        level: Authoring,
        summary: "Rewrite eqalignno to the standard align* environment.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::EQALIGNNO],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::EQALIGNNO],
            touches: cmd_targets![&base::cmd::CR, &base::cmd::_BACKSLASH, &base::cmd::NEWLINE],
        },
        produces: RuleProduces {
            targets: &[RuleTarget::Environment(&ams::env::ALIGN_STAR), RuleTarget::Command(&ams::cmd::TAG)],
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
            let mut children = Vec::new();

            for row in alignment_rows(cx, body) {
                let (content, tag) = split_eqalignno_row(Self::KEY, cx, row.children)?;
                children.extend(content);
                if let Some(tag) = tag {
                    children.push(cx.ast.new_node(tag));
                }
                children.extend(row.terminator);
            }

            replace_with_environment(cx, node_id, &ams::env::ALIGN_STAR, Vec::new(), children);
            Ok(crate::rewrite::rule::RuleEffect::Applied)
        }
    }
}

/// Splits off the numbering cell after the second top-level `&` and returns
/// the remaining row with the tag command for a nonempty numbering cell.
fn split_eqalignno_row(
    rule: crate::rewrite::rule::RuleKey,
    cx: &mut RuleContext<'_>,
    mut row: Vec<NodeId>,
) -> Result<(Vec<NodeId>, Option<Node>), RuleError> {
    let Some(amp_index) = row
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(cx.ast.node(**node), Node::AlignmentTab))
        .nth(1)
        .map(|(index, _)| index)
    else {
        return Ok((row, None));
    };

    let mut tag_nodes = row.split_off(amp_index + 1);
    if tag_nodes
        .iter()
        .any(|node| matches!(cx.ast.node(*node), Node::AlignmentTab))
    {
        return Err(cx
            .for_rule(rule)
            .invalid_shape(r"\eqalignno rows should contain at most three alignment cells"));
    }
    cx.ast.remove_detached(row.pop().expect("the numbering separator exists"));
    if tag_nodes.is_empty() {
        return Ok((row, None));
    }

    let parenthesized = has_outer_parentheses(cx, &tag_nodes);
    if parenthesized {
        cx.ast.remove_detached(tag_nodes.pop().expect("a parenthesized tag ends with `)`"));
        cx.ast.remove_detached(tag_nodes.remove(0));
    }
    let tag = tag_content(cx, tag_nodes);
    Ok((row, Some(tag_command(tag, !parenthesized))))
}

/// Returns whether the first `(` and the last `)` enclose the whole cell.
fn has_outer_parentheses(cx: &RuleContext<'_>, nodes: &[NodeId]) -> bool {
    if !matches!(nodes.first().map(|node| cx.ast.node(*node)), Some(Node::Char('('))) {
        return false;
    }
    let mut depth = 0;
    for (index, node) in nodes.iter().enumerate() {
        match cx.ast.node(*node) {
            Node::Char('(') => depth += 1,
            Node::Char(')') => {
                depth -= 1;
                if depth == 0 {
                    return index + 1 == nodes.len();
                }
            }
            _ => {}
        }
    }
    false
}

fn tag_content(cx: &mut RuleContext<'_>, nodes: Vec<NodeId>) -> NodeId {
    // Digits and periods look the same in text and math, so such tags become
    // plain text. Other tags keep their math subtree as inline math inside the
    // text-mode tag argument.
    let mut text = String::new();
    let plain = nodes.iter().all(|node| match cx.ast.node(*node) {
        Node::Char(ch) if ch.is_ascii_digit() || *ch == '.' => {
            text.push(*ch);
            true
        }
        _ => false,
    });
    if plain {
        for node in nodes {
            cx.ast.remove_detached(node);
        }
        return cx.ast.new_node(Node::Text(text));
    }

    let math = cx.ast.new_node(Node::Group {
        children: nodes,
        kind: GroupKind::InlineMath,
        mode: ContentMode::Math,
    });
    cx.ast.new_node(Node::Group {
        children: vec![math],
        kind: GroupKind::Implicit,
        mode: ContentMode::Text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: EQALIGNNO_TO_ALIGN_ENV,
        level: Authoring,
        examples: [
        {
            label: eqalignno_right_tag_branch,
            packages: ["base", "ams"],
            input: r"\eqalignno{F(x)&=\int_0^x f(t)\,dt&(1)\cr F'(x)&=f(x)&(2)\cr F''(x)&=f'(x)&(3)}",
            expected: r"\begin{align*} F(x)&=\int_0^x f(t)\,dt \tag{1}\\ F'(x)&=f(x) \tag{2}\\ F''(x)&=f'(x) \tag{3} \end{align*}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: EQALIGNNO_TO_ALIGN_ENV,
        level: Authoring,
        examples: [
            {
                label: drops_the_segment_after_a_final_row_terminator,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&(1)\cr}",
                expected: r"\begin{align*}x&=y\tag{1}\end{align*}",
            },
            {
                label: keeps_digits_and_periods_as_text,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&(1.2)}",
                expected: r"\begin{align*}x&=y\tag{1.2}\end{align*}",
            },
            {
                label: stars_tags_without_outer_parentheses,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&1}",
                expected: r"\begin{align*}x&=y\tag*{1}\end{align*}",
            },
            {
                label: keeps_other_tag_content_as_math,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&(2a^*)}",
                expected: r"\begin{align*}x&=y\tag{$2a^*$}\end{align*}",
            },
            {
                label: keeps_separate_parenthesized_parts_as_one_starred_tag,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&(n)+(m)}",
                expected: r"\begin{align*}x&=y\tag*{$(n)+(m)$}\end{align*}",
            },
            {
                label: leaves_rows_without_a_number_untagged,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&\cr z&=w\cr\cr u&=v&(2)\cr}",
                expected: r"\begin{align*}x&=y\\z&=w\\\\u&=v\tag{2}\end{align*}",
            },
            {
                label: ignores_alignment_tabs_nested_in_the_number,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&\begin{matrix}n&i\end{matrix}}",
                expected: r"\begin{align*}x&=y\tag*{$\begin{matrix}n&i\end{matrix}$}\end{align*}",
            },
            {
                label: starts_rows_with_leading_scripts,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&(1)\cr_Mx&=z&(2)\cr}",
                expected: r"\begin{align*}x&=y\tag{1}\\{}_Mx&=z\tag{2}\end{align*}",
            },
            {
                label: keeps_linebreaks_out_of_the_number_cell,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&(1)\\[2pt]z&=w&(2)\newline}",
                expected: r"\begin{align*}x&=y\tag{1}\\[2pt]z&=w\tag{2}\end{align*}",
            },
            {
                label: does_not_split_rows_inside_groups,
                packages: ["base", "ams"],
                input: r"\eqalignno{x&=y&({\cr})}",
                expected: r"\begin{align*}x&=y\tag{${\cr}$}\end{align*}",
            },
        ]
    }
}
